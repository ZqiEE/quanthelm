# QuantHelm / 量舵

AI-native Binance quantitative trading system with a deterministic Rust risk and execution core.

> 让 AI 研究市场，让 Rust 掌控风险。

## Status

QuantHelm is under active development and is **not ready for live trading**.

Current milestone: **M1 read-only Binance market data**.

Implemented:

- Rust 2024 workspace pinned to Rust 1.97.1
- decimal trading-domain types
- fail-closed risk gate
- read-only Binance USDⓈ-M public REST client
- typed `exchangeInfo` parsing with unknown-filter retention
- 15m, 1h, and 4h Kline normalization
- gap, duplicate, and out-of-order detection
- append-only JSONL storage and deterministic replay
- read-only CI, dependency auditing, and security guidance

Not implemented:

- API credentials
- private account streams
- order placement or cancellation
- live strategies
- AI execution authority

## Quick start

```bash
cargo run --locked -p quanthelm -- status

cargo run --locked -p quanthelm -- binance exchange-info --symbol BTCUSDT

cargo run --locked -p quanthelm -- binance download-klines \
  --symbol BTCUSDT \
  --interval 15m \
  --limit 500 \
  --output data/btcusdt-15m.jsonl

cargo run --locked -p quanthelm -- replay \
  --input data/btcusdt-15m.jsonl
```

## Architecture

```text
AI research (advisory only)
            ↓ typed proposals
market data → strategy → portfolio → risk gate → execution
     ↑                                       ↓
replay/store                           exchange adapter
```

AI cannot place orders, alter hard risk limits, approve live strategies, or access API secrets.

## Documentation

- [Architecture](docs/ARCHITECTURE.md)
- [Product scope](docs/PRODUCT.md)
- [Market-data integrity](docs/MARKET_DATA.md)
- [Risk policy](docs/RISK_POLICY.md)
- [Roadmap](docs/ROADMAP.md)
- [Security policy](SECURITY.md)
- [Contributing](CONTRIBUTING.md)

## Safety

QuantHelm does not provide investment advice or guarantee returns. Derivatives and leverage can cause substantial losses. Do not enable withdrawal permissions on an exchange API key. Use Testnet and dry-run modes before considering live execution.
