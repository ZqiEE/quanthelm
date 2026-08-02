# Contributing

QuantHelm handles safety-sensitive trading logic. Contributions are reviewed in this order:

1. Correctness and tests
2. Data integrity and recovery
3. Risk controls
4. Observability and documentation
5. Strategies and AI features

## Development checks

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --all-features --no-deps
```

## Pull requests

A PR must state:

- the concrete problem;
- scope and non-goals;
- failure modes and safe default;
- validation performed;
- impact on orders, positions, balances, or limits.

Trading and risk changes also require recovery tests, audit events, and a Testnet validation plan.

## Rejected patterns

The project will not accept changes that:

- bypass the risk gate;
- let AI place orders or change hard limits;
- increase default leverage without evidence and review;
- log credentials or signed requests;
- advertise guaranteed returns;
- add an unverified strategy without reproducible research.
