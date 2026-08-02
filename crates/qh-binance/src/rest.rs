use std::time::Duration;

use chrono::{DateTime, Utc};
use qh_domain::Symbol;
use qh_market_data::{Interval, Kline};
use reqwest::Client;
use tracing::debug;

use crate::{
    BinanceError, BookTickerSnapshot, ExchangeInfo, FundingRateRecord, KlineRequest,
    MarkPriceSnapshot, MarketContextSnapshot, OpenInterestSnapshot, RollingTicker24h,
    context::{
        parse_book_ticker, parse_funding_rates, parse_mark_price, parse_open_interest,
        parse_rolling_ticker_24h,
    },
    parse_exchange_info, parse_klines,
};

/// Binance USDⓈ-M production REST endpoint.
pub const MAINNET_REST_BASE: &str = "https://fapi.binance.com";

/// Read-only public REST client.
#[derive(Debug, Clone)]
pub struct BinancePublicClient {
    http: Client,
    rest_base: String,
}

impl BinancePublicClient {
    /// Production client with a bounded timeout.
    pub fn mainnet() -> Result<Self, BinanceError> {
        Self::with_rest_base(MAINNET_REST_BASE)
    }

    /// Client with an override base URL, useful for tests and controlled proxies.
    pub fn with_rest_base(rest_base: impl Into<String>) -> Result<Self, BinanceError> {
        let http = Client::builder()
            .timeout(Duration::from_secs(10))
            .user_agent(concat!("quanthelm/", env!("CARGO_PKG_VERSION")))
            .build()?;
        Ok(Self {
            http,
            rest_base: rest_base.into().trim_end_matches('/').to_owned(),
        })
    }

    /// Tests public REST connectivity.
    pub async fn ping(&self) -> Result<(), BinanceError> {
        let _ = self.get_text("/fapi/v1/ping", &[]).await?;
        Ok(())
    }

    /// Fetches and parses current exchange metadata.
    pub async fn exchange_info(&self) -> Result<ExchangeInfo, BinanceError> {
        let received_at = Utc::now();
        let payload = self.get_text("/fapi/v1/exchangeInfo", &[]).await?;
        parse_exchange_info(&payload, received_at)
    }

    /// Fetches the current mark price and funding state for one contract.
    pub async fn mark_price(&self, symbol: &Symbol) -> Result<MarkPriceSnapshot, BinanceError> {
        let received_at = Utc::now();
        let query = [("symbol".to_owned(), symbol.to_string())];
        let payload = self.get_text("/fapi/v1/premiumIndex", &query).await?;
        let snapshot = parse_mark_price(&payload, received_at)?;
        ensure_symbol(symbol, &snapshot.symbol, "premiumIndex")?;
        Ok(snapshot)
    }

    /// Fetches the current best bid and ask for one contract.
    pub async fn book_ticker(&self, symbol: &Symbol) -> Result<BookTickerSnapshot, BinanceError> {
        let received_at = Utc::now();
        let query = [("symbol".to_owned(), symbol.to_string())];
        let payload = self.get_text("/fapi/v1/ticker/bookTicker", &query).await?;
        let snapshot = parse_book_ticker(&payload, received_at)?;
        ensure_symbol(symbol, &snapshot.symbol, "bookTicker")?;
        Ok(snapshot)
    }

    /// Fetches recent funding-rate records for one contract.
    pub async fn funding_rates(
        &self,
        symbol: &Symbol,
        limit: u16,
    ) -> Result<Vec<FundingRateRecord>, BinanceError> {
        if !(1..=1_000).contains(&limit) {
            return Err(BinanceError::InvalidRequest(
                "funding-rate limit must be between 1 and 1000".to_owned(),
            ));
        }
        let query = [
            ("symbol".to_owned(), symbol.to_string()),
            ("limit".to_owned(), limit.to_string()),
        ];
        let payload = self.get_text("/fapi/v1/fundingRate", &query).await?;
        let records = parse_funding_rates(&payload)?;
        for record in &records {
            ensure_symbol(symbol, &record.symbol, "fundingRate")?;
        }
        Ok(records)
    }

