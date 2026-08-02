# Security Policy

## Project maturity

QuantHelm is pre-alpha and must not be considered production-ready.

## Credential rules

- Enable only read and trading permissions.
- Never enable withdrawals.
- Use an IP allowlist where available.
- Keep Testnet and live credentials separate.
- Never place credentials in Git, logs, reports, crash dumps, or AI context.
- Rotate a credential immediately if exposure is suspected.

## Reporting

Use GitHub Security Advisories for vulnerabilities. Do not publish credentials, account identifiers, exploitable details, or live positions in a public issue.

## Safe failure

A failure in configuration, market-data freshness, persistence, reconciliation, or the risk engine must prevent new risk. AI unavailability must not affect the deterministic trading core.
