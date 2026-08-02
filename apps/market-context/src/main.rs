use std::{error::Error, str::FromStr};

use clap::Parser;
use qh_binance::{BinancePublicClient, MAINNET_REST_BASE};
use qh_domain::Symbol;

/// Fetches one read-only Binance USDⓈ-M public market context snapshot.
#[derive(Debug, Parser)]
#[command(name = "quanthelm-market-context", version)]
struct Cli {
    /// Exact contract symbol, for example BTCUSDT.
    #[arg(long)]
    symbol: String,
    /// Number of recent funding records, 1 through 1000.
    #[arg(long, default_value_t = 16)]
    funding_limit: u16,
    /// Public REST base override.
    #[arg(long, default_value = MAINNET_REST_BASE)]
    rest_base: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error + Send + Sync>> {
    let cli = Cli::parse();
    let symbol = Symbol::from_str(&cli.symbol)?;
    let client = BinancePublicClient::with_rest_base(cli.rest_base)?;
    let snapshot = client.market_context(&symbol, cli.funding_limit).await?;
    println!("{}", serde_json::to_string_pretty(&snapshot)?);
    Ok(())
}
