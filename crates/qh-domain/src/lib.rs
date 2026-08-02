//! Strongly typed, exchange-neutral trading domain types.

use std::{fmt, str::FromStr};

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

/// Domain validation failure.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum DomainError {
    /// A text identifier was empty or invalid.
    #[error("invalid identifier: {0}")]
    InvalidIdentifier(String),
    /// A numeric value was not strictly positive.
    #[error("{field} must be positive, got {value}")]
    NonPositive {
        /// Field name.
        field: &'static str,
        /// Rejected value.
        value: Decimal,
    },
    /// A required limit price was missing.
    #[error("limit order requires a price")]
    MissingLimitPrice,
}

/// Binance-style contract symbol, represented independently from the adapter.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Symbol(String);

impl Symbol {
    /// Creates a validated symbol.
    pub fn new(value: impl Into<String>) -> Result<Self, DomainError> {
        let value = value.into();
        let valid = !value.is_empty()
            && value.len() <= 32
            && value
                .bytes()
                .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit() || byte == b'_');
        if valid {
            Ok(Self(value))
        } else {
            Err(DomainError::InvalidIdentifier(value))
        }
    }

    /// Returns the raw symbol text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for Symbol {
    type Error = DomainError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl From<Symbol> for String {
    fn from(value: Symbol) -> Self {
        value.0
    }
}

impl fmt::Display for Symbol {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl FromStr for Symbol {
    type Err = DomainError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::new(value)
    }
}

macro_rules! positive_decimal_type {
    ($name:ident, $field:literal) => {
        #[doc = concat!("Validated positive ", $field, ".")]
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
        pub struct $name(Decimal);

        impl $name {
            #[doc = concat!("Creates a positive ", $field, ".")]
            pub fn new(value: Decimal) -> Result<Self, DomainError> {
                if value > Decimal::ZERO {
                    Ok(Self(value))
                } else {
                    Err(DomainError::NonPositive {
                        field: $field,
                        value,
                    })
                }
            }

            #[doc = concat!("Returns the underlying ", $field, ".")]
            #[must_use]
            pub const fn value(self) -> Decimal {
                self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                self.0.fmt(formatter)
            }
        }
    };
}

positive_decimal_type!(Price, "price");
positive_decimal_type!(Quantity, "quantity");
positive_decimal_type!(Notional, "notional");

/// Trade direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Side {
    /// Buy.
    Buy,
    /// Sell.
    Sell,
}

/// Whether an intent opens or reduces exposure.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PositionEffect {
    /// May create or increase exposure.
    Open,
    /// Must reduce absolute exposure.
    Reduce,
}

/// Exchange-neutral order style.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrderStyle {
    /// Market order.
    Market,
    /// Limit order.
    Limit,
}

/// Unique strategy identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct StrategyId(Uuid);

impl StrategyId {
    /// Creates a random strategy identifier.
    #[must_use]
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for StrategyId {
    fn default() -> Self {
        Self::new()
    }
}

/// Unique order-intent identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct IntentId(Uuid);

impl IntentId {
    /// Creates a random intent identifier.
    #[must_use]
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for IntentId {
    fn default() -> Self {
        Self::new()
    }
}

/// A strategy request that has no authority to reach an exchange directly.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OrderIntent {
    /// Unique intent ID.
    pub id: IntentId,
    /// Strategy that produced the intent.
    pub strategy_id: StrategyId,
    /// Contract symbol.
    pub symbol: Symbol,
    /// Buy or sell.
    pub side: Side,
    /// Open or reduce exposure.
    pub position_effect: PositionEffect,
    /// Market or limit.
    pub style: OrderStyle,
    /// Requested quantity.
    pub quantity: Quantity,
    /// Required for limit orders.
    pub limit_price: Option<Price>,
    /// Stable machine-readable rationale.
    pub reason: String,
    /// Creation timestamp.
    pub created_at: DateTime<Utc>,
    /// Optional expiry.
    pub expires_at: Option<DateTime<Utc>>,
}

impl OrderIntent {
    /// Validates cross-field invariants.
    pub fn validate(&self) -> Result<(), DomainError> {
        if self.style == OrderStyle::Limit && self.limit_price.is_none() {
            return Err(DomainError::MissingLimitPrice);
        }
        if self.reason.trim().is_empty() {
            return Err(DomainError::InvalidIdentifier("empty reason".to_owned()));
        }
        Ok(())
    }
}

/// An intent approved by the risk gate for execution.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApprovedOrder {
    /// Original validated intent.
    pub intent: OrderIntent,
    /// Time of risk approval.
    pub approved_at: DateTime<Utc>,
}

impl ApprovedOrder {
    /// Creates an approved order. Only the risk crate should normally call this.
    #[must_use]
    pub fn new(intent: OrderIntent, approved_at: DateTime<Utc>) -> Self {
        Self {
            intent,
            approved_at,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn symbol_rejects_lowercase() {
        assert!(Symbol::new("btcusdt").is_err());
    }

    #[test]
    fn positive_types_reject_zero() {
        assert!(Price::new(Decimal::ZERO).is_err());
        assert!(Quantity::new(Decimal::ZERO).is_err());
    }

    #[test]
    fn limit_order_requires_price() {
        let intent = OrderIntent {
            id: IntentId::new(),
            strategy_id: StrategyId::new(),
            symbol: Symbol::new("BTCUSDT").expect("valid symbol"),
            side: Side::Buy,
            position_effect: PositionEffect::Open,
            style: OrderStyle::Limit,
            quantity: Quantity::new(Decimal::ONE).expect("positive quantity"),
            limit_price: None,
            reason: "test".to_owned(),
            created_at: Utc::now(),
            expires_at: None,
        };

        assert_eq!(intent.validate(), Err(DomainError::MissingLimitPrice));
    }
}
