# QuantHelm Product Specification

**Version:** 0.1  
**Status:** Engineering baseline  
**Primary user:** the project author  
**Market:** Binance USDⓈ-M perpetual futures

## Product definition

QuantHelm turns a trading hypothesis into a reproducible experiment, subjects it to deterministic validation, and allows only approved strategy versions to reach a Rust risk and execution core.

```text
idea → structured proposal → versioned experiment → backtest
     → stress tests → paper/Testnet → human approval
     → risk gate → execution → reconciliation → review
```

## Product principles

1. Capital safety before strategy returns.
2. Correctness before feature count.
3. Deterministic authority before AI autonomy.
4. Recovery and reconciliation before live trading.
5. Reproducibility before impressive charts.
6. Personal utility before monetization.

## MVP

- Binance USDⓈ-M perpetual futures only;
- one-way position mode and isolated margin first;
- 15m, 1h, and 4h strategies;
- 5–20 liquid contracts selected by deterministic rules;
- versioned market data;
- shared strategy interfaces for replay, backtest, and live operation;
- portfolio-level sizing;
- four-layer risk checks;
- Testnet execution and reconciliation;
- AI-generated research proposals and reports.

## AI boundary

AI may:

- translate natural language into a structured research proposal;
- propose factors, tests, and failure hypotheses;
- launch approved read-only experiment tools;
- compare reports and explain anomalies.

AI may not:

- access exchange secrets;
- place or cancel orders;
- change leverage or hard limits;
- approve a strategy for live trading;
- modify production configuration;
- execute generated code outside an isolated validation path.

## Strategy lifecycle

```text
DRAFT → RESEARCH → BACKTESTED → VALIDATED
      → PAPER → TESTNET → APPROVED → LIVE
      → PAUSED / RETIRED
```

Every transition records actor, timestamp, code SHA, configuration hash, data version, report identifiers, and reason.

## MVP success criteria

Success is not measured primarily by return. The baseline succeeds when:

- repeated backtests are deterministic;
- every fill is traceable to an intent and risk decision;
- reconnect and restart do not duplicate orders;
- uncertain order outcomes are reconciled instead of retried blindly;
- stale data or inconsistent account state stops new risk;
- AI failure has no effect on trading safety;
- daily reports explain PnL, costs, risk events, and system incidents.

## Explicit non-goals

- guaranteed returns;
- signal marketplace;
- copy trading or custody;
- spot, COIN-M, options, and cross-exchange arbitrage in MVP;
- high-frequency market making;
- reinforcement learning with direct execution authority;
- multi-tenant SaaS.