    /// Fetches current open interest for one contract.
    pub async fn open_interest(
        &self,
        symbol: &Symbol,
    ) -> Result<OpenInterestSnapshot, BinanceError> {
        let received_at = Utc::now();
        let query = [("symbol".to_owned(), symbol.to_string())];
        let payload = self.get_text("/fapi/v1/openInterest", &query).await?;
        let snapshot = parse_open_interest(&payload, received_at)?;
        ensure_symbol(symbol, &snapshot.symbol, "openInterest")?;
        Ok(snapshot)
    }

    /// Fetches rolling 24-hour statistics for one contract.
    pub async fn rolling_ticker_24h(
        &self,
        symbol: &Symbol,
    ) -> Result<RollingTicker24h, BinanceError> {
        let received_at = Utc::now();
        let query = [("symbol".to_owned(), symbol.to_string())];
        let payload = self.get_text("/fapi/v1/ticker/24hr", &query).await?;
        let snapshot = parse_rolling_ticker_24h(&payload, received_at)?;
        ensure_symbol(symbol, &snapshot.symbol, "ticker24h")?;
        Ok(snapshot)
    }

    /// Fetches a coherent public-market context snapshot without credentials.
    pub async fn market_context(
        &self,
        symbol: &Symbol,
        funding_limit: u16,
    ) -> Result<MarketContextSnapshot, BinanceError> {
        let (mark_price, book_ticker, open_interest, rolling_24h, recent_funding) = tokio::try_join!(
            self.mark_price(symbol),
            self.book_ticker(symbol),
            self.open_interest(symbol),
            self.rolling_ticker_24h(symbol),
            self.funding_rates(symbol, funding_limit),
        )?;
        Ok(MarketContextSnapshot {
            symbol: symbol.clone(),
            mark_price,
            book_ticker,
            open_interest,
            rolling_24h,
            recent_funding,
            completed_at: Utc::now(),
        })
    }

    /// Fetches normalized candlesticks.
    pub async fn klines(&self, request: &KlineRequest) -> Result<Vec<Kline>, BinanceError> {
        request.validate()?;
        let mut query = vec![
            ("symbol".to_owned(), request.symbol.to_string()),
            ("interval".to_owned(), request.interval.to_string()),
            ("limit".to_owned(), request.limit.to_string()),
        ];
        if let Some(value) = request.start_time_ms {
            query.push(("startTime".to_owned(), value.to_string()));
        }
        if let Some(value) = request.end_time_ms {
            query.push(("endTime".to_owned(), value.to_string()));
        }

        let received_at = Utc::now();
        let payload = self.get_text("/fapi/v1/klines", &query).await?;
        parse_klines(
            &payload,
            request.symbol.clone(),
            request.interval,
            received_at,
        )
    }

