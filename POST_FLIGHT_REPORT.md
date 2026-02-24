# Post-Flight Report

## 1. Repo Summary (What it is, what it promises, how to run)

### What it is (factual)
RHOF is a Rust workspace for provenance-first remote opportunity discovery/tracking with:
- a core provenance/domain model (`rhof-core`)
- immutable artifact storage + HTTP utilities (`rhof-storage`)
- a fixture-first adapter framework + generator (`rhof-adapters`)
- sync/dedup/rules/report/parquet pipeline (`rhof-sync`)
- an Axum/Askama/HTMX web UI (`rhof-web`)
- a CLI entrypoint (`rhof-cli`)

Workspace crates are declared in `Cargo.toml:1` and `Cargo.toml:2`. The CLI entrypoint is `crates/rhof-cli/src/main.rs:37`.

### What it promises (from prompt plan)
The prompt sequence defines RHOF as a production-grade Rust/Axum implementation that should `fetch -> immutable artifact -> parse -> normalize -> dedupe -> enrich -> version -> persist`, with provenance visibility, HTMX UI, Parquet snapshots, and ToS-safe source handling (`.prompts/PROMPT_00_s.txt:89`, `.prompts/PROMPT_00_s.txt:92`, `.prompts/PROMPT_00_s.txt:95`).

### How to run locally (as-built, factual)
- `just db-up` (Postgres on host port `5401`) (`docker-compose.yml:7`, `justfile:11`)
- `cp .env.example .env` (`README.md:17`)
- `just migrate` (sqlx embedded migrations) (`README.md:19`, `justfile:17`, `crates/rhof-sync/src/lib.rs:1107`)
- `cargo run -p rhof-cli -- sync` (`README.md:22`, `crates/rhof-cli/src/main.rs:42`)
- `cargo run -p rhof-cli -- report daily --runs 3` (`README.md:24`, `.prompts/PROMPT_07.txt:10`)
- `cargo run -p rhof-cli -- serve` (`README.md:26`, `crates/rhof-web/src/lib.rs:206`)
- Optional scheduler mode: `cargo run -p rhof-cli -- scheduler` with `RHOF_SCHEDULER_ENABLED=true` (`README.md:32`, `crates/rhof-sync/src/lib.rs:1137`)
- Optional Tailwind standalone installer: `just tailwind-install` (`justfile:23`, `scripts/install-tailwind.sh:1`)

### Build signals / dependencies (factual)
- Rust/Cargo workspace (`Cargo.toml:1`)
- Docker Postgres service (`docker-compose.yml:2`)
- Python + PyYAML for adapter contract checks (`.github/workflows/ci.yml:57`, `scripts/check_adapters.py:7`)
- Tailwind standalone binary workflow (no Node) (`.prompts/PROMPT_00_s.txt:37`, `scripts/install-tailwind.sh:31`)

### Verification performed during this audit (factual)
- `cargo build --workspace` passed
- `cargo test --workspace` passed (16 tests)
- `python3 scripts/check_adapters.py` passed
- `cargo run -p rhof-cli -- scheduler` correctly errors when disabled (`RHOF_SCHEDULER_ENABLED=false`)
- `rhof-cli migrate` and repeated `rhof-cli sync` were previously re-verified in this session’s earlier fix round (DB persistence + idempotent version insertion)

### Demo mode status (factual)
The happy path is still fixture-driven (source fixtures/manual bundles), but it now persists to Postgres and also writes reports/parquet (`crates/rhof-sync/src/lib.rs:495`, `crates/rhof-sync/src/lib.rs:676`, `crates/rhof-sync/src/lib.rs:1013`).

## 2. Prompt Intent Map (compressed)

### Scope / vision statement (extracted)
- Build a provenance-first RHOF system in Rust/Axum with immutable artifacts, evidence on extracted fields, dedup/review, data-driven rules, Parquet exports, CLI/web UI, and ToS-safe adapters (`.prompts/PROMPT_00_s.txt:99`, `.prompts/PROMPT_00_s.txt:102`, `.prompts/PROMPT_00_s.txt:107`).

