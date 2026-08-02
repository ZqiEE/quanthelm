use std::str::FromStr;

use chrono::{DateTime, Utc};
use qh_domain::{Price, Symbol};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::BinanceError;

/// Current fair-price and funding state for one contract.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MarkPriceSnapshot {
    /// Contract symbol.
    pub symbol: Symbol,
    /// Exchange mark price.
    pub mark_price: Price,
    /// Underlying index price.
    pub index_price: Price,
    /// Estimated settlement price when supplied and positive.
    pub estimated_settle_price: Option<Price>,
    /// Most recently published funding rate.
    pub last_funding_rate: Decimal,
    /// Interest-rate component.
    pub interest_rate: Decimal,
    /// Next scheduled funding timestamp.
    pub next_funding_time: DateTime<Utc>,
    /// Exchange timestamp for this snapshot.
    pub exchange_time: DateTime<Utc>,
    /// Local receive timestamp.
    pub received_at: DateTime<Utc>,
}

/// Best bid and ask for one contract.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BookTickerSnapshot {
    /// Contract symbol.
    pub symbol: Symbol,
    /// Best bid price.
    pub bid_price: Price,
    /// Best bid quantity.
    pub bid_quantity: Decimal,
    /// Best ask price.
    pub ask_price: Price,
    /// Best ask quantity.
    pub ask_quantity: Decimal,
    /// Optional exchange update identifier.
    pub update_id: Option<u64>,
    /// Optional exchange timestamp.
    pub exchange_time: Option<DateTime<Utc>>,
    /// Local receive timestamp.
    pub received_at: DateTime<Utc>,
}

/// One historical funding-rate observation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FundingRateRecord {
    /// Contract symbol.
    pub symbol: Symbol,
    /// Funding rate, which may be negative.
    pub funding_rate: Decimal,
    /// Funding timestamp.
    pub funding_time: DateTime<Utc>,
    /// Mark price associated with the funding event when supplied and positive.
    pub mark_price: Option<Price>,
}

/// Current outstanding contract quantity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OpenInterestSnapshot {
    /// Contract symbol.
    pub symbol: Symbol,
    /// Outstanding contract quantity.
    pub open_interest: Decimal,
    /// Exchange timestamp.
    pub exchange_time: DateTime<Utc>,
    /// Local receive timestamp.
    pub received_at: DateTime<Utc>,
}

/// Rolling 24-hour market statistics.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RollingTicker24h {
    /// Contract symbol.
    pub symbol: Symbol,
    /// Absolute price change.
    pub price_change: Decimal,
    /// Percentage price change.
    pub price_change_percent: Decimal,
    /// Volume-weighted average price.
    pub weighted_average_price: Price,
    /// Last traded price.
    pub last_price: Price,
    /// Last traded quantity.
    pub last_quantity: Decimal,
    /// Window open price.
    pub open_price: Price,
    /// Window high price.
    pub high_price: Price,
    /// Window low price.
    pub low_price: Price,
    /// Base-asset volume.
    pub volume: Decimal,
    /// Quote-asset volume.
    pub quote_volume: Decimal,
    /// Window open timestamp.
    pub open_time: DateTime<Utc>,
    /// Window close timestamp.
    pub close_time: DateTime<Utc>,
    /// Number of trades in the rolling window.
    pub trade_count: u64,
    /// Local receive timestamp.
    pub received_at: DateTime<Utc>,
}

