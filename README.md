# RHOF (Remote Hourly Opportunity Finder)

Rust/Axum implementation of the RHOF system for discovering and tracking remote hourly/flexible opportunities with provenance-first data capture.

## Workspace Layout

- `crates/` Rust workspace crates (`core`, `storage`, `adapters`, `sync`, `web`, `cli`)
- `migrations/` sqlx migrations
- `rules/` YAML-driven enrichment/risk/tag rules
- `docs/` architecture, data model, runbook, source notes
- `assets/` Tailwind input and compiled static CSS

## Quickstart (planned flow)

1. Start Postgres on host port `5401`: `just db-up`
2. Copy `.env.example` to `.env` and adjust if needed
3. Run migrations: `just migrate`
4. Start web app: `just serve`

Note: this repository is scaffolded prompt-by-prompt; later prompts add full implementation.