### Constraints / quality bars (extracted)
- Repo-root generation, preserve `.prompts/`, no nested git repo (`.prompts/PROMPT_00_s.txt:16`, `.prompts/PROMPT_00_s.txt:20`)
- Postgres host port `5401` (`.prompts/PROMPT_00_s.txt:45`, `.prompts/PROMPT_00_s.txt:80`)
- Tailwind standalone binary, no Node (`.prompts/PROMPT_00_s.txt:37`)
- Fixture + snapshot tests for adapters (`.prompts/PROMPT_04.txt:34`, `.prompts/PROMPT_04.txt:35`)
- Data-driven YAML rules (`.prompts/PROMPT_06.txt:14`)
- Chunked adapter expansion with gates (`.prompts/PROMPT_10.txt:23`, `.prompts/PROMPT_10.txt:33`)

### Sequencing assumptions (observed)
- Prompt 05 intentionally wires dedup/enrichment hooks before Prompt 06 fills behavior (`.prompts/PROMPT_05.txt:3`, `.prompts/PROMPT_05.txt:5`)
- Prompt 07 introduces `report`, Prompt 09 extends the same CLI (`.prompts/PROMPT_07.txt:12`, `.prompts/PROMPT_09.txt:10`)
- Prompt 10 relies on Prompt 01 `sources.yaml` schema (`.prompts/PROMPT_01.txt:38`, `.prompts/PROMPT_10.txt:24`)

### Prompt ID -> intent map
- `PROMPT_00_s.txt`: execution contract, stack, mission, ports, principles
- `PROMPT_01.txt`: workspace scaffold + env/Docker/Just/docs/rules/source registry
- `PROMPT_02.txt`: core provenance/domain model + DB schema
- `PROMPT_03.txt`: artifact storage + HTTP/retry/rate limits + tests
- `PROMPT_04.txt`: adapter contract + initial adapters + fixtures/manual schema + snapshots
- `PROMPT_05.txt`: sync orchestration + scheduler seam + reports + idempotency
- `PROMPT_06.txt`: dedup + YAML rules + tests
- `PROMPT_07.txt`: Parquet + manifest + report CLI
- `PROMPT_08.txt`: web UI routes + Askama + HTMX + Plotly JSON + smoke tests
- `PROMPT_09.txt`: CLI commands + seed semantics + CI + guardrails
- `PROMPT_10.txt`: adapter generator + templates + CI contract checks + chunked expansion workflow

## 3. Traceability: Prompt -> Artifact Delivery Table

