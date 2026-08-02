# Market-data integrity

QuantHelm M1 begins with public, read-only Binance USDⓈ-M data. No API key, account endpoint, or order endpoint is present in this slice.

## Supported REST endpoints

- `GET /fapi/v1/ping`
- `GET /fapi/v1/exchangeInfo`
- `GET /fapi/v1/klines`

The adapter keeps Binance-specific response models inside `qh-binance` and emits exchange-neutral `qh-market-data` types.

## Current WebSocket routing roots

The adapter exposes URL builders for Binance's classified USDⓈ-M WebSocket architecture:

- high-frequency public data: `wss://fstream.binance.com/public`
- regular market data: `wss://fstream.binance.com/market`

This slice does not open WebSocket connections yet. The next slice will add supervised connection rotation, reconnect backoff, duplicate handling, and REST gap backfill.

## Integrity rules

1. Exact raw exchange metadata is stored with a SHA-256 hash.
2. Unknown exchange filters are retained by name rather than silently ignored.
3. Price and quantity normalization is derived from current `exchangeInfo`.
4. Kline intervals are limited to the MVP set: `15m`, `1h`, and `4h`.
5. Every persisted candle can be replayed in deterministic file order.
6. Missing, duplicate, and out-of-order candles are reported explicitly.
7. Failure to prove continuity must never be represented as a complete stream.

## CLI examples

```bash
cargo run --locked -p quanthelm -- binance exchange-info --symbol BTCUSDT

cargo run --locked -p quanthelm -- binance download-klines \
  --symbol BTCUSDT \
  --interval 15m \
  --limit 500 \
  --output data/btcusdt-15m.jsonl

cargo run --locked -p quanthelm -- replay \
  --input data/btcusdt-15m.jsonl
```

## Next slice

- supervised `/public` and `/market` WebSocket connections;
- proactive connection rotation;
- ping/pong and backoff;
- raw WebSocket event persistence;
- closed-kline normalization;
- automatic REST backfill after a detected gap;
- deterministic reconnect and out-of-order fixtures.
