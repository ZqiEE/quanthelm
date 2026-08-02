use std::{
    error::Error,
    io,
    path::{Path, PathBuf},
    str::FromStr,
    time::Duration,
};

use chrono::{DateTime, Utc};
use clap::Parser;
use qh_binance::{
    BinancePublicClient, KlineStreamItem, KlineStreamSupervisor, MAINNET_REST_BASE,
    StreamLifecycle,
};
use qh_domain::Symbol;
use qh_market_data::{Continuity, GapDetector, Interval, Kline, RawEvent};
use qh_storage::JsonlStore;
use serde::Serialize;
use tokio::{fs, sync::{mpsc, watch}, time};
use tracing::{info, warn};
use tracing_subscriber::EnvFilter;
use uuid::Uuid;

type AppError = Box<dyn Error + Send + Sync>;

/// Runs an identified, read-only Binance USDⓈ-M Kline collector.
#[derive(Debug, Parser)]
#[command(name = "quanthelm-collector", version)]
struct Cli {
    /// Exact contract symbol. Repeat for multiple symbols.
    #[arg(long = "symbol", required = true)]
    symbols: Vec<String>,
    /// Supported interval: 15m, 1h, or 4h.
    #[arg(long)]
    interval: String,
    /// Directory for raw events, accepted candles, and metrics.
    #[arg(long)]
    output_directory: PathBuf,
    /// Metrics snapshot interval in seconds.
    #[arg(long, default_value_t = 60)]
    metrics_interval_seconds: u64,
    /// Public REST base override used only for gap recovery.
    #[arg(long, default_value = MAINNET_REST_BASE)]
    rest_base: String,
    /// Logging filter.
    #[arg(long, default_value = "info")]
    log: String,
}

#[derive(Debug, Clone, Serialize)]
struct CollectedRawEvent {
    collector_instance_id: Uuid,
    event: RawEvent,
}

#[derive(Debug, Clone, Serialize)]
struct CollectorMetrics {
    collector_instance_id: Uuid,
    started_at: DateTime<Utc>,
    completed_at: Option<DateTime<Utc>>,
    connecting: u64,
    connected: u64,
    rotating: u64,
    disconnected: u64,
    raw_events: u64,
    closed_klines: u64,
    protocol_errors: u64,
    gaps_detected: u64,
    recovered_klines: u64,
    duplicates_ignored: u64,
    out_of_order_ignored: u64,
    last_event_at: Option<DateTime<Utc>>,
}

impl CollectorMetrics {
    fn new(collector_instance_id: Uuid) -> Self {
        Self {
            collector_instance_id,
            started_at: Utc::now(),
            completed_at: None,
            connecting: 0,
            connected: 0,
            rotating: 0,
            disconnected: 0,
            raw_events: 0,
            closed_klines: 0,
            protocol_errors: 0,
            gaps_detected: 0,
            recovered_klines: 0,
            duplicates_ignored: 0,
            out_of_order_ignored: 0,
            last_event_at: None,
        }
    }

    fn observe_lifecycle(&mut self, state: &StreamLifecycle) {
        match state {
            StreamLifecycle::Connecting => self.connecting += 1,
            StreamLifecycle::Connected => self.connected += 1,
            StreamLifecycle::Rotating => self.rotating += 1,
            StreamLifecycle::Disconnected { .. } => self.disconnected += 1,
        }
        self.last_event_at = Some(Utc::now());
    }

    fn observe_ingest(&mut self, outcome: IngestOutcome) {
        match outcome {
            IngestOutcome::Accepted => self.closed_klines += 1,
            IngestOutcome::Recovered { candles } => {
                self.gaps_detected += 1;
                self.recovered_klines += candles;
                self.closed_klines += candles + 1;
            }
            IngestOutcome::Duplicate => self.duplicates_ignored += 1,
            IngestOutcome::OutOfOrder => self.out_of_order_ignored += 1,
        }
        self.last_event_at = Some(Utc::now());
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum IngestOutcome {
    Accepted,
    Recovered { candles: u64 },
    Duplicate,
    OutOfOrder,
}

#[tokio::main]
async fn main() -> Result<(), AppError> {
    let cli = Cli::parse();
    init_tracing(&cli.log)?;
    run(cli).await
}

async fn run(cli: Cli) -> Result<(), AppError> {
    if cli.metrics_interval_seconds == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "metrics interval must be positive",
        )
        .into());
    }

    let interval = Interval::from_str(&cli.interval)?;
    let pairs = cli
        .symbols
        .into_iter()
        .map(|symbol| Symbol::from_str(&symbol).map(|symbol| (symbol, interval)))
        .collect::<Result<Vec<_>, _>>()?;
    let collector_instance_id = Uuid::new_v4();
    let supervisor = KlineStreamSupervisor::new(&pairs)?;
    let client = BinancePublicClient::with_rest_base(cli.rest_base)?;
    let raw_store = JsonlStore::new(cli.output_directory.join("raw.jsonl"));
    let candle_store = JsonlStore::new(cli.output_directory.join("closed-klines.jsonl"));
    let metrics_path = cli.output_directory.join("metrics.json");
    let (output_tx, mut output_rx) = mpsc::channel(2_048);
    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let supervisor_task = tokio::spawn(supervisor.run(output_tx, shutdown_rx));
    let mut detector = GapDetector::default();
    let mut metrics = CollectorMetrics::new(collector_instance_id);
    let mut snapshot_tick = time::interval(Duration::from_secs(cli.metrics_interval_seconds));
    snapshot_tick.set_missed_tick_behavior(time::MissedTickBehavior::Skip);
    let stop = tokio::signal::ctrl_c();
    tokio::pin!(stop);

    info!(%collector_instance_id, "starting read-only market-data collector");
    write_metrics(&metrics_path, &metrics).await?;

    loop {
        tokio::select! {
            result = &mut stop => {
                result?;
                let _ = shutdown_tx.send(true);
                break;
            }
            _ = snapshot_tick.tick() => {
                write_metrics(&metrics_path, &metrics).await?;
            }
            item = output_rx.recv() => {
                let Some(item) = item else {
                    break;
                };
                match item {
                    KlineStreamItem::Lifecycle(state) => {
                        metrics.observe_lifecycle(&state);
                        log_lifecycle(&state);
                    }
                    KlineStreamItem::Raw(event) => {
                        metrics.raw_events += 1;
                        metrics.last_event_at = Some(Utc::now());
                        raw_store.append(&CollectedRawEvent {
                            collector_instance_id,
                            event,
                        }).await?;
                    }
                    KlineStreamItem::ClosedKline(candle) => {
                        let outcome = ingest_with_backfill(
                            &client,
                            &mut detector,
                            &candle_store,
                            candle,
                        ).await?;
                        metrics.observe_ingest(outcome);
                    }
                    KlineStreamItem::ProtocolError { message } => {
                        metrics.protocol_errors += 1;
                        metrics.last_event_at = Some(Utc::now());
                        warn!(%message, "rejected Binance WebSocket payload");
                    }
                }
            }
        }
    }

    let _ = shutdown_tx.send(true);
    match supervisor_task.await {
        Ok(result) => result?,
        Err(error) => return Err(Box::new(error)),
    }
    metrics.completed_at = Some(Utc::now());
    write_metrics(&metrics_path, &metrics).await?;
    info!(%collector_instance_id, "collector stopped cleanly");
    Ok(())
}

