//! Exchange-neutral market-data events, hashes, and continuity checks.

use std::{
    collections::HashMap,
    fmt,
    str::FromStr,
};

use chrono::{DateTime, Duration, Utc};
use qh_domain::{DomainError, Price, Symbol};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

/// Market-data validation failure.
#[derive(Debug, Error)]
pub enum MarketDataError {
    /// Unsupported interval text.
    #[error("unsupported interval: {0}")]
    UnsupportedInterval(String),
    /// Timestamp was outside the supported chrono range.
    #[error("invalid unix timestamp in milliseconds: {0}")]
    InvalidTimestamp(i64),
    /// Domain value was invalid.
    #[error(transparent)]
    Domain(#[from] DomainError),
}

/// Supported MVP candlestick intervals.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Interval {
    /// Fifteen minutes.
    #[serde(rename = "15m")]
    M15,
    /// One hour.
    #[serde(rename = "1h")]
    H1,
    /// Four hours.
    #[serde(rename = "4h")]
    H4,
}

impl Interval {
    /// Binance API representation.
    #[must_use]
    pub const fn as_binance(self) -> &'static str {
        match self {
            Self::M15 => "15m",
            Self::H1 => "1h",
            Self::H4 => "4h",
        }
    }

    /// Exact interval length in milliseconds.
    #[must_use]
    pub const fn milliseconds(self) -> i64 {
        match self {
            Self::M15 => 15 * 60 * 1_000,
            Self::H1 => 60 * 60 * 1_000,
            Self::H4 => 4 * 60 * 60 * 1_000,
        }
    }

    /// Exact interval length.
    #[must_use]
    pub fn duration(self) -> Duration {
        Duration::milliseconds(self.milliseconds())
    }
}

impl fmt::Display for Interval {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_binance())
    }
}

impl FromStr for Interval {
    type Err = MarketDataError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim() {
            "15m" => Ok(Self::M15),
            "1h" => Ok(Self::H1),
            "4h" => Ok(Self::H4),
            other => Err(MarketDataError::UnsupportedInterval(other.to_owned())),
        }
    }
}

/// Normalized closed or in-progress candlestick.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Kline {
    /// Contract symbol.
    pub symbol: Symbol,
    /// Candle interval.
    pub interval: Interval,
    /// Inclusive candle open time.
    pub open_time: DateTime<Utc>,
    /// Inclusive exchange close timestamp.
    pub close_time: DateTime<Utc>,
    /// Open price.
    pub open: Price,
    /// High price.
    pub high: Price,
    /// Low price.
    pub low: Price,
    /// Close price.
    pub close: Price,
    /// Base-asset volume. Zero is valid.
    pub volume: Decimal,
    /// Quote-asset volume. Zero is valid.
    pub quote_volume: Decimal,
    /// Trade count.
    pub trade_count: u64,
    /// Taker-buy base volume.
    pub taker_buy_volume: Decimal,
    /// Taker-buy quote volume.
    pub taker_buy_quote_volume: Decimal,
    /// True only when the exchange says the candle is closed.
    pub is_closed: bool,
}

impl Kline {
    /// Converts a millisecond timestamp into UTC.
    pub fn timestamp(milliseconds: i64) -> Result<DateTime<Utc>, MarketDataError> {
        DateTime::from_timestamp_millis(milliseconds)
            .ok_or(MarketDataError::InvalidTimestamp(milliseconds))
    }
}

/// Auditable raw event metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RawEvent {
    /// Schema version for forwards-compatible replay.
    pub schema_version: u16,
    /// Adapter source name.
    pub source: String,
    /// Endpoint or stream identifier.
    pub channel: String,
    /// Optional exchange event time.
    pub exchange_event_time: Option<DateTime<Utc>>,
    /// Local receive time.
    pub received_at: DateTime<Utc>,
    /// SHA-256 of the exact UTF-8 payload bytes.
    pub payload_sha256: String,
    /// Original JSON payload.
    pub payload: String,
}

impl RawEvent {
    /// Builds an auditable event from an exact payload.
    #[must_use]
    pub fn new(
        source: impl Into<String>,
        channel: impl Into<String>,
        exchange_event_time: Option<DateTime<Utc>>,
        received_at: DateTime<Utc>,
        payload: impl Into<String>,
    ) -> Self {
        let payload = payload.into();
        Self {
            schema_version: 1,
            source: source.into(),
            channel: channel.into(),
            exchange_event_time,
            received_at,
            payload_sha256: sha256_hex(payload.as_bytes()),
            payload,
        }
    }

    /// Verifies that stored content still matches its hash.
    #[must_use]
    pub fn verify_hash(&self) -> bool {
        self.payload_sha256 == sha256_hex(self.payload.as_bytes())
    }
}

