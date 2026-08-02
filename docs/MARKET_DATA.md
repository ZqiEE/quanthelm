# Market-data integrity

QuantHelm M1 uses public, read-only Binance USDⓈ-M data. No API key, account endpoint, or order endpoint exists in this layer.

## Supported REST endpoints

- `GET /fapi/v1/ping`
- `GET /fapi/v1/exchangeInfo`
- `GET /fapi/v1/klines`
- `GET /fapi/v1/premiumIndex`
- `GET /fapi/v1/ticker/bookTicker`
- `GET /fapi/v1/fundingRate`
- `GET /fapi/v1/openInterest`
- `GET /fapi/v1/ticker/24hr`

The adapter keeps Binance-specific response models inside `qh-binance` and emits strongly typed public models. No endpoint in this milestone accepts credentials or performs a write.

## Public market context

`BinancePublicClient::market_context` concurrently fetches one exact-symbol snapshot containing:

- mark price and index price;
- latest funding and interest rates;
- next funding timestamp;
- best bid/ask prices and quantities;
- current open interest;
- rolling 24-hour price, volume, quote volume, and trade count;
- a bounded set of recent funding observations.

Every response is checked against the requested symbol. A mismatched symbol, malformed decimal, non-positive required price, invalid timestamp, or funding limit outside `1..=1000` is an explicit error.

The independent CLI keeps this research surface separate from the trading process:

```bash
cargo run --locked -p quanthelm-market-context -- \
  --symbol BTCUSDT \
  --funding-limit 16
```

## WebSocket routing

Kline streams use Binance's classified regular-market endpoint:

- `wss://fstream.binance.com/market/stream?streams=<stream-1>/<stream-2>`

The supervisor:

- accepts one or more symbol/interval pairs;
- lowercases stream names only at the Binance boundary;
- rotates the session after 23 hours and 50 minutes;
- answers server ping frames with a pong carrying the same payload;
- reconnects with deterministic exponential backoff from one to 60 seconds;
- emits lifecycle transitions without treating a reconnect as data continuity;
- preserves every accepted text frame as a `RawEvent` with SHA-256;
- emits a normalized candle only when Binance marks `x=true`.

## Gap recovery

`GapDetector` tracks the latest accepted candle per `(symbol, interval)`.

When a newer candle proves that one or more candles are missing:

1. the detector does **not** advance its accepted state;
2. the REST client requests the half-open missing range;
3. pagination is limited to Binance's 1,500-candle page size;
4. every returned open time must match the next expected open time exactly;
5. the recovered candles are persisted and accepted in sequence;
6. only then is the newer WebSocket candle accepted.

An empty page, an unaligned range, a non-progressing page, or any timestamp mismatch is a hard error. The system never labels an unproven stream complete.

## Integrity rules

1. Exact raw exchange metadata and WebSocket text frames carry SHA-256 hashes.
2. Unknown exchange filters are retained by name rather than silently ignored.
3. Price and quantity normalization is derived from current `exchangeInfo`.
4. Kline intervals are limited to the MVP set: `15m`, `1h`, and `4h`.
5. Every persisted candle can be replayed in deterministic file order.
6. Missing, duplicate, and out-of-order candles are reported explicitly.
7. Duplicate and out-of-order candles never advance continuity state.
8. Public context responses must match the requested symbol.
9. Failure to prove continuity or parse a context component must never be represented as a valid snapshot.

## Kline CLI examples

```bash
cargo run --locked -p quanthelm -- binance watch-klines \
  --symbol BTCUSDT \
  --symbol ETHUSDT \
  --interval 15m \
  --output-directory data/live-15m

cargo run --locked -p quanthelm -- replay \
  --input data/live-15m/closed-klines.jsonl
```

The output directory contains:

- `raw.jsonl`: exact accepted text frames with receive time and payload hash;
- `closed-klines.jsonl`: accepted closed candles, including REST-recovered candles.

## Operational acceptance

The code-level M1 implementation is complete only after the [seven-day soak test](SOAK_TEST.md) proves stable reconnect, rotation, persistence, and continuity behavior under a real public market-data session.
