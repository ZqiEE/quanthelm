# Architecture

## Authority model

```text
AI research layer
  └─ proposals, experiments, explanations

Deterministic Rust core
  ├─ domain and configuration
  ├─ market data and features
  ├─ strategy and portfolio
  ├─ risk gate
  ├─ execution
  └─ reconciliation

Exchange adapter
  └─ REST, WebSocket, signatures, rate limits, metadata
```

AI has no dependency path to execution. Every order source, including manual tools, must produce an `OrderIntent` and pass the same risk gate.

## Crate dependency direction

```text
qh-domain
   ↑
qh-config      qh-exchange
   ↑              ↑
   └──────── qh-risk
                  ↑
             applications
```

Future strategy, portfolio, backtest, execution, reconciliation, AI, and reporting crates must preserve one-way dependencies and avoid exchange-specific types outside adapters.

## Sources of truth

| Data | Authority |
|---|---|
| Orders and fills | Binance |
| Live balances and positions | Binance, mirrored after reconciliation |
| Historical research data | Versioned local datasets |
| Strategy configuration | Git SHA and configuration hash |
| Risk decisions | Append-only audit events |
| AI explanations | Non-authoritative derived content |

## Safe defaults

| Failure | Default response |
|---|---|
| AI unavailable | Core continues; no new AI work |
| Market data stale | Reject new exposure |
| Private stream disconnected | Reconnect and reconcile |
| Order result unknown | Query; never blindly duplicate |
| Persistence unavailable | Halt or reduce-only |
| Risk engine unavailable | Reject every order |
| Local/exchange mismatch | Reduce-only until resolved |
| Metadata stale | Reject affected symbols |

## Planned runtime components

- `collector`: public market data and metadata;
- `trader`: private streams, strategies, portfolio, risk, execution, reconciliation;
- `researcher`: historical data, features, backtests, and AI experiments;
- `server`: local API and status surface;
- `reporter`: daily attribution and alerts.

MVP may run these as tasks in fewer processes, but code boundaries should allow separation later.
