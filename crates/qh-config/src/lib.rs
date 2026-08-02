//! Validated application configuration and redacted credentials.

use std::{env, fmt, str::FromStr};

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Configuration error.
#[derive(Debug, Error)]
pub enum ConfigError {
    /// Unknown environment name.
    #[error("unknown environment: {0}")]
    UnknownEnvironment(String),
    /// Numeric setting could not be parsed.
    #[error("invalid decimal for {name}: {value}")]
    InvalidDecimal {
        /// Setting name.
        name: &'static str,
        /// Raw value.
        value: String,
    },
    /// A validated limit was outside its allowed range.
    #[error("invalid risk limit: {0}")]
    InvalidRiskLimit(&'static str),
}

/// Runtime environment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Environment {
    /// Local development with no exchange writes.
    Development,
    /// Binance Testnet or demo environment.
    Testnet,
    /// Live market data with execution disabled.
    LiveDryRun,
    /// Live trading, requiring explicit approval in future milestones.
    Live,
}

impl FromStr for Environment {
    type Err = ConfigError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim().to_ascii_lowercase().as_str() {
            "development" | "dev" => Ok(Self::Development),
            "testnet" | "test" => Ok(Self::Testnet),
            "live-dry-run" | "dry-run" => Ok(Self::LiveDryRun),
            "live" => Ok(Self::Live),
            _ => Err(ConfigError::UnknownEnvironment(value.to_owned())),
        }
    }
}

/// Hard risk limits loaded at startup.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RiskConfig {
    /// Maximum gross notional divided by equity.
    pub max_gross_leverage: Decimal,
    /// Positive loss fraction, for example `0.02`.
    pub max_daily_loss_fraction: Decimal,
    /// Positive drawdown fraction, for example `0.08`.
    pub max_drawdown_fraction: Decimal,
}

impl RiskConfig {
    /// Validates safe ranges for the engineering baseline.
    pub fn validate(&self) -> Result<(), ConfigError> {
        if self.max_gross_leverage <= Decimal::ZERO || self.max_gross_leverage > Decimal::from(5) {
            return Err(ConfigError::InvalidRiskLimit(
                "max_gross_leverage must be in (0, 5]",
            ));
        }
        if self.max_daily_loss_fraction <= Decimal::ZERO
            || self.max_daily_loss_fraction >= Decimal::ONE
        {
            return Err(ConfigError::InvalidRiskLimit(
                "max_daily_loss_fraction must be in (0, 1)",
            ));
        }
        if self.max_drawdown_fraction <= Decimal::ZERO
            || self.max_drawdown_fraction >= Decimal::ONE
        {
            return Err(ConfigError::InvalidRiskLimit(
                "max_drawdown_fraction must be in (0, 1)",
            ));
        }
        Ok(())
    }
}

impl Default for RiskConfig {
    fn default() -> Self {
        Self {
            max_gross_leverage: Decimal::from(2),
            max_daily_loss_fraction: Decimal::new(2, 2),
            max_drawdown_fraction: Decimal::new(8, 2),
        }
    }
}

/// Application configuration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppConfig {
    /// Runtime environment.
    pub environment: Environment,
    /// Logging filter.
    pub log_filter: String,
    /// Hard risk limits.
    pub risk: RiskConfig,
}

impl AppConfig {
    /// Loads configuration from environment variables and validates it.
    pub fn from_env() -> Result<Self, ConfigError> {
        let environment = env::var("QUANTHELM_ENVIRONMENT")
            .unwrap_or_else(|_| "development".to_owned())
            .parse()?;
        let log_filter = env::var("QUANTHELM_LOG").unwrap_or_else(|_| "info".to_owned());
        let risk = RiskConfig {
            max_gross_leverage: decimal_env("QUANTHELM_MAX_GROSS_LEVERAGE", "2.0")?,
            max_daily_loss_fraction: decimal_env(
                "QUANTHELM_MAX_DAILY_LOSS_FRACTION",
                "0.02",
            )?,
            max_drawdown_fraction: decimal_env(
                "QUANTHELM_MAX_DRAWDOWN_FRACTION",
                "0.08",
            )?,
        };
        risk.validate()?;
        Ok(Self {
            environment,
            log_filter,
            risk,
        })
    }
}

fn decimal_env(name: &'static str, default: &'static str) -> Result<Decimal, ConfigError> {
    let value = env::var(name).unwrap_or_else(|_| default.to_owned());
    Decimal::from_str(&value).map_err(|_| ConfigError::InvalidDecimal { name, value })
}

/// Secret text that never exposes its contents through `Debug` or `Display`.
#[derive(Clone, PartialEq, Eq)]
pub struct SecretString(String);

impl SecretString {
    /// Wraps a secret value.
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// Exposes the secret only at the narrow integration boundary that needs it.
    #[must_use]
    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for SecretString {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("SecretString([REDACTED])")
    }
}

impl fmt::Display for SecretString {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("[REDACTED]")
    }
}

/// Exchange API credentials. This type is intentionally not serializable.
#[derive(Clone, PartialEq, Eq)]
pub struct ApiCredentials {
    /// API key.
    pub api_key: SecretString,
    /// API secret.
    pub api_secret: SecretString,
}

impl fmt::Debug for ApiCredentials {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ApiCredentials")
            .field("api_key", &"[REDACTED]")
            .field("api_secret", &"[REDACTED]")
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_valid() {
        assert!(RiskConfig::default().validate().is_ok());
    }

    #[test]
    fn excessive_leverage_is_rejected() {
        let config = RiskConfig {
            max_gross_leverage: Decimal::from(6),
            ..RiskConfig::default()
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn credentials_are_redacted() {
        let credentials = ApiCredentials {
            api_key: SecretString::new("key"),
            api_secret: SecretString::new("secret"),
        };
        let rendered = format!("{credentials:?}");
        assert!(!rendered.contains("key"));
        assert!(!rendered.contains("secret"));
    }
}