| Prompt ID | Intended artifacts | Found artifacts | Status | Notes | Suggested follow-up |
|---|---|---|---|---|---|
| `PROMPT_00_s.txt` | Execution contract + stack + mission constraints | `.prompts/PROMPT_00_s.txt`, repo-root RHOF workspace, runnable CLI/web, persisted sync path | Partial | Major contract elements now implemented, but live fetching and full review/dedup cluster lifecycle are still incomplete (`crates/rhof-adapters/src/lib.rs:234`, `crates/rhof-sync/src/lib.rs:898`). | Finish durable review resolution + dedup cluster persistence; broaden real parser coverage. |
| `PROMPT_01.txt` | Repo-root workspace + Docker/Just/env/docs/rules/`sources.yaml` + Tailwind workflow | All listed scaffold artifacts present | Delivered | Tailwind bootstrap gap resolved with `just tailwind-install` and script (`justfile:23`, `scripts/install-tailwind.sh:31`). | Optional: add checksum verification in Tailwind installer. |
| `PROMPT_02.txt` | Domain model + migrations + migration execution path | `rhof-core` types + SQL migration + embedded migrator + CLI migrate command | Delivered | Migrations are runnable and migration history is tracked (`crates/rhof-sync/src/lib.rs:31`, `crates/rhof-sync/src/lib.rs:1107`). | Add migration integration test against ephemeral Postgres in CI (future). |
| `PROMPT_03.txt` | Immutable artifact storage + HTTP client/retry/rate-limits + tests | `crates/rhof-storage/src/lib.rs` | Delivered | Strong implementation and tests for hashing/atomic writes/backoff (`crates/rhof-storage/src/lib.rs:65`, `crates/rhof-storage/src/lib.rs:397`). | Add HTTP integration tests for retry classifications. |
| `PROMPT_04.txt` | Adapter trait + 5 initial adapters + unified fixture/manual schema + snapshots | `rhof-adapters`, fixture bundles, manual prolific fixture | Partial | Contract/schema/tests are strong. Appen now demonstrates a raw HTML parse path via `scraper`, but most adapters still replay parsed fixture records (`crates/rhof-adapters/src/lib.rs:252`, `crates/rhof-adapters/src/lib.rs:333`). | Roll out raw parser implementations to the remaining adapters incrementally. |
| `PROMPT_05.txt` | Sync orchestration + reports + scheduler + idempotency | `rhof-sync`, DB persistence path, reports/parquet outputs, scheduler command path | Partial | Sync is DB-backed and idempotent for versions (`crates/rhof-sync/src/lib.rs:495`, `crates/rhof-sync/src/lib.rs:676`). Scheduler jobs now execute sync when enabled, but operational hardening is minimal (`crates/rhof-sync/src/lib.rs:525`, `crates/rhof-sync/src/lib.rs:1137`). | Add scheduler metrics/supervision/backoff controls. |
| `PROMPT_06.txt` | Dedup + YAML rules + tests | `rhof-sync` dedup/rules + `rules/*.yaml` + tests | Partial | Dedup/rules logic and tests are present (`crates/rhof-sync/src/lib.rs:214`, `crates/rhof-sync/src/lib.rs:326`). Review items persist (`crates/rhof-sync/src/lib.rs:898`), but dedup cluster tables remain unused. | Persist dedup cluster proposals/members in DB tables. |
| `PROMPT_07.txt` | Parquet exports + manifest + `report daily` CLI | `rhof-sync` parquet/manifest + `rhof-cli report daily` | Delivered | Manifest hashes + Parquet outputs are implemented and routable in reports (`crates/rhof-sync/src/lib.rs:1013`, `crates/rhof-sync/src/lib.rs:1147`). | Document Parquet schemas formally in `docs/`. |
| `PROMPT_08.txt` | Axum routes + Askama + HTMX + Plotly JSON + smoke tests | `rhof-web` routes/templates/tests | Delivered | Route surface and smoke tests match prompt; web loaders now prefer DB-backed sources/opportunities (`crates/rhof-web/src/lib.rs:199`, `crates/rhof-web/src/lib.rs:403`, `crates/rhof-web/src/lib.rs:545`). | Make review resolve endpoint durable (DB update). |
| `PROMPT_09.txt` | CLI commands + seed + CI + guardrails + port policy | `rhof-cli`, CI workflow, adapter checklist, evidence warnings, source badges | Delivered | `migrate`, `sync`, `serve`, `report`, `seed`, `debug`, `scheduler` all exist (`crates/rhof-cli/src/main.rs:13`). SQLx contributor prerequisites are now documented (`README.md:36`, `docs/RUNBOOK.md:10`). | Optional: add CI check for `rhof-cli migrate` against service DB. |
| `PROMPT_10.txt` | Adapter generator + templates + CI contract checks + expansion docs | generator code/templates/check script + runbook/source docs | Delivered | Generator and docs are in place; contract checker now enforces non-empty parsed records and `evidence_coverage_percent >= 90` for enabled sources (`scripts/check_adapters.py:84`, `scripts/check_adapters.py:89`). | Add stronger parser-quality checks (e.g., targeted snapshot test matrix by source). |

## 4. Completeness Score (0–100) + Rubric Breakdown

### Overall Score: **84 / 100**