/// One coherent public-market context snapshot for research and risk filtering.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MarketContextSnapshot {
    /// Contract symbol shared by all components.
    pub symbol: Symbol,
    /// Fair-price and funding state.
    pub mark_price: MarkPriceSnapshot,
    /// Best bid and ask.
    pub book_ticker: BookTickerSnapshot,
    /// Current open interest.
    pub open_interest: OpenInterestSnapshot,
    /// Rolling 24-hour statistics.
    pub rolling_24h: RollingTicker24h,
    /// Recent funding observations in ascending API order.
    pub recent_funding: Vec<FundingRateRecord>,
    /// Time after all component requests completed.
    pub completed_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawMarkPrice {
    symbol: String,
    mark_price: String,
    index_price: String,
    estimated_settle_price: Option<String>,
    last_funding_rate: String,
    interest_rate: String,
    next_funding_time: i64,
    time: i64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawBookTicker {
    symbol: String,
    bid_price: String,
    bid_qty: String,
    ask_price: String,
    ask_qty: String,
    time: Option<i64>,
    last_update_id: Option<u64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawFundingRate {
    symbol: String,
    funding_rate: String,
    funding_time: i64,
    mark_price: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawOpenInterest {
    symbol: String,
    open_interest: String,
    time: i64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawRollingTicker24h {
    symbol: String,
    price_change: String,
    price_change_percent: String,
    weighted_avg_price: String,
    last_price: String,
    last_qty: String,
    open_price: String,
    high_price: String,
    low_price: String,
    volume: String,
    quote_volume: String,
    open_time: i64,
    close_time: i64,
    count: u64,
}

pub(crate) fn parse_mark_price(
    payload: &str,
    received_at: DateTime<Utc>,
) -> Result<MarkPriceSnapshot, BinanceError> {
    let raw: RawMarkPrice = decode(payload)?;
    Ok(MarkPriceSnapshot {
        symbol: Symbol::new(raw.symbol)?,
        mark_price: required_price(&raw.mark_price, "premiumIndex.markPrice")?,
        index_price: required_price(&raw.index_price, "premiumIndex.indexPrice")?,
        estimated_settle_price: optional_positive_price(
            raw.estimated_settle_price.as_deref(),
            "premiumIndex.estimatedSettlePrice",
        )?,
        last_funding_rate: decimal(&raw.last_funding_rate, "premiumIndex.lastFundingRate")?,
        interest_rate: decimal(&raw.interest_rate, "premiumIndex.interestRate")?,
        next_funding_time: timestamp(raw.next_funding_time)?,
        exchange_time: timestamp(raw.time)?,
        received_at,
    })
}

pub(crate) fn parse_book_ticker(
    payload: &str,
    received_at: DateTime<Utc>,
) -> Result<BookTickerSnapshot, BinanceError> {
    let raw: RawBookTicker = decode(payload)?;
    Ok(BookTickerSnapshot {
        symbol: Symbol::new(raw.symbol)?,
        bid_price: required_price(&raw.bid_price, "bookTicker.bidPrice")?,
        bid_quantity: decimal(&raw.bid_qty, "bookTicker.bidQty")?,
        ask_price: required_price(&raw.ask_price, "bookTicker.askPrice")?,
        ask_quantity: decimal(&raw.ask_qty, "bookTicker.askQty")?,
        update_id: raw.last_update_id,
        exchange_time: raw.time.map(timestamp).transpose()?,
        received_at,
    })
}

pub(crate) fn parse_funding_rates(payload: &str) -> Result<Vec<FundingRateRecord>, BinanceError> {
    let rows: Vec<RawFundingRate> = decode(payload)?;
    rows.into_iter()
        .map(|raw| {
            Ok(FundingRateRecord {
                symbol: Symbol::new(raw.symbol)?,
                funding_rate: decimal(&raw.funding_rate, "fundingRate.fundingRate")?,
                funding_time: timestamp(raw.funding_time)?,
                mark_price: optional_positive_price(
                    raw.mark_price.as_deref(),
                    "fundingRate.markPrice",
                )?,
            })
        })
        .collect()
}

pub(crate) fn parse_open_interest(
    payload: &str,
    received_at: DateTime<Utc>,
) -> Result<OpenInterestSnapshot, BinanceError> {
    let raw: RawOpenInterest = decode(payload)?;
    Ok(OpenInterestSnapshot {
        symbol: Symbol::new(raw.symbol)?,
        open_interest: decimal(&raw.open_interest, "openInterest.openInterest")?,
        exchange_time: timestamp(raw.time)?,
        received_at,
    })
}

pub(crate) fn parse_rolling_ticker_24h(
    payload: &str,
    received_at: DateTime<Utc>,
) -> Result<RollingTicker24h, BinanceError> {
    let raw: RawRollingTicker24h = decode(payload)?;
    Ok(RollingTicker24h {
        symbol: Symbol::new(raw.symbol)?,
        price_change: decimal(&raw.price_change, "ticker24h.priceChange")?,
        price_change_percent: decimal(
            &raw.price_change_percent,
            "ticker24h.priceChangePercent",
        )?,
        weighted_average_price: required_price(
            &raw.weighted_avg_price,
            "ticker24h.weightedAvgPrice",
        )?,
        last_price: required_price(&raw.last_price, "ticker24h.lastPrice")?,
        last_quantity: decimal(&raw.last_qty, "ticker24h.lastQty")?,
        open_price: required_price(&raw.open_price, "ticker24h.openPrice")?,
        high_price: required_price(&raw.high_price, "ticker24h.highPrice")?,
        low_price: required_price(&raw.low_price, "ticker24h.lowPrice")?,
        volume: decimal(&raw.volume, "ticker24h.volume")?,
        quote_volume: decimal(&raw.quote_volume, "ticker24h.quoteVolume")?,
        open_time: timestamp(raw.open_time)?,
        close_time: timestamp(raw.close_time)?,
        trade_count: raw.count,
        received_at,
    })
}

fn decode<T>(payload: &str) -> Result<T, BinanceError>
where
    T: for<'de> Deserialize<'de>,
{
    serde_json::from_str(payload)
        .map_err(|error| BinanceError::InvalidResponse(error.to_string()))
}

fn decimal(raw: &str, field: &'static str) -> Result<Decimal, BinanceError> {
    Decimal::from_str(raw).map_err(|_| BinanceError::InvalidDecimal {
        field,
        value: raw.to_owned(),
    })
}

fn required_price(raw: &str, field: &'static str) -> Result<Price, BinanceError> {
    Price::new(decimal(raw, field)?).map_err(BinanceError::from)
}

fn optional_positive_price(
    raw: Option<&str>,
    field: &'static str,
) -> Result<Option<Price>, BinanceError> {
    let Some(raw) = raw else {
        return Ok(None);
    };
    let value = decimal(raw, field)?;
    if value <= Decimal::ZERO {
        Ok(None)
    } else {
        Ok(Some(Price::new(value)?))
    }
}

fn timestamp(milliseconds: i64) -> Result<DateTime<Utc>, BinanceError> {
    DateTime::from_timestamp_millis(milliseconds)
        .ok_or_else(|| BinanceError::InvalidResponse(format!("invalid timestamp: {milliseconds}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_market_context_components() {
        let received_at = Utc::now();
        let mark = parse_mark_price(
            r#"{"symbol":"BTCUSDT","markPrice":"100.5","indexPrice":"100.4","estimatedSettlePrice":"0.0","lastFundingRate":"-0.0001","interestRate":"0.0001","nextFundingTime":1720000000000,"time":1719999000000}"#,
            received_at,
        )
        .expect("mark price");
        assert_eq!(mark.symbol.as_str(), "BTCUSDT");
        assert!(mark.estimated_settle_price.is_none());
        assert!(mark.last_funding_rate < Decimal::ZERO);

        let book = parse_book_ticker(
            r#"{"symbol":"BTCUSDT","bidPrice":"100.4","bidQty":"2.0","askPrice":"100.5","askQty":"3.0","time":1719999000001,"lastUpdateId":42}"#,
            received_at,
        )
        .expect("book ticker");
        assert_eq!(book.update_id, Some(42));

        let funding = parse_funding_rates(
            r#"[{"symbol":"BTCUSDT","fundingRate":"0.0002","fundingTime":1719990000000,"markPrice":"100.0"}]"#,
        )
        .expect("funding rates");
        assert_eq!(funding.len(), 1);

        let open_interest = parse_open_interest(
            r#"{"openInterest":"12345.6","symbol":"BTCUSDT","time":1719999000002}"#,
            received_at,
        )
        .expect("open interest");
        assert_eq!(open_interest.open_interest, Decimal::new(123456, 1));

        let ticker = parse_rolling_ticker_24h(
            r#"{"symbol":"BTCUSDT","priceChange":"5.0","priceChangePercent":"5.0","weightedAvgPrice":"98.0","lastPrice":"100.0","lastQty":"0.5","openPrice":"95.0","highPrice":"101.0","lowPrice":"94.0","volume":"1000.0","quoteVolume":"98000.0","openTime":1719912600000,"closeTime":1719999000000,"count":10000}"#,
            received_at,
        )
        .expect("24h ticker");
        assert_eq!(ticker.trade_count, 10_000);
    }
}
