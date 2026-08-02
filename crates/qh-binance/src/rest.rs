use std::time::Duration;

use chrono::Utc;
use qh_market_data::Kline;
use reqwest::Client;
use tracing::debug;

use crate::{
    BinanceError,
    ExchangeInfo,
    KlineRequest,
    parse_exchange_info,
    parse_klines,
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