### A) Core Functionality (0–25): **22 / 25**
- Sync persists DB rows + versions + tags/risk/review and writes reports/parquet (`crates/rhof-sync/src/lib.rs:676`, `crates/rhof-sync/src/lib.rs:823`, `crates/rhof-sync/src/lib.rs:1013`).
- `migrate`, `sync`, `report`, `serve`, and `scheduler` command paths exist (`crates/rhof-cli/src/main.rs:74`, `crates/rhof-cli/src/main.rs:78`).
- Web UI prefers DB-backed reads (`crates/rhof-web/src/lib.rs:403`, `crates/rhof-web/src/lib.rs:441`, `crates/rhof-web/src/lib.rs:545`).
- Remaining gap: adapter realism is uneven (only partial raw parsing demonstrated), and review resolve is not durable yet (`crates/rhof-adapters/src/lib.rs:252`, `crates/rhof-web/src/lib.rs:341`).

### B) Developer Experience (0–20): **17 / 20**
- Quickstart and runbook are accurate (`README.md:13`, `docs/RUNBOOK.md:5`).
- Tailwind bootstrap is reproducible (`justfile:23`, `scripts/install-tailwind.sh:31`).
- `sqlx-prepare` prerequisites are documented (`README.md:38`, `docs/RUNBOOK.md:10`).
- Remaining friction: some workflows still assume manual local tool installs (`cargo-sqlx`, Tailwind binary download).

### C) Tests + Quality Gates (0–15): **13 / 15**
- 16 Rust tests pass across key crates.
- CI runs `fmt`, `clippy`, adapter contract checks, and workspace tests (`.github/workflows/ci.yml:16`, `.github/workflows/ci.yml:27`, `.github/workflows/ci.yml:61`, `.github/workflows/ci.yml:64`).
- Adapter contract checker is stronger now (`scripts/check_adapters.py:84`, `scripts/check_adapters.py:89`).
- Remaining gap: no automated DB integration test in CI for migrations/sync idempotency.

### D) Docs + Examples (0–15): **13 / 15**
- README quickstart, runbook, source notes, and adapter checklist are all useful and current (`README.md:13`, `docs/RUNBOOK.md:12`, `docs/SOURCES.md:1`, `docs/ADAPTER_CHECKLIST.md:1`).
- `docs/ARCHITECTURE.md` and `docs/DATA_MODEL.md` are now populated with as-built details (`docs/ARCHITECTURE.md:3`, `docs/DATA_MODEL.md:3`).
- Remaining gap: missing formal schemas/spec docs for report JSON and Parquet outputs.

### E) Operability + Safety (0–15): **13 / 15**
- Environment-driven config + port policy consistency (`.env.example:1`, `docker-compose.yml:7`).
- Immutable hash-addressed artifacts + atomic writes (`crates/rhof-storage/src/lib.rs:65`).
- Missing-evidence runtime warnings (`crates/rhof-sync/src/lib.rs:1215`).
- Scheduler mode exists (`crates/rhof-sync/src/lib.rs:1137`) but is operationally minimal (no daemon hardening/metrics).

### F) Packaging + Release Readiness (0–10): **6 / 10**
- Workspace version/license metadata present (`Cargo.toml:12`, `Cargo.toml:15`).
- Still missing root `LICENSE` file/changelog/release checklist.
- Distribution story is source-first (`cargo run`) rather than packaged binaries/releases.

### Biggest reason the score is not higher
Adapter implementations are still mostly fixture-replay based, so the project demonstrates architecture, provenance, and persistence well, but not yet broad real parser robustness (`crates/rhof-adapters/src/lib.rs:243`, `crates/rhof-adapters/src/lib.rs:252`).

### Single most leverage improvement to raise it fastest
Implement raw-HTML/JSON parser logic for the remaining initial adapters using the Appen parser path as the template (`crates/rhof-adapters/src/lib.rs:249`, `crates/rhof-adapters/src/lib.rs:397`).

## 5. General Excellence Rating (1–10) + Evidence

### Rating: **8 / 10**

