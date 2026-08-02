# ADR-001: AI is advisory; Rust is authoritative

- Status: Accepted
- Date: 2026-08-02

## Context

Large language models are effective at translating ideas, planning experiments, summarizing evidence, and explaining anomalies. Their outputs are non-deterministic and unsuitable as the authority for funds, orders, or hard risk limits.

## Decision

- AI emits structured proposals validated against schemas.
- AI may use least-privilege research and reporting tools.
- AI cannot access credentials, execution, leverage, live approval, or hard-limit mutation.
- The Rust domain, risk, execution, and reconciliation path is authoritative.

## Consequences

The system is safer, auditable, and model-provider neutral. Research-to-live promotion is slower because proposals require deterministic validation and human approval; this is intentional.
