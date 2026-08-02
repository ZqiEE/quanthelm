//! QuantHelm command-line entry point.

use clap::{Parser, Subcommand};
use qh_config::AppConfig;
use tracing::info;
use tracing_subscriber::EnvFilter;

type AppError = Box<dyn std::error::Error + Send + Sync>;

#[derive(Debug, Parser)]
#[command(
    name = "quanthelm",
    version,
    about = "AI-native quantitative trading system"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Prints the current engineering status without contacting an exchange.
    Status,
    /// Loads and validates environment configuration.
    ValidateConfig,
}

fn main() -> Result<(), AppError> {
    let cli = Cli::parse();
    let config = AppConfig::from_env()?;
    init_tracing(&config.log_filter)?;

    match cli.command {
        Command::Status => {
            info!(environment = ?config.environment, "QuantHelm initialized");
            println!("QuantHelm 0.1.0 — engineering baseline");
            println!("Environment: {:?}", config.environment);
            println!("Exchange writes: disabled");
            println!("AI trading authority: none");
        }
        Command::ValidateConfig => {
            println!("{}", serde_json::to_string_pretty(&config)?);
        }
    }

    Ok(())
}

fn init_tracing(filter: &str) -> Result<(), AppError> {
    let filter = EnvFilter::try_new(filter)?;
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .try_init()?;
    Ok(())
}
