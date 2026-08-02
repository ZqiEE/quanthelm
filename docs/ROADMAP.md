# Roadmap

## M0 — Engineering baseline

- Rust workspace and CI
- strong domain types
- validated configuration and secret redaction
- exchange abstraction
- independent risk gate
- security, contribution, architecture, and product documents

**Exit:** CI passes and no live credential or exchange call exists.

## M1 — Read-only Binance data

- exchange metadata and symbol filters
- historical Klines
- mark price, book ticker, funding rate, and open interest
- supervised WebSocket connections
- gap detection and REST backfill
- raw event persistence and deterministic replay

**Exit:** seven-day continuous collection with no unexplained Kline gaps.

## M2 — Trustworthy backtesting

- shared feature and strategy interfaces
- event-driven ledger
- fees, funding, slippage, partial fills, and rejects
- multi-symbol portfolio
- out-of-sample and parameter-stability reports

**Exit:** reproducible results, balanced ledger, and look-ahead tests.

## M3 — Testnet execution

- private account stream
- order state machine
- client order IDs and idempotency
- partial fills, cancellation, and unknown-result recovery
- restart recovery and reconciliation

**Exit:** no duplicate order under response-loss, replay, or restart tests.

## M4 — Complete risk engine

- order, symbol, strategy, and account checks
- risk state transitions
- stale-data, daily-loss, drawdown, and reconciliation gates
- immutable audit events

**Exit:** no execution path can bypass risk.

## M5 — AI researcher

- provider-neutral model gateway
- JSON Schema proposals
- least-privilege research tools
- experiment orchestration and explanations
- prompt-injection and invalid-output tests

**Exit:** AI has no trading authority and can be disabled safely.

## M6 — Small-capital live candidate

- live dry-run
- alerts and runbooks
- backups and disaster-recovery exercise
- at least 30 days of Testnet operation
- explicit local approval command

**Exit:** a human must approve an immutable strategy/configuration version before live mode.
