//! Deterministic, fail-closed order risk gate.

use chrono::Utc;
use qh_config::RiskConfig;
use qh_domain::{ApprovedOrder, IntentId, OrderIntent, PositionEffect, Price};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

/// Global risk operating state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RiskState {
    /// Normal operation.
    Normal,
    /// Reduced capability; callers may apply stricter limits.
    Degraded,
    /// Only exposure-reducing actions are allowed.
    ReduceOnly,
    /// Strategy orders are blocked.
    Halted,
    /// A pre-approved emergency reduction procedure is active.
    EmergencyExit,
}

/// Stable machine-readable rejection reason.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RiskCode {
    /// Intent itself is invalid.
    InvalidIntent,
    /// Account equity is missing or non-positive.
    InvalidEquity,
    /// System state does not permit the action.
    StateBlocked,
    /// Market data is too old or incomplete.
    StaleMarketData,
    /// Daily loss threshold has been reached.
    DailyLossLimit,
    /// Drawdown threshold has been reached.
    DrawdownLimit,
    /// Projected gross leverage is too high.
    GrossLeverageLimit,
}

/// Account-level inputs required by the baseline gate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccountRiskSnapshot {
    /// Current account equity.
    pub equity: Decimal,
    /// Existing gross position notional.
    pub gross_notional: Decimal,
    /// Signed daily PnL fraction; a loss is negative.
    pub daily_pnl_fraction: Decimal,
    /// Positive peak-to-current drawdown fraction.
    pub drawdown_fraction: Decimal,
}

/// Inputs whose freshness must be established outside the strategy.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MarketRiskSnapshot {
    /// Mark price used for notional calculations.
    pub mark_price: Price,
    /// True only when all required inputs satisfy configured freshness thresholds.
    pub is_fresh: bool,
}

/// Complete deterministic risk context.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RiskContext {
    /// Global operating state.
    pub state: RiskState,
    /// Account snapshot.
    pub account: AccountRiskSnapshot,
    /// Market snapshot for the intent symbol.
    pub market: MarketRiskSnapshot,
}

/// Result of evaluating an order intent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RiskDecision {
    /// Intent may proceed to execution.
    Approved(ApprovedOrder),
    /// Intent is rejected without side effects.
    Rejected {
        /// Rejected intent identifier.
        intent_id: IntentId,
        /// Machine-readable code.
        code: RiskCode,
        /// Human-readable explanation.
        reason: String,
    },
}

/// Fail-closed risk gate.
#[derive(Debug, Clone)]
pub struct RiskGate {
    limits: RiskConfig,
}

impl RiskGate {
    /// Creates a gate after validating hard limits.
    pub fn new(limits: RiskConfig) -> Result<Self, qh_config::ConfigError> {
        limits.validate()?;
        Ok(Self { limits })
    }

    /// Evaluates an intent without mutating account state.
    #[must_use]
    pub fn evaluate(&self, context: &RiskContext, intent: OrderIntent) -> RiskDecision {
        if let Err(error) = intent.validate() {
            return rejected(intent.id, RiskCode::InvalidIntent, error.to_string());
        }

        if context.account.equity <= Decimal::ZERO {
            return rejected(
                intent.id,
                RiskCode::InvalidEquity,
                "account equity must be positive".to_owned(),
            );
        }

        let opens_exposure = intent.position_effect == PositionEffect::Open;
        if opens_exposure
            && matches!(
                context.state,
                RiskState::ReduceOnly | RiskState::Halted | RiskState::EmergencyExit
            )
        {
            return rejected(
                intent.id,
                RiskCode::StateBlocked,
                format!("risk state {:?} blocks new exposure", context.state),
            );
        }

        if opens_exposure && !context.market.is_fresh {
            return rejected(
                intent.id,
                RiskCode::StaleMarketData,
                "market data is not fresh".to_owned(),
            );
        }

        if opens_exposure
            && context.account.daily_pnl_fraction <= -self.limits.max_daily_loss_fraction
        {
            return rejected(
                intent.id,
                RiskCode::DailyLossLimit,
                "daily loss limit reached".to_owned(),
            );
        }

        if opens_exposure && context.account.drawdown_fraction >= self.limits.max_drawdown_fraction
        {
            return rejected(
                intent.id,
                RiskCode::DrawdownLimit,
                "maximum drawdown limit reached".to_owned(),
            );
        }

        if opens_exposure {
            let added_notional = intent.quantity.value() * context.market.mark_price.value();
            let projected_gross = context.account.gross_notional + added_notional;
            let projected_leverage = projected_gross / context.account.equity;
            if projected_leverage > self.limits.max_gross_leverage {
                return rejected(
                    intent.id,
                    RiskCode::GrossLeverageLimit,
                    format!(
                        "projected gross leverage {projected_leverage} exceeds {}",
                        self.limits.max_gross_leverage
                    ),
                );
            }
        }

        RiskDecision::Approved(ApprovedOrder::new(intent, Utc::now()))
    }
}

