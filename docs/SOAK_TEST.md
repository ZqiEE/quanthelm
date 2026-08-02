# M1 seven-day market-data soak test

This runbook validates the read-only collector before any private API or execution work begins.

## Scope

Run at least:

- `BTCUSDT`, `ETHUSDT`, and one additional high-liquidity USDⓈ-M perpetual;
- 15-minute candles;
- one uninterrupted seven-day observation window;
- the same released binary and configuration for the full run.

Example:

```bash
mkdir -p data/soak-15m

RUST_LOG=info cargo run --release --locked -p quanthelm -- \
  binance watch-klines \
  --symbol BTCUSDT \
  --symbol ETHUSDT \
  --symbol BNBUSDT \
  --interval 15m \
  --output-directory data/soak-15m \
  2>&1 | tee data/soak-15m/collector.log
```

## Required observations

Record:

- process start and stop times in UTC;
- commit SHA and `Cargo.lock` SHA-256;
- operating system and Rust version;
- every lifecycle transition;
- reconnect and proactive-rotation reasons;
- every protocol rejection;
- every detected gap and REST recovery;
- process RSS and disk growth at least daily.

## Acceptance criteria

The soak passes only when:

1. the process runs for seven days without an uncontrolled exit;
2. at least one proactive session rotation completes successfully;
3. every disconnect is followed by a bounded reconnect or an explicit final error;
4. replay reports no unresolved gaps in accepted closed candles;
5. every reported gap has a matching successful REST recovery record;
6. no duplicate closed candle is persisted;
7. no out-of-order candle advances the accepted stream state;
8. every raw event verifies its SHA-256 hash;
9. malformed payloads do not crash the collector;
10. memory and disk growth are explainable and bounded by retained data volume.

## Verification

```bash
cargo run --release --locked -p quanthelm -- replay \
  --input data/soak-15m/closed-klines.jsonl

sha256sum Cargo.lock
wc -l data/soak-15m/raw.jsonl data/soak-15m/closed-klines.jsonl
```

## Failure policy

A failed soak does not permit private streams, API credentials, Testnet orders, or execution work to proceed. Open an issue containing sanitized logs, timestamps, the exact commit SHA, and the smallest reproducible event sequence.
