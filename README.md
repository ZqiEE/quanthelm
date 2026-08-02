# QuantHelm / 量舵

[![CI](https://github.com/ZqiEE/quanthelm/actions/workflows/ci.yml/badge.svg)](https://github.com/ZqiEE/quanthelm/actions/workflows/ci.yml)

**AI-native Binance quantitative trading system with a deterministic Rust risk and execution core.**

> 让 AI 研究市场，让 Rust 掌控风险。

QuantHelm is being built for independent quantitative research and disciplined execution on Binance USDⓈ-M perpetual futures. AI may propose experiments and explain results; it never owns exchange credentials, risk limits, or order execution.

## Status

**Pre-alpha. Not ready for live trading.** The current milestone establishes the engineering, domain, security, and risk baseline.

## MVP scope

- Binance USDⓈ-M perpetual futures
- 15m, 1h, and 4h research horizons
- Versioned market data and deterministic replay
- Event-driven backtesting
- Portfolio-level position sizing
- Independent risk gate
- Testnet execution and reconciliation
- AI research assistant with structured, non-trading tools

## Non-goals

- Profit guarantees or signal selling
- AI-controlled order placement
- Automatic leverage increases
- High-frequency market making
- Copy trading, custody, or withdrawals
- Unreviewed strategy changes in production

## Workspace

```text
apps/quanthelm     CLI entry point
crates/qh-domain   strongly typed trading domain
crates/qh-config   validated configuration and redacted credentials
crates/qh-exchange exchange abstraction
crates/qh-risk     deterministic risk gate
docs/              product, architecture, risk, roadmap, and ADRs
```

## Quick start

```bash
cargo run -p quanthelm -- status
cargo test --workspace
```

Live credentials are not required and should not be configured at this stage.

## Design rule

> No trustworthy data, recovery, reconciliation, and risk gate means no live trading.

## Documentation

- [Product specification](docs/PRODUCT.md)
- [Architecture](docs/ARCHITECTURE.md)
- [Risk policy](docs/RISK_POLICY.md)
- [Roadmap](docs/ROADMAP.md)
- [Contributing](CONTRIBUTING.md)
- [Security](SECURITY.md)

## Disclaimer

This software is provided for research and engineering purposes. It is not investment advice and does not guarantee profitability. Derivatives and leveraged trading can cause substantial losses. Confirm that Binance services are legally available in your jurisdiction before any use.

Licensed under either [Apache-2.0](LICENSE-APACHE) or [MIT](LICENSE-MIT), at your option.