fn rejected(intent_id: IntentId, code: RiskCode, reason: String) -> RiskDecision {
    RiskDecision::Rejected {
        intent_id,
        code,
        reason,
    }
}

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use qh_domain::{IntentId, OrderStyle, PositionEffect, Quantity, Side, StrategyId, Symbol};

    use super::*;

    fn intent(effect: PositionEffect, quantity: Decimal) -> OrderIntent {
        OrderIntent {
            id: IntentId::new(),
            strategy_id: StrategyId::new(),
            symbol: Symbol::new("BTCUSDT").expect("valid symbol"),
            side: Side::Buy,
            position_effect: effect,
            style: OrderStyle::Market,
            quantity: Quantity::new(quantity).expect("positive quantity"),
            limit_price: None,
            reason: "risk-test".to_owned(),
            created_at: Utc::now(),
            expires_at: None,
        }
    }

    fn context(state: RiskState) -> RiskContext {
        RiskContext {
            state,
            account: AccountRiskSnapshot {
                equity: Decimal::from(1_000),
                gross_notional: Decimal::ZERO,
                daily_pnl_fraction: Decimal::ZERO,
                drawdown_fraction: Decimal::ZERO,
            },
            market: MarketRiskSnapshot {
                mark_price: Price::new(Decimal::from(100)).expect("positive price"),
                is_fresh: true,
            },
        }
    }

    #[test]
    fn normal_state_approves_small_order() {
        let gate = RiskGate::new(RiskConfig::default()).expect("valid defaults");
        let decision = gate.evaluate(
            &context(RiskState::Normal),
            intent(PositionEffect::Open, Decimal::ONE),
        );
        assert!(matches!(decision, RiskDecision::Approved(_)));
    }

    #[test]
    fn reduce_only_blocks_opening_but_allows_reduction() {
        let gate = RiskGate::new(RiskConfig::default()).expect("valid defaults");
        let context = context(RiskState::ReduceOnly);

        assert!(matches!(
            gate.evaluate(&context, intent(PositionEffect::Open, Decimal::ONE)),
            RiskDecision::Rejected {
                code: RiskCode::StateBlocked,
                ..
            }
        ));
        assert!(matches!(
            gate.evaluate(&context, intent(PositionEffect::Reduce, Decimal::ONE)),
            RiskDecision::Approved(_)
        ));
    }

    #[test]
    fn projected_leverage_is_enforced() {
        let gate = RiskGate::new(RiskConfig::default()).expect("valid defaults");
        let decision = gate.evaluate(
            &context(RiskState::Normal),
            intent(PositionEffect::Open, Decimal::from(21)),
        );
        assert!(matches!(
            decision,
            RiskDecision::Rejected {
                code: RiskCode::GrossLeverageLimit,
                ..
            }
        ));
    }

    #[test]
    fn stale_data_blocks_new_exposure() {
        let gate = RiskGate::new(RiskConfig::default()).expect("valid defaults");
        let mut context = context(RiskState::Normal);
        context.market.is_fresh = false;
        assert!(matches!(
            gate.evaluate(&context, intent(PositionEffect::Open, Decimal::ONE)),
            RiskDecision::Rejected {
                code: RiskCode::StaleMarketData,
                ..
            }
        ));
    }
}
