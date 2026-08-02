# ADR-002: MVP supports Binance USDⓈ-M perpetual futures only

- Status: Accepted
- Date: 2026-08-02

## Context

Supporting spot, USDⓈ-M, COIN-M, options, and several exchanges at once would multiply settlement, metadata, margin, risk, and execution complexity before the core is trustworthy.

## Decision

The MVP supports Binance USDⓈ-M perpetual futures, USDT-settled contracts first, one-way positions, isolated margin, and 15m/1h/4h horizons.

## Consequences

The product reaches a complete, testable loop sooner. Future markets require dedicated adapters, domain review, and independent test plans rather than conditional branches scattered through strategies.
