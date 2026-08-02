use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

use crate::BinanceError;

/// Price bounds and tick size.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PriceFilter {
    /// Disabled when zero.
    pub min_price: Decimal,
    /// Disabled when zero.
    pub max_price: Decimal,
    /// Disabled when zero.
    pub tick_size: Decimal,
}

impl PriceFilter {
    /// Floors a candidate to the nearest valid tick anchored at `min_price`.
    pub fn floor(&self, candidate: Decimal) -> Result<Decimal, BinanceError> {
        if candidate <= Decimal::ZERO {
            return Err(BinanceError::InvalidRequest(
                "candidate price must be positive".to_owned(),
            ));
        }
        if self.min_price > Decimal::ZERO && candidate < self.min_price {
            return Err(BinanceError::InvalidRequest(format!(
                "price {candidate} is below minimum {}",
                self.min_price
            )));
        }

        let anchor = if self.min_price > Decimal::ZERO {
            self.min_price
        } else {
            Decimal::ZERO
        };
        let normalized = if self.tick_size > Decimal::ZERO {
            candidate - ((candidate - anchor) % self.tick_size)
        } else {
            candidate
        };

        if self.max_price > Decimal::ZERO && normalized > self.max_price {
            return Err(BinanceError::InvalidRequest(format!(
                "price {normalized} exceeds maximum {}",
                self.max_price
            )));
        }
        Ok(normalized)
    }
}

/// Quantity bounds and step size.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LotSizeFilter {
    /// Minimum quantity.
    pub min_quantity: Decimal,
    /// Maximum quantity.
    pub max_quantity: Decimal,
    /// Quantity increment.
    pub step_size: Decimal,
}

impl LotSizeFilter {
    /// Floors a candidate to the nearest valid quantity step.
    pub fn floor(&self, candidate: Decimal) -> Result<Decimal, BinanceError> {
        if candidate < self.min_quantity {
            return Err(BinanceError::InvalidRequest(format!(
                "quantity {candidate} is below minimum {}",
                self.min_quantity
            )));
        }
        let normalized = if self.step_size > Decimal::ZERO {
            candidate - ((candidate - self.min_quantity) % self.step_size)
        } else {
            candidate
        };
        if normalized > self.max_quantity {
            return Err(BinanceError::InvalidRequest(format!(
                "quantity {normalized} exceeds maximum {}",
                self.max_quantity
            )));
        }
        Ok(normalized)
    }
}

/// Price range relative to current mark price.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PercentPriceFilter {
    /// Upper multiplier.
    pub multiplier_up: Decimal,
    /// Lower multiplier.
    pub multiplier_down: Decimal,
    /// Decimal precision from exchange metadata.
    pub multiplier_decimal: u32,
}

/// Parsed per-symbol trading rules.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SymbolRules {
    /// Limit-order price filter.
    pub price: Option<PriceFilter>,
    /// Limit-order quantity filter.
    pub lot_size: Option<LotSizeFilter>,
    /// Market-order quantity filter.
    pub market_lot_size: Option<LotSizeFilter>,
    /// Relative mark-price bounds.
    pub percent_price: Option<PercentPriceFilter>,
    /// Minimum order notional.
    pub min_notional: Option<Decimal>,
    /// Maximum open order count.
    pub max_orders: Option<u64>,
    /// Maximum open algorithm order count.
    pub max_algo_orders: Option<u64>,
    /// Unknown filter names retained for forward compatibility.
    pub unknown_filters: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn price_and_quantity_floor_to_steps() {
        let price = PriceFilter {
            min_price: Decimal::new(1, 1),
            max_price: Decimal::from(1_000_000),
            tick_size: Decimal::new(1, 1),
        }
        .floor(Decimal::new(12_345, 2))
        .expect("valid price");
        assert_eq!(price, Decimal::new(12_340, 2));

        let quantity = LotSizeFilter {
            min_quantity: Decimal::new(1, 3),
            max_quantity: Decimal::from(1_000),
            step_size: Decimal::new(1, 3),
        }
        .floor(Decimal::new(1_234_567, 6))
        .expect("valid quantity");
        assert_eq!(quantity, Decimal::new(1_234_000, 6));
    }
}
