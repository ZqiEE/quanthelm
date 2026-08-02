use qh_domain::DomainError;
use qh_market_data::MarketDataError;
use reqwest::StatusCode;
use thiserror::Error;

/// Binance adapter failure.
#[derive(Debug, Error)]
pub enum BinanceError {
    /// HTTP client construction or request failure.
    #[error("Binance HTTP error: {0}")]
    Http(#[from] reqwest::Error),
    /// Non-success response with a sanitized body.
    #[error("Binance returned HTTP {status}: {body}")]
    HttpStatus {
        /// HTTP status.
        status: StatusCode,
        /// Bounded response body.
        body: String,
    },
    /// Required response field was absent or invalid.
    #[error("invalid Binance response: {0}")]
    InvalidResponse(String),
    /// Decimal text could not be parsed.
    #[error("invalid decimal for {field}: {value}")]
    InvalidDecimal {
        /// Field name.
        field: &'static str,
        /// Invalid text.
        value: String,
    },
    /// Domain validation failed.
    #[error(transparent)]
    Domain(#[from] DomainError),
    /// Market-data validation failed.
    #[error(transparent)]
    MarketData(#[from] MarketDataError),
    /// Request parameters were invalid.
    #[error("invalid Binance request: {0}")]
    InvalidRequest(String),
}
