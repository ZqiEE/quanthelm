//! Exchange-neutral boundary implemented by adapters such as Binance.

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use qh_domain::{ApprovedOrder, Quantity, Symbol};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Exchange integration failure.
#[derive(Debug, Error)]
pub enum ExchangeError {
    /// Request timed out and the final state may be unknown.
    #[error("exchange request timed out; outcome_unknown={outcome_unknown}")]
    Timeout {
        /// True when the caller must reconcile before retrying.
        outcome_unknown: bool,
    },
    /// Exchange rejected a request.
    #[error("exchange rejected request: {code}: {message}")]
    Rejected {
        /// Exchange or adapter error code.
        code: String,
        /// Sanitized message.
        message: String,
    },
    /// Transport or protocol failure.
    #[error("exchange transport error: {0}")]
    Transport(String),
    /// Adapter received an invalid state transition.
    #[error("invalid exchange state: {0}")]
    InvalidState(String),
}

/// Minimal account snapshot used by the first integration milestone.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccountSnapshot {
    /// Account equity in the settlement asset.
    pub equity: Decimal,
    /// Gross position notional.
    pub gross_notional: Decimal,
    /// Snapshot time.
    pub observed_at: DateTime<Utc>,
}

/// Minimal position snapshot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PositionSnapshot {
    /// Contract.
    pub symbol: Symbol,
    /// Signed position quantity.
    pub signed_quantity: Decimal,
    /// Exchange observation time.
    pub observed_at: DateTime<Utc>,
}

/// Exchange order acknowledgement.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OrderAcknowledgement {
    /// Client-generated idempotency key.
    pub client_order_id: String,
    /// Exchange order identifier.
    pub exchange_order_id: String,
    /// Acknowledgement time.
    pub acknowledged_at: DateTime<Utc>,
}

/// Open order summary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OpenOrder {
    /// Client order identifier.
    pub client_order_id: String,
    /// Exchange order identifier.
    pub exchange_order_id: String,
    /// Contract.
    pub symbol: Symbol,
    /// Remaining quantity.
    pub remaining_quantity: Quantity,
}

/// Exchange interface. Implementations must treat uncertain writes as reconciliation problems.
#[async_trait]
pub trait Exchange: Send + Sync {
    /// Checks adapter and exchange connectivity.
    async fn health(&self) -> Result<(), ExchangeError>;

    /// Returns the authoritative account snapshot.
    async fn account_snapshot(&self) -> Result<AccountSnapshot, ExchangeError>;

    /// Returns currently open positions.
    async fn positions(&self) -> Result<Vec<PositionSnapshot>, ExchangeError>;

    /// Places a risk-approved order with an idempotent client order ID.
    async fn place_order(
        &self,
        order: ApprovedOrder,
        client_order_id: &str,
    ) -> Result<OrderAcknowledgement, ExchangeError>;

    /// Cancels an order by client order ID.
    async fn cancel_order(&self, client_order_id: &str) -> Result<(), ExchangeError>;

    /// Returns all open orders.
    async fn open_orders(&self) -> Result<Vec<OpenOrder>, ExchangeError>;
}
