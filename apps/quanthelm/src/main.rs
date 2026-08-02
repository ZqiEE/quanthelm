//! QuantHelm command-line entry point.

use std::{
    error::Error,
    io,
    path::PathBuf,
    str::FromStr,
};

use clap::{Parser, Subcommand};
use qh_binance::{
    BinancePublicClient,
    KlineRequest,
    MAINNET_REST_BASE,
};
use qh_config::AppConfig;
use qh_domain::Symbol;
use qh_market_data::{
    Continuity,
    GapDetector,
    Interval,
    Kline,
};
use qh_storage::JsonlStore;
use tracing::info;
use tracing_subscriber::EnvFilter;

type AppError = Box<dyn Error + Send + Sync>;

#[derive(Debug, Parser)]
#[command(
    name = "quanthelm",
    version,
    about = "AI-native quantitative trading system"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Prints the current engineering status without contacting an exchange.
    Status,
    /// Loads and validates environment configuration.
    ValidateConfig,
    /// Read-only Binance USD-M public market-data commands.
    Binance {
        /// Binance market-data operation.
        #[command(subcommand)]
        command: BinanceCommand,
    },
    /// Replays stored candles and reports continuity problems.
    Replay {
        /// JSONL file created by `download-klines`.
        #[arg(long)]
        input: PathBuf,
    },
}

#[derive(Debug, Subcommand)]
enum BinanceCommand {
    /// Fetches current exchange rules and prints one symbol or a summary.
    ExchangeInfo {
        /// Optional exact contract symbol.
        #[arg(long)]
        symbol: Option<String>,
        /// Public REST base override.
        #[arg(long, default_value = MAINNET_REST_BASE)]
        rest_base: String,
    },
    /// Downloads normalized candles into append-only JSONL storage.
    DownloadKlines {
        /// Exact contract symbol, for example BTCUSDT.
        #[arg(long)]
        symbol: String,
        /// Supported interval: 15m, 1h, or 4h.
        #[arg(long)]
        interval: String,
        /// Number of candles, 1 through 1500.
        #[arg(long, default_value_t = 500)]
        limit: u16,
        /// Optional inclusive start timestamp in milliseconds.
        #[arg(long)]
        start_time_ms: Option<i64>,
        /// Optional inclusive end timestamp in milliseconds.
        #[arg(long)]
        end_time_ms: Option<i64>,
        /// Output JSONL file.
        #[arg(long)]
        output: PathBuf,
        /// Public REST base override.
        #[arg(long, default_value = MAINNET_REST_BASE)]
        rest_base: String,
    },
}

#[tokio::main]
async fn main() -> Result<(), AppError> {
    let cli = Cli::parse();
    let config = AppConfig::from_env()?;
    init_tracing(&config.log_filter)?;

    match cli.command {
        Command::Status => {
            info!(environment = ?config.environment, "QuantHelm initialized");
            println!("QuantHelm 0.1.0 — M1 market-data slice");
            println!("Environment: {:?}", config.environment);
            println!("Exchange writes: disabled");
            println!("API credentials: not accepted by market-data commands");
            println!("AI trading authority: none");
        }
        Command::ValidateConfig => {
            println!("{}", serde_json::to_string_pretty(&config)?);
        }
        Command::Binance { command } => run_binance(command).await?,
        Command::Replay { input } => replay(input).await?,
    }

    Ok(())
}

async fn run_binance(command: BinanceCommand) -> Result<(), AppError> {
    match command {
        BinanceCommand::ExchangeInfo { symbol, rest_base } => {
            let client = BinancePublicClient::with_rest_base(rest_base)?;
            let info = client.exchange_info().await?;
            if let Some(symbol) = symbol {
                let symbol = Symbol::from_str(&symbol)?;
                let contract = info.contract(&symbol).ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::NotFound,
                        format!("symbol not present in exchange info: {symbol}"),
                    )
                })?;
                println!("{}", serde_json::to_string_pretty(contract)?);
            } else {
                println!("Server time: {}", info.server_time);
                println!("Contracts: {}", info.contracts.len());
                println!(
                    "Trading perpetuals: {}",
                    info.trading_perpetuals().count()
                );
                println!("Raw SHA-256: {}", info.raw_event.payload_sha256);
            }
        }
        BinanceCommand::DownloadKlines {
            symbol,
            interval,
            limit,
            start_time_ms,
            end_time_ms,
            output,
            rest_base,
        } => {
            let request = KlineRequest {
                symbol: Symbol::from_str(&symbol)?,
                interval: Interval::from_str(&interval)?,
                start_time_ms,
                end_time_ms,
                limit,
            };
            let client = BinancePublicClient::with_rest_base(rest_base)?;
            let candles = client.klines(&request).await?;
            let store = JsonlStore::new(&output);
            let mut detector = GapDetector::default();
            let mut gaps = 0_u64;
            let mut duplicates = 0_u64;
            let mut out_of_order = 0_u64;

            for candle in &candles {
                match detector.observe(candle) {
                    Continuity::Gap { .. } => gaps += 1,
                    Continuity::Duplicate => duplicates += 1,
                    Continuity::OutOfOrder { .. } => out_of_order += 1,
                    Continuity::First | Continuity::Continuous => {}
                }
                store.append(candle).await?;
            }

            println!("Stored {} candles in {}", candles.len(), output.display());
            println!("Gaps: {gaps}");
            println!("Duplicates: {duplicates}");
            println!("Out of order: {out_of_order}");
        }
    }
    Ok(())
}

async fn replay(input: PathBuf) -> Result<(), AppError> {
    let store = JsonlStore::new(&input);
    let candles: Vec<Kline> = store.read_all().await?;
    let mut detector = GapDetector::default();
    let mut continuous = 0_u64;
    let mut gaps = 0_u64;
    let mut duplicates = 0_u64;
    let mut out_of_order = 0_u64;

    for candle in &candles {
        match detector.observe(candle) {
            Continuity::First | Continuity::Continuous => continuous += 1,
            Continuity::Gap { .. } => gaps += 1,
            Continuity::Duplicate => duplicates += 1,
            Continuity::OutOfOrder { .. } => out_of_order += 1,
        }
    }

    println!("Replayed {} candles from {}", candles.len(), input.display());
    println!("First or continuous: {continuous}");
    println!("Gaps: {gaps}");
    println!("Duplicates: {duplicates}");
    println!("Out of order: {out_of_order}");
    Ok(())
}

fn init_tracing(filter: &str) -> Result<(), AppError> {
    let filter = EnvFilter::try_new(filter)?;
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .try_init()?;
    Ok(())
}