async fn ingest_with_backfill(
    client: &BinancePublicClient,
    detector: &mut GapDetector,
    store: &JsonlStore,
    candle: Kline,
) -> Result<IngestOutcome, AppError> {
    match detector.observe(&candle) {
        Continuity::First | Continuity::Continuous => {
            store.append(&candle).await?;
            Ok(IngestOutcome::Accepted)
        }
        Continuity::Duplicate => {
            warn!(symbol = %candle.symbol, open_time = %candle.open_time, "ignored duplicate candle");
            Ok(IngestOutcome::Duplicate)
        }
        Continuity::OutOfOrder { latest_open_time, .. } => {
            warn!(symbol = %candle.symbol, open_time = %candle.open_time, %latest_open_time, "ignored out-of-order candle");
            Ok(IngestOutcome::OutOfOrder)
        }
        Continuity::Gap {
            expected_open_time,
            actual_open_time,
            missing_candles,
        } => {
            let recovered = client
                .backfill_klines(
                    &candle.symbol,
                    candle.interval,
                    expected_open_time,
                    actual_open_time,
                )
                .await?;
            for missing in recovered {
                accept_continuous(detector, store, missing).await?;
            }
            accept_continuous(detector, store, candle).await?;
            Ok(IngestOutcome::Recovered {
                candles: missing_candles,
            })
        }
    }
}

async fn accept_continuous(
    detector: &mut GapDetector,
    store: &JsonlStore,
    candle: Kline,
) -> Result<(), AppError> {
    match detector.observe(&candle) {
        Continuity::First | Continuity::Continuous => {
            store.append(&candle).await?;
            Ok(())
        }
        continuity => Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "REST recovery did not restore continuity for {} at {}: {continuity:?}",
                candle.symbol, candle.open_time
            ),
        )
        .into()),
    }
}

async fn write_metrics(path: &Path, metrics: &CollectorMetrics) -> Result<(), AppError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).await?;
    }
    let temporary = path.with_extension("json.tmp");
    let mut encoded = serde_json::to_vec_pretty(metrics)?;
    encoded.push(b'\n');
    fs::write(&temporary, encoded).await?;
    fs::rename(temporary, path).await?;
    Ok(())
}

fn log_lifecycle(state: &StreamLifecycle) {
    match state {
        StreamLifecycle::Connecting => info!("connecting Binance market-data stream"),
        StreamLifecycle::Connected => info!("Binance market-data stream connected"),
        StreamLifecycle::Rotating => info!("rotating Binance market-data stream"),
        StreamLifecycle::Disconnected { reason } => {
            warn!(%reason, "Binance market-data stream disconnected");
        }
    }
}

fn init_tracing(filter: &str) -> Result<(), AppError> {
    let filter = EnvFilter::try_new(filter)?;
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .try_init()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metrics_count_recovery_and_lifecycle() {
        let mut metrics = CollectorMetrics::new(Uuid::nil());
        metrics.observe_lifecycle(&StreamLifecycle::Connecting);
        metrics.observe_lifecycle(&StreamLifecycle::Connected);
        metrics.observe_ingest(IngestOutcome::Recovered { candles: 2 });
        metrics.observe_ingest(IngestOutcome::Duplicate);

        assert_eq!(metrics.connecting, 1);
        assert_eq!(metrics.connected, 1);
        assert_eq!(metrics.gaps_detected, 1);
        assert_eq!(metrics.recovered_klines, 2);
        assert_eq!(metrics.closed_klines, 3);
        assert_eq!(metrics.duplicates_ignored, 1);
    }

    #[test]
    fn raw_envelope_contains_instance_identity() {
        let collector_instance_id = Uuid::new_v4();
        let envelope = CollectedRawEvent {
            collector_instance_id,
            event: RawEvent::new("binance", "test", None, Utc::now(), "{}"),
        };
        let encoded = serde_json::to_string(&envelope).expect("serialize envelope");
        assert!(encoded.contains(&collector_instance_id.to_string()));
    }
}
