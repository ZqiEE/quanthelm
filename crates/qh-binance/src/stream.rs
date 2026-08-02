use std::time::Duration;

use chrono::{DateTime, Utc};
use futures_util::{SinkExt, StreamExt};
use qh_domain::{Price, Symbol};
use qh_market_data::{Interval, Kline, RawEvent};
use rust_decimal::Decimal;
use serde_json::Value;
use tokio::{
    sync::{mpsc, watch},
    time::{Instant, sleep},
};
use tokio_tungstenite::{connect_async, tungstenite::Message};

use crate::{BinanceError, WebSocketCategory, combined_stream_url};

/// Rotate before Binance's documented 24-hour connection limit.
pub const DEFAULT_MAX_SESSION_AGE: Duration = Duration::from_secs(23 * 60 * 60 + 50 * 60);

/// Observable WebSocket connection state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StreamLifecycle {
    /// A connection attempt is starting.
    Connecting,
    /// A connection is active.
    Connected,
    /// The session is being proactively replaced.
    Rotating,
    /// A connection ended and will be retried.
    Disconnected {
        /// Sanitized reason.
        reason: String,
    },
}

/// Output produced by the kline stream supervisor.
#[derive(Debug, Clone)]
pub enum KlineStreamItem {
    /// Connection lifecycle transition.
    Lifecycle(StreamLifecycle),
    /// Exact text payload with integrity hash.
    Raw(RawEvent),
    /// A newly closed normalized candle.
    ClosedKline(Kline),
    /// A malformed stream payload that was not accepted.
    ProtocolError {
        /// Sanitized parse error.
        message: String,
    },
}

/// Parsed result from one Binance text frame.
#[derive(Debug, Clone)]
pub struct ParsedKlineStreamMessage {
    /// Auditable exact frame text.
    pub raw_event: RawEvent,
    /// Present only when Binance marks the candle closed.
    pub closed_kline: Option<Kline>,
}

/// Deterministic exponential reconnect delay.
#[derive(Debug, Clone)]
pub struct ReconnectBackoff {
    initial: Duration,
    maximum: Duration,
    next: Duration,
}

impl ReconnectBackoff {
    /// Creates a bounded exponential sequence.
    #[must_use]
    pub fn new(initial: Duration, maximum: Duration) -> Self {
        let initial = initial.max(Duration::from_millis(1));
        let maximum = maximum.max(initial);
        Self {
            initial,
            maximum,
            next: initial,
        }
    }

    /// Returns the current delay and advances the sequence.
    pub fn next_delay(&mut self) -> Duration {
        let current = self.next;
        self.next = self.next.saturating_mul(2).min(self.maximum);
        current
    }

    /// Resets after a successful connection.
    pub fn reset(&mut self) {
        self.next = self.initial;
    }
}

impl Default for ReconnectBackoff {
    fn default() -> Self {
        Self::new(Duration::from_secs(1), Duration::from_secs(60))
    }
}

/// Supervised read-only Binance combined kline connection.
#[derive(Debug, Clone)]
pub struct KlineStreamSupervisor {
    streams: Vec<String>,
    max_session_age: Duration,
}

impl KlineStreamSupervisor {
    /// Creates a supervisor for one or more symbol/interval pairs.
    pub fn new(pairs: &[(Symbol, Interval)]) -> Result<Self, BinanceError> {
        if pairs.is_empty() {
            return Err(BinanceError::InvalidRequest(
                "at least one kline stream is required".to_owned(),
            ));
        }
        let streams = pairs
            .iter()
            .map(|(symbol, interval)| {
                format!(
                    "{}@kline_{}",
                    symbol.as_str().to_ascii_lowercase(),
                    interval.as_binance()
                )
            })
            .collect();
        Ok(Self {
            streams,
            max_session_age: DEFAULT_MAX_SESSION_AGE,
        })
    }

    /// Overrides rotation age for deterministic tests or controlled environments.
    #[must_use]
    pub fn with_max_session_age(mut self, age: Duration) -> Self {
        self.max_session_age = age.max(Duration::from_secs(1));
        self
    }