/// Result of observing one candle in stream order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Continuity {
    /// First candle for a symbol and interval.
    First,
    /// Candle follows the previous candle exactly.
    Continuous,
    /// Exact same open time was already observed.
    Duplicate,
    /// One or more candles are missing.
    Gap {
        /// First missing candle open time.
        expected_open_time: DateTime<Utc>,
        /// Observed candle open time.
        actual_open_time: DateTime<Utc>,
        /// Number of missing candles.
        missing_candles: u64,
    },
    /// Candle is older than the latest accepted candle.
    OutOfOrder {
        /// Latest accepted open time.
        latest_open_time: DateTime<Utc>,
        /// Older observed open time.
        actual_open_time: DateTime<Utc>,
    },
}

/// Stateful continuity detector keyed by symbol and interval.
#[derive(Debug, Default)]
pub struct GapDetector {
    latest: HashMap<(Symbol, Interval), DateTime<Utc>>,
}

impl GapDetector {
    /// Returns the latest accepted open time for a symbol and interval.
    #[must_use]
    pub fn latest_open_time(
        &self,
        symbol: &Symbol,
        interval: Interval,
    ) -> Option<DateTime<Utc>> {
        self.latest.get(&(symbol.clone(), interval)).copied()
    }

    /// Observes a candle and classifies its ordering.
    pub fn observe(&mut self, kline: &Kline) -> Continuity {
        let key = (kline.symbol.clone(), kline.interval);
        let Some(previous) = self.latest.get(&key).copied() else {
            self.latest.insert(key, kline.open_time);
            return Continuity::First;
        };

        if kline.open_time == previous {
            return Continuity::Duplicate;
        }
        if kline.open_time < previous {
            return Continuity::OutOfOrder {
                latest_open_time: previous,
                actual_open_time: kline.open_time,
            };
        }

        let expected = previous + kline.interval.duration();

        if kline.open_time == expected {
            self.latest.insert(key, kline.open_time);
            Continuity::Continuous
        } else if kline.open_time > expected {
            let missing = (kline.open_time - expected).num_milliseconds()
                / kline.interval.milliseconds();
            Continuity::Gap {
                expected_open_time: expected,
                actual_open_time: kline.open_time,
                missing_candles: u64::try_from(missing).unwrap_or(u64::MAX),
            }
        } else {
            Continuity::OutOfOrder {
                latest_open_time: previous,
                actual_open_time: kline.open_time,
            }
        }
    }
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut output = String::with_capacity(digest.len() * 2);
    for byte in digest {
        use std::fmt::Write;
        let _ = write!(output, "{byte:02x}");
    }
    output
}

#[cfg(test)]
mod tests {
    use chrono::TimeZone;

    use super::*;

    fn candle(open_time_ms: i64) -> Kline {
        let price = Price::new(Decimal::from(100)).expect("positive price");
        Kline {
            symbol: Symbol::new("BTCUSDT").expect("valid symbol"),
            interval: Interval::M15,
            open_time: Utc
                .timestamp_millis_opt(open_time_ms)
                .single()
                .expect("valid timestamp"),
            close_time: Utc
                .timestamp_millis_opt(open_time_ms + Interval::M15.milliseconds() - 1)
                .single()
                .expect("valid timestamp"),
            open: price,
            high: price,
            low: price,
            close: price,
            volume: Decimal::ZERO,
            quote_volume: Decimal::ZERO,
            trade_count: 0,
            taker_buy_volume: Decimal::ZERO,
            taker_buy_quote_volume: Decimal::ZERO,
            is_closed: true,
        }
    }

    #[test]
    fn raw_event_hash_detects_changes() {
        let mut event = RawEvent::new("binance", "exchangeInfo", None, Utc::now(), "{}");
        assert!(event.verify_hash());
        event.payload.push(' ');
        assert!(!event.verify_hash());
    }

    #[test]
    fn gap_detector_reports_missing_candle() {
        let mut detector = GapDetector::default();
        assert_eq!(detector.observe(&candle(0)), Continuity::First);
        let result = detector.observe(&candle(Interval::M15.milliseconds() * 2));
        assert!(matches!(
            result,
            Continuity::Gap {
                missing_candles: 1,
                ..
            }
        ));
    }

    #[test]
    fn gap_detector_requires_missing_candles_before_advancing() {
        let mut detector = GapDetector::default();
        let interval = Interval::M15.milliseconds();
        assert_eq!(detector.observe(&candle(0)), Continuity::First);
        assert!(matches!(
            detector.observe(&candle(interval * 2)),
            Continuity::Gap {
                missing_candles: 1,
                ..
            }
        ));
        assert_eq!(detector.observe(&candle(interval)), Continuity::Continuous);
        assert_eq!(
            detector.observe(&candle(interval * 2)),
            Continuity::Continuous
        );
    }

    #[test]
    fn gap_detector_does_not_advance_on_old_data() {
        let mut detector = GapDetector::default();
        let interval = Interval::M15.milliseconds();
        detector.observe(&candle(interval));
        assert!(matches!(
            detector.observe(&candle(0)),
            Continuity::OutOfOrder { .. }
        ));
        assert_eq!(
            detector.latest_open_time(
                &Symbol::new("BTCUSDT").expect("valid symbol"),
                Interval::M15,
            ),
            Some(
                Utc.timestamp_millis_opt(interval)
                    .single()
                    .expect("valid timestamp")
            )
        );
    }
}