Evidence:
- Prompt-intended architecture is materially reflected in the codebase and runtime behavior, not just scaffolding.
- Provenance-first field model is clear and consistent (`crates/rhof-core/src/lib.rs:20`).
- Artifact storage implementation is robust for this maturity level (`crates/rhof-storage/src/lib.rs:65`, `crates/rhof-storage/src/lib.rs:117`).
- Sync pipeline now includes real DB persistence/versioning and idempotent behavior (`crates/rhof-sync/src/lib.rs:676`).
- Scheduler command path exists and is runnable (`crates/rhof-sync/src/lib.rs:1137`).
- Web UI reads DB-backed data when available, improving alignment with canonical storage (`crates/rhof-web/src/lib.rs:403`, `crates/rhof-web/src/lib.rs:545`).
- Adapter generator/templates/checker and chunking workflow are strong long-term scaling investments (`crates/rhof-adapters/src/lib.rs:428`, `scripts/check_adapters.py:83`, `docs/RUNBOOK.md:33`).
- Docs quality improved materially with architecture/data model content (`docs/ARCHITECTURE.md:3`, `docs/DATA_MODEL.md:3`).
- Frontend baseline polish improved with committed CSS, independent of Tailwind build step (`assets/static/app.css:1`).
- Remaining weakness is adapter realism breadth and incomplete review/dedup lifecycle persistence.

## 6. Priority Issues (P0–P3) (Prompt ID, Problem, Impact, Suggested Fix)

No `P0` or `P1` issues remain.

| Issue ID | Priority | Prompt ID | Problem | Evidence | Impact | Suggested Fix |
|---|---|---|---|---|---|---|
| PFN-014 | P2 | `PROMPT_04`, `PROMPT_10` | Only one adapter (Appen) demonstrates raw fixture parsing; others still replay `parsed_records`. | `crates/rhof-adapters/src/lib.rs:252`, `crates/rhof-adapters/src/lib.rs:430` | Limits confidence in parser resilience across source changes. | Implement raw parser paths for remaining initial adapters incrementally, preserving snapshot tests. |
| PFN-015 | P2 | `PROMPT_06`, `PROMPT_08` | Dedup review lifecycle is partially persisted (`review_items`) but cluster proposal tables are unused and review resolve endpoint is UI-only. | `crates/rhof-sync/src/lib.rs:898`, `migrations/20260223210000_init_schema.up.sql:106`, `crates/rhof-web/src/lib.rs:341` | Incomplete review/audit workflow and underused schema design. | Persist dedup clusters/members and make `/review/:id/resolve` update `review_items`. |
| PFN-016 | P2 | `PROMPT_05`, `PROMPT_09` | Scheduler mode exists, but lacks operational hardening (status reporting/metrics/locking/retry policy). | `crates/rhof-sync/src/lib.rs:525`, `crates/rhof-sync/src/lib.rs:1137` | Suitable for local use, but not yet trustworthy for unattended operation. | Add daemon logging/metrics, optional run locking, and explicit failure handling/backoff policy. |
| PFN-017 | P3 | `PROMPT_10` | `sample-source` generator artifacts still live in repo and can be mistaken for a real adapter implementation outside docs. | `fixtures/sample-source/sample/bundle.json:1`, `crates/rhof-adapters/tests/sample-source_snapshot.rs:1` | Minor surface-area noise for contributors. | Move generator sample artifacts into `examples/` or delete/regenerate in tests as needed. |
| PFN-018 | P3 | Packaging (cross-cutting) | Release/package hygiene is still minimal (no root `LICENSE` file/changelog/release checklist). | `Cargo.toml:15`, root scan (no `LICENSE*` / `CHANGELOG*`) | Lowers external credibility and onboarding confidence. | Add `LICENSE`, `CHANGELOG.md` stub, and a simple release checklist section in README/docs. |

## 7. Overengineering / Complexity Risks (Complexity vs Value)

### Complexity vs Value (Top 10)