    /// Fetches every candle in a missing half-open interval and proves continuity.
    pub async fn backfill_klines(
        &self,
        symbol: &Symbol,
        interval: Interval,
        start_open_time: DateTime<Utc>,
        end_open_time_exclusive: DateTime<Utc>,
    ) -> Result<Vec<Kline>, BinanceError> {
        let expected = backfill_candle_count(start_open_time, end_open_time_exclusive, interval)?;
        let mut cursor = start_open_time;
        let mut recovered = Vec::with_capacity(expected);

        while cursor < end_open_time_exclusive {
            let remaining = usize::try_from(
                (end_open_time_exclusive - cursor).num_milliseconds() / interval.milliseconds(),
            )
            .map_err(|_| BinanceError::InvalidRequest("backfill range is too large".to_owned()))?;
            let limit = u16::try_from(remaining.min(1_500)).map_err(|_| {
                BinanceError::InvalidRequest("invalid backfill page size".to_owned())
            })?;
            let request = KlineRequest {
                symbol: symbol.clone(),
                interval,
                start_time_ms: Some(cursor.timestamp_millis()),
                end_time_ms: Some(end_open_time_exclusive.timestamp_millis() - 1),
                limit,
            };
            let page = self.klines(&request).await?;
            if page.is_empty() {
                return Err(BinanceError::InvalidResponse(format!(
                    "backfill returned no candle for expected open time {cursor}"
                )));
            }

            let previous_cursor = cursor;
            for candle in page {
                if candle.open_time >= end_open_time_exclusive {
                    break;
                }
                if candle.open_time != cursor {
                    return Err(BinanceError::InvalidResponse(format!(
                        "backfill continuity failure: expected {cursor}, received {}",
                        candle.open_time
                    )));
                }
                cursor += interval.duration();
                recovered.push(candle);
            }
            if cursor == previous_cursor {
                return Err(BinanceError::InvalidResponse(
                    "backfill page made no progress".to_owned(),
                ));
            }
        }

        if recovered.len() != expected {
            return Err(BinanceError::InvalidResponse(format!(
                "backfill expected {expected} candles but recovered {}",
                recovered.len()
            )));
        }
        Ok(recovered)
    }

    async fn get_text(
        &self,
        path: &str,
        query: &[(String, String)],
    ) -> Result<String, BinanceError> {
        let url = format!("{}{}", self.rest_base, path);
        debug!(%url, "sending read-only Binance request");
        let response = self.http.get(url).query(query).send().await?;
        let status = response.status();
        let body = response.text().await?;
        if !status.is_success() {
            return Err(BinanceError::HttpStatus {
                status,
                body: bounded_body(body),
            });
        }
        Ok(body)
    }
}

fn ensure_symbol(
    requested: &Symbol,
    received: &Symbol,
    endpoint: &str,
) -> Result<(), BinanceError> {
    if requested == received {
        Ok(())
    } else {
        Err(BinanceError::InvalidResponse(format!(
            "{endpoint} returned symbol {received} for requested {requested}"
        )))
    }
}

fn backfill_candle_count(
    start: DateTime<Utc>,
    end_exclusive: DateTime<Utc>,
    interval: Interval,
) -> Result<usize, BinanceError> {
    let span = (end_exclusive - start).num_milliseconds();
    if span <= 0 {
        return Err(BinanceError::InvalidRequest(
            "backfill end must be after start".to_owned(),
        ));
    }
    if span % interval.milliseconds() != 0 {
        return Err(BinanceError::InvalidRequest(
            "backfill range must align to the candle interval".to_owned(),
        ));
    }
    let count = span / interval.milliseconds();
    if count > 100_000 {
        return Err(BinanceError::InvalidRequest(
            "backfill range exceeds 100000 candles".to_owned(),
        ));
    }
    usize::try_from(count)
        .map_err(|_| BinanceError::InvalidRequest("backfill range is too large".to_owned()))
}

fn bounded_body(body: String) -> String {
    const MAX_CHARS: usize = 1_024;
    if body.chars().count() <= MAX_CHARS {
        body
    } else {
        let mut bounded: String = body.chars().take(MAX_CHARS).collect();
        bounded.push('…');
        bounded
    }
}

#[cfg(test)]
mod tests {
    use chrono::TimeZone;

    use super::*;

    #[test]
    fn backfill_range_must_be_aligned() {
        let start = Utc.timestamp_millis_opt(0).single().expect("valid time");
        let aligned = Utc
            .timestamp_millis_opt(Interval::M15.milliseconds() * 2)
            .single()
            .expect("valid time");
        assert_eq!(
            backfill_candle_count(start, aligned, Interval::M15).expect("aligned range"),
            2
        );

        let unaligned = Utc
            .timestamp_millis_opt(Interval::M15.milliseconds() + 1)
            .single()
            .expect("valid time");
        assert!(backfill_candle_count(start, unaligned, Interval::M15).is_err());
    }
}
