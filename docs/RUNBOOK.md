# Runbook

## Core Workflows

### Database

1. Start Postgres: `just db-up` (host port `5401`)
2. Run migrations: `just migrate`
3. Stop Postgres: `just db-down`

### Sync / Reports

1. Run sync: `cargo run -p rhof-cli -- sync`
2. Review outputs:
   - `reports/<run_id>/daily_brief.md`
   - `reports/<run_id>/opportunities_delta.json`
   - `reports/<run_id>/snapshots/*.parquet`
   - `reports/<run_id>/snapshots/manifest.json`
3. Summarize recent runs: `cargo run -p rhof-cli -- report daily --runs 3`

### Seed (Fixture-Derived)

1. Seed from checked-in fixture bundles: `cargo run -p rhof-cli -- seed`
2. Seed reuses the fixture-driven pipeline and is deterministic/idempotent at the artifact level (hash-addressed artifacts + stable keys in staged records)

## Adapter Expansion Workflow (PROMPT_10)

1. Add/update source entry in `sources.yaml`
2. Generate scaffold: `cargo run -p rhof-cli -- new-adapter <source_id>`
3. Replace generated fixture placeholders with real captured fixture bundle + raw artifacts
4. Implement adapter parsing logic and register in `adapter_for_source`
5. Add/complete snapshot parsing test
6. Run adapter contract checks: `python3 scripts/check_adapters.py`
7. Run tests
8. Run local sync + report
9. Expand in chunks of 3-5 sources; after chunk 1, ensure end-to-end sync + report + web smoke checks before continuing

## Frontend Assets

1. Install standalone Tailwind binary (once per machine): `just tailwind-install`
2. Rebuild CSS: `just tailwind`
