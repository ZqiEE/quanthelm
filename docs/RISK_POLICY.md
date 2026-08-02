# Default Risk Policy

Defaults are conservative starting points, not investment advice. Every change must be versioned and audited.

## Authority

- The risk gate has final veto power.
- AI and strategies cannot modify hard limits.
- A risk-engine failure rejects orders.
- An unexplained exchange-state mismatch enters reduce-only mode.

## Initial limits

| Limit | Default |
|---|---:|
| Maximum gross notional leverage | 2.0x |
| Maximum daily loss fraction | 2% |
| Maximum total drawdown fraction | 8% |
| Maximum single-symbol share of equity | 20% |
| Maximum single-strategy risk budget | 25% |
| Preferred margin mode | Isolated |
| AI order authority | None |
| Withdrawal permission | Forbidden |

## Risk states

- `Normal`: normal operation.
- `Degraded`: reduced capability or lower risk.
- `ReduceOnly`: only actions that reduce absolute exposure.
- `Halted`: strategy-generated orders are blocked.
- `EmergencyExit`: cancel and reduce exposure according to a pre-approved procedure.

## Mandatory triggers

At least `ReduceOnly` is required when:

- account, position, or open-order reconciliation fails;
- private account events cannot be trusted;
- market data exceeds its freshness threshold;
- an order outcome remains unknown;
- durable state cannot be written;
- risk configuration is missing or invalid;
- symbol metadata is stale;
- system clock drift exceeds the configured threshold.

## Prohibited behavior

- using liquidation as a stop-loss;
- unlimited averaging down;
- increasing leverage after losses;
- allowing model confidence to bypass limits;
- exposing secrets to AI context;
- silently accepting incomplete risk inputs.
