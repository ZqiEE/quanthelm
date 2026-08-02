use chrono::{DateTime, Utc};
use qh_domain::Symbol;
use qh_market_data::{
    Interval,
    RawEvent,
};
use serde::{
    Deserialize,
    Serialize,
};

use crate::{
    BinanceError,
    SymbolRules,
};

/// Contract classification.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ContractType {
    /// Perpetual futures contract.
    Perpetual,
    /// Any supported-but-not-yet-modeled contract value.
    Other(String),
}

impl From<String> for ContractType {
    fn from(value: String) -> Self {
        match value.as_str() {
            "PERPETUAL" => Self::Perpetual,
            _ => Self::Other(value),
        }
    }
}

/// Trading status reported by exchange metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ContractStatus {
    /// Trading enabled.
    Trading,
    /// Trading pending.
    PendingTrading,
    /// Trading halt.
    TradingHalt,
    /// Cancellations only.
    TradingCancelOnly,
    /// Any supported-but-not-yet-modeled value.
    Other(String),
}

impl From<String> for ContractStatus {
    fn from(value: String) -> Self {
        match value.as_str() {
            "TRADING" => Self::Trading,
            "PENDING_TRADING" => Self::PendingTrading,
            "TRADING_HALT" => Self::TradingHalt,
            "TRADING_CANCEL_ONLY" => Self::TradingCancelOnly,
            _ => Self::Other(value),
        }
    }
}

/// One USDⓈ-M contract from exchange metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContractSpec {
    /// Contract symbol.
    pub symbol: Symbol,
    /// Contract type.
    pub contract_type: ContractType,
    /// Current exchange status.
    pub status: ContractStatus,
    /// Underlying/base asset.
    pub base_asset: String,
    /// Quote asset.
    pub quote_asset: String,
    /// Margin asset.
    pub margin_asset: String,
    /// Exchange onboarding time.
    pub onboarded_at: DateTime<Utc>,
    /// Parsed trading rules.
    pub rules: SymbolRules,
}

/// Exchange metadata snapshot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExchangeInfo {
    /// Exchange server time.
    pub server_time: DateTime<Utc>,
    /// Contracts.
    pub contracts: Vec<ContractSpec>,
    /// Exact raw payload for auditing.
    pub raw_event: RawEvent,
}

impl ExchangeInfo {
    /// Looks up one contract.
    #[must_use]
    pub fn contract(&self, symbol: &Symbol) -> Option<&ContractSpec> {
        self.contracts.iter().find(|item| &item.symbol == symbol)
    }

    /// Returns trading perpetual contracts.
    pub fn trading_perpetuals(&self) -> impl Iterator<Item = &ContractSpec> {
        self.contracts.iter().filter(|item| {
            item.contract_type == ContractType::Perpetual
                && item.status == ContractStatus::Trading
        })
    }
}

/// Kline REST request.
#[derive(Debug, Clone)]
pub struct KlineRequest {
    /// Contract symbol.
    pub symbol: Symbol,
    /// Supported interval.
    pub interval: Interval,
    /// Optional inclusive start in milliseconds.
    pub start_time_ms: Option<i64>,
    /// Optional inclusive end in milliseconds.
    pub end_time_ms: Option<i64>,
    /// Number of candles, 1 through 1500.
    pub limit: u16,
}

impl KlineRequest {
    /// Validates endpoint constraints.
    pub fn validate(&self) -> Result<(), BinanceError> {
        if !(1..=1_500).contains(&self.limit) {
            return Err(BinanceError::InvalidRequest(
                "kline limit must be between 1 and 1500".to_owned(),
            ));
        }
        if let (Some(start), Some(end)) = (self.start_time_ms, self.end_time_ms)
            && start > end
        {
            return Err(BinanceError::InvalidRequest(
                "start_time_ms must not exceed end_time_ms".to_owned(),
            ));
        }
        Ok(())
    }
}