    /// Runs until shutdown is requested or the consumer disappears.
    pub async fn run(
        self,
        output: mpsc::Sender<KlineStreamItem>,
        mut shutdown: watch::Receiver<bool>,
    ) -> Result<(), BinanceError> {
        let url = combined_stream_url(WebSocketCategory::Market, &self.streams)?;
        let mut backoff = ReconnectBackoff::default();

        loop {
            if *shutdown.borrow() {
                return Ok(());
            }
            if !send_item(
                &output,
                KlineStreamItem::Lifecycle(StreamLifecycle::Connecting),
            )
            .await
            {
                return Ok(());
            }

            match connect_async(url.as_str()).await {
                Ok((socket, _response)) => {
                    backoff.reset();
                    if !send_item(
                        &output,
                        KlineStreamItem::Lifecycle(StreamLifecycle::Connected),
                    )
                    .await
                    {
                        return Ok(());
                    }

                    let (mut writer, mut reader) = socket.split();
                    let rotation = sleep_until_age(self.max_session_age);
                    tokio::pin!(rotation);
                    let mut reconnect_reason = "stream ended".to_owned();

                    loop {
                        tokio::select! {
                            changed = shutdown.changed() => {
                                if changed.is_err() || *shutdown.borrow() {
                                    let _ = writer.close().await;
                                    return Ok(());
                                }
                            }
                            () = &mut rotation => {
                                let _ = send_item(
                                    &output,
                                    KlineStreamItem::Lifecycle(StreamLifecycle::Rotating),
                                ).await;
                                reconnect_reason = "proactive session rotation".to_owned();
                                let _ = writer.close().await;
                                break;
                            }
                            incoming = reader.next() => {
                                match incoming {
                                    Some(Ok(Message::Text(text))) => {
                                        match parse_kline_stream_message(text.as_str(), Utc::now()) {
                                            Ok(parsed) => {
                                                if !send_item(&output, KlineStreamItem::Raw(parsed.raw_event)).await {
                                                    return Ok(());
                                                }
                                                if let Some(kline) = parsed.closed_kline
                                                    && !send_item(&output, KlineStreamItem::ClosedKline(kline)).await
                                                {
                                                    return Ok(());
                                                }
                                            }
                                            Err(error) => {
                                                if !send_item(
                                                    &output,
                                                    KlineStreamItem::ProtocolError {
                                                        message: error.to_string(),
                                                    },
                                                ).await {
                                                    return Ok(());
                                                }
                                            }
                                        }
                                    }
                                    Some(Ok(Message::Ping(payload))) => {
                                        if let Err(error) = writer.send(Message::Pong(payload)).await {
                                            reconnect_reason = format!("failed to send pong: {error}");
                                            break;
                                        }
                                    }
                                    Some(Ok(Message::Pong(_))) => {}
                                    Some(Ok(Message::Close(frame))) => {
                                        reconnect_reason = format!("server closed stream: {frame:?}");
                                        break;
                                    }
                                    Some(Ok(Message::Binary(_))) => {
                                        if !send_item(
                                            &output,
                                            KlineStreamItem::ProtocolError {
                                                message: "unexpected binary Binance market-data frame".to_owned(),
                                            },
                                        ).await {
                                            return Ok(());
                                        }
                                    }
                                    Some(Ok(_)) => {}
                                    Some(Err(error)) => {
                                        reconnect_reason = format!("WebSocket transport error: {error}");
                                        break;
                                    }
                                    None => break,
                                }
                            }
                        }
                    }

                    if !send_item(
                        &output,
                        KlineStreamItem::Lifecycle(StreamLifecycle::Disconnected {
                            reason: reconnect_reason,
                        }),
                    )
                    .await
                    {
                        return Ok(());
                    }
                }
                Err(error) => {
                    if !send_item(
                        &output,
                        KlineStreamItem::Lifecycle(StreamLifecycle::Disconnected {
                            reason: format!("WebSocket connect error: {error}"),
                        }),
                    )
                    .await
                    {
                        return Ok(());
                    }
                }
            }

            let delay = backoff.next_delay();
            tokio::select! {
                () = sleep(delay) => {}
                changed = shutdown.changed() => {
                    if changed.is_err() || *shutdown.borrow() {
                        return Ok(());
                    }
                }
            }
        }
    }
}

/// Parses a raw or combined Binance kline stream text frame.
pub fn parse_kline_stream_message(
    payload: &str,
    received_at: DateTime<Utc>,
) -> Result<ParsedKlineStreamMessage, BinanceError> {
    let root: Value = serde_json::from_str(payload)
        .map_err(|error| BinanceError::InvalidResponse(error.to_string()))?;
    let (channel, data) = match (root.get("stream"), root.get("data")) {
        (Some(stream), Some(data)) => (
            stream
                .as_str()
                .ok_or_else(|| BinanceError::InvalidResponse("stream must be text".to_owned()))?
                .to_owned(),
            data,
        ),
        _ => ("raw-kline-stream".to_owned(), &root),
    };

    let event_type = text_field(data, "e", "stream.e")?;
    if event_type != "kline" {
        return Err(BinanceError::InvalidResponse(format!(
            "unexpected stream event type: {event_type}"
        )));
    }
    let event_time = Kline::timestamp(i64_field(data, "E", "stream.E")?)?;
    let kline = data
        .get("k")
        .ok_or_else(|| BinanceError::InvalidResponse("stream.k is missing".to_owned()))?;
    let symbol = Symbol::new(text_field(kline, "s", "stream.k.s")?)?;
    let interval = text_field(kline, "i", "stream.k.i")?.parse()?;
    let is_closed = bool_field(kline, "x", "stream.k.x")?;

    let normalized = Kline {
        symbol,
        interval,
        open_time: Kline::timestamp(i64_field(kline, "t", "stream.k.t")?)?,
        close_time: Kline::timestamp(i64_field(kline, "T", "stream.k.T")?)?,
        open: Price::new(decimal_field(kline, "o", "stream.k.o")?)?,
        high: Price::new(decimal_field(kline, "h", "stream.k.h")?)?,
        low: Price::new(decimal_field(kline, "l", "stream.k.l")?)?,
        close: Price::new(decimal_field(kline, "c", "stream.k.c")?)?,
        volume: decimal_field(kline, "v", "stream.k.v")?,
        quote_volume: decimal_field(kline, "q", "stream.k.q")?,
        trade_count: u64_field(kline, "n", "stream.k.n")?,
        taker_buy_volume: decimal_field(kline, "V", "stream.k.V")?,
        taker_buy_quote_volume: decimal_field(kline, "Q", "stream.k.Q")?,
        is_closed,
    };

    Ok(ParsedKlineStreamMessage {
        raw_event: RawEvent::new(
            "binance-usdm",
            channel,
            Some(event_time),
            received_at,
            payload,
        ),
        closed_kline: is_closed.then_some(normalized),
    })
}

