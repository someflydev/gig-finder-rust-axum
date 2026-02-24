# RHOF (Remote Hourly Opportunity Finder)

Rust/Axum implementation of the RHOF system for discovering and tracking remote hourly/flexible opportunities with provenance-first data capture.

## Workspace Layout

- `crates/` Rust workspace crates (`core`, `storage`, `adapters`, `sync`, `web`, `cli`)
- `migrations/` sqlx migrations
- `rules/` YAML-driven enrichment/risk/tag rules
- `docs/` architecture, data model, runbook, source notes
- `assets/` Tailwind input and compiled static CSS

## Quickstart

1. Start Postgres on host port `5401`:
   `just db-up`
2. Copy env config:
   `cp .env.example .env`
3. Apply database migrations:
   `just migrate`
4. Run a fixture-driven sync (writes `artifacts/` and `reports/<run_id>/`):
   `cargo run -p rhof-cli -- sync`
5. View a summary of recent runs:
   `cargo run -p rhof-cli -- report daily --runs 3`
6. Start the web UI (default `http://localhost:8000`):
   `cargo run -p rhof-cli -- serve`

Useful commands:

- `cargo run -p rhof-cli -- seed` (fixture-derived seed/import path)
- `cargo run -p rhof-cli -- debug` (env + recent report summary)
- `just tailwind-install` (installs standalone Tailwind binary to `./bin/tailwindcss`)
- `cargo test --workspace`
