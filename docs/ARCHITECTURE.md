# Architecture

## As-Built Overview

RHOF is a Rust workspace with six crates:

- `rhof-core`: canonical domain and provenance types (`Field<T>`, `EvidenceRef`, `OpportunityDraft`, `Opportunity`)
- `rhof-storage`: immutable artifact storage + HTTP client/retry/rate-limit utilities
- `rhof-adapters`: source adapter contract, fixture bundle schema, fixture-first adapter implementations, generator templates
- `rhof-sync`: source registry loading, sync orchestration, dedup/rules enrichment, DB persistence, reports, Parquet export, scheduler scaffolding
- `rhof-web`: Axum + Askama + HTMX UI and JSON chart route
- `rhof-cli`: operational entrypoints (`migrate`, `sync`, `report`, `seed`, `debug`, `serve`, `scheduler`)

## Pipeline (Current Runtime Path)

1. `rhof-cli sync` loads env config and constructs `rhof-sync` pipeline.
2. `sources.yaml` is parsed as the authoritative source registry (`sources:` top-level list).
3. Source rows are upserted into Postgres (`sources` table).
4. A `fetch_runs` row is created.
5. For each enabled source:
   - load fixture/manual bundle
   - store immutable raw artifact under `ARTIFACTS_DIR` (hash-addressed)
   - upsert `raw_artifacts` row with deterministic raw artifact ID (fixture-derived)
   - parse adapter output into `OpportunityDraft`
6. Drafts are normalized into canonical keys.
7. Dedup hook runs (Jaro-Winkler thresholding + review flags).
8. YAML-driven enrichment rules run (`rules/tags.yaml`, `rules/risk.yaml`, `rules/pay.yaml`).
9. Opportunities + versions + tags + risk flags + review items are persisted into Postgres.
10. Reports and Parquet snapshots are written under `reports/<run_id>/`.

## Data Read Paths

- Web UI now prefers DB-backed source/opportunity reads (via `sqlx`) and falls back to `sources.yaml` + latest report JSON if DB data is unavailable.
- Reports page and chart continue to read generated `reports/` artifacts for run summaries.

## Scheduler Status

- Cron scheduler jobs can be created from env (`SYNC_CRON_1`, `SYNC_CRON_2`) and are executed in `rhof-cli scheduler`.
- Current scheduler mode is operational but minimal (no supervision/retries/metrics daemon features yet).

## Known Gaps / Roadmap Notes

- Adapters are still fixture-first and mostly replay parsed fixture records; raw HTML/JSON parsing is only partially demonstrated.
- Dedup cluster proposal persistence (`dedup_clusters`, `dedup_cluster_members`) is not yet implemented.
- Review resolve endpoint in web UI is UI-only (non-durable) and should be connected to persisted `review_items`.