fn sleep_until_age(age: Duration) -> tokio::time::Sleep {
    tokio::time::sleep_until(Instant::now() + age)
}

async fn send_item(output: &mpsc::Sender<KlineStreamItem>, item: KlineStreamItem) -> bool {
    output.send(item).await.is_ok()
}

fn text_field<'a>(value: &'a Value, key: &str, field: &str) -> Result<&'a str, BinanceError> {
    value
        .get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| BinanceError::InvalidResponse(format!("{field} must be text")))
}

fn i64_field(value: &Value, key: &str, field: &str) -> Result<i64, BinanceError> {
    value
        .get(key)
        .and_then(Value::as_i64)
        .ok_or_else(|| BinanceError::InvalidResponse(format!("{field} must be an integer")))
}

fn u64_field(value: &Value, key: &str, field: &str) -> Result<u64, BinanceError> {
    value
        .get(key)
        .and_then(Value::as_u64)
        .ok_or_else(|| BinanceError::InvalidResponse(format!("{field} must be an integer")))
}

fn bool_field(value: &Value, key: &str, field: &str) -> Result<bool, BinanceError> {
    value
        .get(key)
        .and_then(Value::as_bool)
        .ok_or_else(|| BinanceError::InvalidResponse(format!("{field} must be boolean")))
}

fn decimal_field(value: &Value, key: &str, field: &str) -> Result<Decimal, BinanceError> {
    let raw = text_field(value, key, field)?;
    raw.parse().map_err(|_| {
        BinanceError::InvalidResponse(format!("{field} contains invalid decimal text"))
    })
}

#[cfg(test)]
mod tests {
    use chrono::TimeZone;

    use super::*;

    const CLOSED_KLINE: &str = r#"{
      "stream":"btcusdt@kline_15m",
      "data":{
        "e":"kline","E":1720000900001,"s":"BTCUSDT",
        "k":{"t":1720000000000,"T":1720000899999,"s":"BTCUSDT","i":"15m",
             "o":"100.0","c":"105.0","h":"110.0","l":"90.0","v":"12.5",
             "n":42,"x":true,"q":"1300.0","V":"7.5","Q":"780.0"}
      }
    }"#;

    #[test]
    fn parses_closed_combined_kline() {
        let parsed = parse_kline_stream_message(
            CLOSED_KLINE,
            Utc.timestamp_millis_opt(1_720_000_900_100)
                .single()
                .expect("valid timestamp"),
        )
        .expect("parse frame");
        let candle = parsed.closed_kline.expect("closed candle");
        assert_eq!(candle.symbol.as_str(), "BTCUSDT");
        assert_eq!(candle.interval, Interval::M15);
        assert_eq!(candle.trade_count, 42);
        assert!(parsed.raw_event.verify_hash());
    }

    #[test]
    fn ignores_open_candle_as_normalized_output() {
        let open = CLOSED_KLINE.replace("\"x\":true", "\"x\":false");
        let parsed = parse_kline_stream_message(&open, Utc::now()).expect("parse frame");
        assert!(parsed.closed_kline.is_none());
    }

    #[test]
    fn reconnect_backoff_is_bounded_and_resettable() {
        let mut backoff = ReconnectBackoff::new(Duration::from_secs(1), Duration::from_secs(4));
        assert_eq!(backoff.next_delay(), Duration::from_secs(1));
        assert_eq!(backoff.next_delay(), Duration::from_secs(2));
        assert_eq!(backoff.next_delay(), Duration::from_secs(4));
        assert_eq!(backoff.next_delay(), Duration::from_secs(4));
        backoff.reset();
        assert_eq!(backoff.next_delay(), Duration::from_secs(1));
    }
}