| Hotspot | Risk | Value delivered | Simplification recommendation |
|---|---|---|---|
| Six-crate workspace | Med | Clear boundaries, maintainability | Keep; avoid adding more crates until adapter/parser breadth increases. |
| Scheduler abstraction + cron runtime | Med | Real automation path now exists | Keep, but implement one simple hardened daemon mode before adding more scheduler features. |
| Hook abstractions (`DedupHook`, `EnrichmentHook`) | Low | Clear seam/testability | Keep as-is. |
| Dual outputs (DB + report JSON/parquet) | Low | Strong auditability and offline analytics | Keep; document DB as canonical, reports as exports. |
| Parquet export logic embedded in sync crate | Med | Valuable export feature | Refactor into a module/file for clarity, not a new crate. |
| Generator + templates + checker before parser maturity | Low | Strong contributor scaffolding | Keep, but continue tightening quality checks. |
| Unified fixture/manual/seed bundle format | Low | Excellent long-run reuse | Keep; add formal schema/validator later. |
| sqlx runtime queries + optional prepare workflow | Med | Real persistence now, flexible implementation | Either adopt query macros later or clearly document runtime-query approach as intentional. |
| Tailwind standalone binary management | Low | Node-free UX | Keep script-based bootstrap; add checksums if needed. |
| Sample generator artifacts in main tree | Low | Demonstrates generator output | Move to `examples/` or generate in CI-only tests. |

## 8. Naming / Structure / Consistency Findings

### Factual findings
- `sources.yaml` canonical `sources:` shape is implemented and enforced by CI check script (`sources.yaml:1`, `scripts/check_adapters.py:21`).
- Port policy is consistently `5401` across Docker/env/CI/docs (`docker-compose.yml:7`, `.env.example:1`, `.github/workflows/ci.yml:47`, `README.md:15`).
- Prompt CLI naming was aligned in `PROMPT_07` to `rhof-cli` (`.prompts/PROMPT_07.txt:10`).
- `docs/SOURCES.md` now clearly labels `sample-source` as a generator example, not an enabled source (`docs/SOURCES.md:18`).
- Tailwind content globs and local binary bootstrap path are now aligned (`assets/tailwind/tailwind.config.js:2`, `justfile:23`).

### Recommendations
- Decide whether to keep `sample-source` artifacts in-tree or move them to `examples/`.
- Add formal schemas/docs for report JSON + Parquet output contracts.
- Add a small “operational modes” matrix (manual sync vs scheduler vs web-only) to README or runbook.

## 9. Highest-Leverage Next Steps (Top 10) + Estimated Effort (S/M/L)

| # | Next step | Why it matters | Evidence anchor | Effort |
|---|---|---|---|---|
| 1 | Implement raw parser paths for remaining initial adapters | Biggest completeness gain after persistence | `crates/rhof-adapters/src/lib.rs:252`, `crates/rhof-adapters/src/lib.rs:430` | M |
| 2 | Persist dedup clusters/members and wire review lifecycle end-to-end | Completes review/audit workflow and uses existing schema | `migrations/20260223210000_init_schema.up.sql:106`, `crates/rhof-sync/src/lib.rs:898` | M |
| 3 | Make `/review/:id/resolve` durable in Postgres | Aligns UI action with persisted review queue | `crates/rhof-web/src/lib.rs:341` | M |
| 4 | Add scheduler hardening (run locks, retries/metrics/logging) | Moves scheduler from local/dev use toward unattended reliability | `crates/rhof-sync/src/lib.rs:1137` | M |
| 5 | Add CI integration test for `migrate` + sync idempotency | Converts manual verification into regression protection | `crates/rhof-sync/src/lib.rs:1107`, `crates/rhof-sync/src/lib.rs:676` | M |
| 6 | Document report JSON + Parquet schemas | Improves downstream usability and product packaging | `crates/rhof-sync/src/lib.rs:1013` | S |
| 7 | Decide fate of `sample-source` scaffolds (`examples/` vs remove) | Reduces contributor ambiguity | `docs/SOURCES.md:18` | S |
| 8 | Add `LICENSE` file + changelog/release checklist | Improves external packaging credibility | `Cargo.toml:15` | S |
| 9 | Add one-command demo script (`db-up` + `migrate` + `sync` + `serve`) | Sharpens onboarding/demo experience | `README.md:13`, `docs/RUNBOOK.md:5` | S |
| 10 | Expand frontend visual polish via Tailwind build (optional) | Improves front-facing demo quality beyond baseline CSS | `assets/static/app.css:1`, `docs/RUNBOOK.md:45` | S |
