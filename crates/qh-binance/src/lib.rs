//! Read-only Binance USDⓈ-M Futures public market-data adapter.
//!
//! This crate intentionally contains no API-key, signing, account, or order code.

mod error;
mod filters;
mod models;
mod parse;
mod rest;
mod stream;
mod websocket;

pub use error::BinanceError;
pub use filters::{LotSizeFilter, PercentPriceFilter, PriceFilter, SymbolRules};
pub use models::{ContractSpec, ContractStatus, ContractType, ExchangeInfo, KlineRequest};
pub use parse::{parse_exchange_info, parse_klines};
pub use rest::{BinancePublicClient, MAINNET_REST_BASE};
pub use stream::{
    DEFAULT_MAX_SESSION_AGE, KlineStreamItem, KlineStreamSupervisor, ParsedKlineStreamMessage,
    ReconnectBackoff, StreamLifecycle, parse_kline_stream_message,
};
pub use websocket::{
    MAINNET_MARKET_WS_BASE, MAINNET_PUBLIC_WS_BASE, WebSocketCategory, combined_stream_url,
    single_stream_url,
};
