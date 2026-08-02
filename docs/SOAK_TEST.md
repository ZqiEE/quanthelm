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

cargo run --release --locked -p quanthelm-collector -- \
  --symbol BTCUSDT \
  --symbol ETHUSDT \
  --symbol BNBUSDT \
  --interval 15m \
  --metrics-interval-seconds 60 \
  --output-directory data/soak-15m \
  --log info \
  2>&1 | tee data/soak-15m/collector.log
```

## Generated evidence

- `raw.jsonl`: exact WebSocket text events, each wrapped with the collector instance UUID;
- `closed-klines.jsonl`: accepted closed candles, including REST-recovered candles;
- `metrics.json`: atomically replaced lifecycle and integrity counters;
- `collector.log`: connection, rotation, rejection, and recovery messages.

## Required observations

Record:

- process start and stop times in UTC;
- collector instance UUID;
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
8. every raw event verifies its SHA-256 hash and has the expected collector UUID;
9. malformed payloads do not crash the collector;
10. `metrics.json` counters agree with persisted evidence and logs;
11. memory and disk growth are explainable and bounded by retained data volume.

## Verification

```bash
cargo run --release --locked -p quanthelm -- replay \
  --input data/soak-15m/closed-klines.jsonl

cat data/soak-15m/metrics.json
sha256sum Cargo.lock
wc -l data/soak-15m/raw.jsonl data/soak-15m/closed-klines.jsonl
```

## Failure policy

A failed soak does not permit private streams, API credentials, Testnet orders, or execution work to proceed. Open an issue containing sanitized logs, timestamps, the exact commit SHA, collector instance UUID, metrics snapshot, and the smallest reproducible event sequence.
