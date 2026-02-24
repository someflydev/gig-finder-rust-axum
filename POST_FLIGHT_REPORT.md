# Post-Flight Report

## 1. Repo Summary (What it is, what it promises, how to run)

### What it is (factual)
RHOF is a Rust workspace for discovering and tracking remote hourly/flexible opportunities with provenance-first capture, a fixture-first adapter framework, a sync/report pipeline, and an Axum/Askama web UI. The workspace defines six crates in `Cargo.toml:1` and `Cargo.toml:2`.

Primary executable entrypoint is `rhof-cli` (`crates/rhof-cli/src/main.rs:37`) with commands `sync`, `report`, `new-adapter`, `seed`, `debug`, `migrate`, and `serve` (`crates/rhof-cli/src/main.rs:13`).

### What it promises (from prompts)
The prompt plan defines RHOF as a production-grade Rust/Axum system with this pipeline: `fetch -> immutable raw artifact -> parse -> normalize -> dedupe -> enrich -> version -> persist`, plus HTMX-first UI, provenance visibility, Parquet snapshots, and ToS-safe adapters (`.prompts/PROMPT_00_s.txt:89`, `.prompts/PROMPT_00_s.txt:92`, `.prompts/PROMPT_00_s.txt:95`).

### How to run locally (as-built, factual)
- Start Postgres on host port `5401`: `just db-up` (`docker-compose.yml:7`, `justfile:11`).
- Apply migrations: `just migrate` (`README.md:19`, `justfile:17`, `crates/rhof-cli/src/main.rs:74`, `crates/rhof-sync/src/lib.rs:1107`).
- Run sync: `cargo run -p rhof-cli -- sync` (`README.md:22`, `crates/rhof-cli/src/main.rs:42`).
- View report summary: `cargo run -p rhof-cli -- report daily --runs 3` (`README.md:24`, `crates/rhof-cli/src/main.rs:50`).
- Start web UI: `cargo run -p rhof-cli -- serve` (`README.md:26`, `crates/rhof-web/src/lib.rs:206`).
- Install Tailwind standalone binary (optional UI styling workflow): `just tailwind-install` (`justfile:23`, `scripts/install-tailwind.sh:1`, `README.md:32`).

### Dependencies / services (factual)
- Rust toolchain + Cargo workspace (`Cargo.toml:1`).
- Postgres via Docker Compose (`docker-compose.yml:2`).
- Python + PyYAML for adapter contract checks in CI (`.github/workflows/ci.yml:57`, `scripts/check_adapters.py:7`).
- Tailwind standalone binary (downloaded by helper script), no Node (`.prompts/PROMPT_00_s.txt:37`, `scripts/install-tailwind.sh:31`).

### Build / test / verification signals (factual)
- CI runs `fmt`, `clippy`, adapter contract checks, and workspace tests (`.github/workflows/ci.yml:8`, `.github/workflows/ci.yml:19`, `.github/workflows/ci.yml:30`, `.github/workflows/ci.yml:61`).
- Local verification during this audit:
  - `cargo build --workspace` passed.
  - `cargo test --workspace` passed (16 Rust tests).
  - `cargo run -p rhof-cli -- migrate` passed after migration idempotency patch.
  - `cargo run -p rhof-cli -- sync` ran twice; DB counts showed `sources=5`, `raw_artifacts=5`, `opportunities=5`, `opportunity_versions=5`, `fetch_runs=2`, demonstrating idempotent version insertion behavior.
  - `_sqlx_migrations` contains one applied migration row (verified via `psql`).

### Demo mode / fixture mode (factual)
The current happy path is still fixture-driven. Sync loads checked-in fixture bundles/manual bundles (not live crawling) and now persists results to Postgres while also writing reports/parquet (`crates/rhof-sync/src/lib.rs:447`, `crates/rhof-sync/src/lib.rs:469`, `crates/rhof-sync/src/lib.rs:495`, `crates/rhof-sync/src/lib.rs:1013`).

## 2. Prompt Intent Map (compressed)

### Scope / vision (extracted)
- Provenance-first RHOF system with immutable artifacts, dedup/review, rules, Parquet snapshots, CLI + web UI, and ToS-safe adapter acquisition (`.prompts/PROMPT_00_s.txt:89`, `.prompts/PROMPT_00_s.txt:99`).

### Non-goals / constraints / quality bars (extracted)
- Repo-root generation, no nested repo (`.prompts/PROMPT_00_s.txt:16`).
- Postgres host port `5401` (`.prompts/PROMPT_00_s.txt:45`, `.prompts/PROMPT_01.txt:56`).
- Tailwind standalone binary, no Node (`.prompts/PROMPT_00_s.txt:37`, `.prompts/PROMPT_01.txt:74`).
- Fixture + snapshot test discipline for adapters (`.prompts/PROMPT_04.txt:34`, `.prompts/PROMPT_04.txt:35`).
- Data-driven YAML rules (`.prompts/PROMPT_06.txt:14`).
- Chunked adapter expansion with gates (`.prompts/PROMPT_10.txt:23`, `.prompts/PROMPT_10.txt:33`).

### Sequencing assumptions (observed)
- Prompt 05 intentionally wires dedup/enrichment hooks before Prompt 06 fills behavior (`.prompts/PROMPT_05.txt:3`, `.prompts/PROMPT_05.txt:5`).
- Prompt 07 introduces `report`; Prompt 09 extends the same CLI (`.prompts/PROMPT_07.txt:12`, `.prompts/PROMPT_09.txt:10`).
- Prompt 10 depends on authoritative `sources.yaml` schema from Prompt 01 (`.prompts/PROMPT_01.txt:38`, `.prompts/PROMPT_10.txt:24`).

### Prompt-by-prompt intent map
- `PROMPT_00_s.txt`: execution contract, stack, mission, ports, non-negotiables.
- `PROMPT_01.txt`: repo-root workspace layout + Docker/Just/env/docs/rules/`sources.yaml`.
- `PROMPT_02.txt`: core provenance/domain model + Postgres schema migration.
- `PROMPT_03.txt`: artifact store + HTTP client + retry/rate-limit + tests.
- `PROMPT_04.txt`: adapter trait + 5 initial adapters + unified fixture/manual schema + snapshot tests.
- `PROMPT_05.txt`: sync orchestration, scheduler seam, reports, idempotency.
- `PROMPT_06.txt`: dedup + YAML rules + tests.
- `PROMPT_07.txt`: Parquet snapshots + manifest + report CLI.
- `PROMPT_08.txt`: Axum web routes + Askama + HTMX + Plotly JSON + smoke tests.
- `PROMPT_09.txt`: CLI command surface, seed semantics, CI, runtime guardrails.
- `PROMPT_10.txt`: adapter generator/templates, CI contract checks, chunked expansion docs/workflow.

## 3. Traceability: Prompt -> Artifact Delivery Table

| Prompt ID | Intended artifacts | Found artifacts | Status | Notes | Suggested follow-up |
|---|---|---|---|---|---|
| `PROMPT_00_s.txt` | Execution contract, stack/mission constraints | `.prompts/PROMPT_00_s.txt`, repo-root workspace, prompt preservation, runnable CLI/web | Partial | Core contract now substantially met for DB persistence/migrations, but scheduler automation and live adapter fetching are still incomplete (`crates/rhof-sync/src/lib.rs:525`, `crates/rhof-adapters/src/lib.rs:234`). | Finish scheduler execution mode and at least one real fetch/parse implementation. |
| `PROMPT_01.txt` | Workspace layout + Docker + justfile + env + Tailwind workflow | Root files/dirs exist, `justfile`, `.env.example`, `README.md`, `sources.yaml`, `rules/*`, `docs/*`, `assets/*`, `crates/*` | Delivered | Tailwind bootstrap gap closed with `just tailwind-install` + install script (`justfile:23`, `scripts/install-tailwind.sh:31`). | Add checksum verification to Tailwind installer script (optional hardening). |
| `PROMPT_02.txt` | Core domain model + migrations + migrate step | `crates/rhof-core/src/lib.rs`, `migrations/*`, sqlx migration entrypoint in `rhof-sync`, `rhof-cli migrate` | Delivered | `Field<T>`, `EvidenceRef`, `OpportunityDraft`, `Opportunity` implemented (`crates/rhof-core/src/lib.rs:11`, `crates/rhof-core/src/lib.rs:45`). Migrations now runnable/idempotent on pre-existing schema (`crates/rhof-sync/src/lib.rs:31`, `crates/rhof-sync/src/lib.rs:1107`, `migrations/20260223210000_init_schema.up.sql:3`). | Add integration test covering `migrate` against a temporary Postgres instance. |
| `PROMPT_03.txt` | Immutable artifact store + HTTP fetch utilities + tests | `crates/rhof-storage/src/lib.rs` | Delivered | Hashing, atomic writes, dedupe, backoff, semaphores/token bucket, tracing spans, and tests all present (`crates/rhof-storage/src/lib.rs:44`, `crates/rhof-storage/src/lib.rs:65`, `crates/rhof-storage/src/lib.rs:272`, `crates/rhof-storage/src/lib.rs:397`). | Add integration tests for retry classifications against a local HTTP test server. |
| `PROMPT_04.txt` | Adapter contract + crawlability + 5 adapters + fixtures/manual schema + snapshot tests | `crates/rhof-adapters/src/lib.rs`, `fixtures/*`, `manual/prolific/sample.json` | Partial | Contract/schema/tests are strong and deterministic (`crates/rhof-adapters/src/lib.rs:58`, `crates/rhof-adapters/src/lib.rs:82`, `crates/rhof-adapters/src/lib.rs:475`). Adapters still replay pre-parsed fixture records and no-op fetch methods (`crates/rhof-adapters/src/lib.rs:234`, `crates/rhof-adapters/src/lib.rs:243`). | Implement one raw fixture parser path (HTML/JSON -> canonical fields) to prove adapter realism. |
| `PROMPT_05.txt` | Full sync orchestration + reports + scheduler + idempotency | `crates/rhof-sync/src/lib.rs`, `reports/*`, `artifacts/*`, Postgres persistence | Partial | Sync now upserts sources/fetch runs/opportunities/opportunity_versions/tags/risk/review and remains idempotent for versions (`crates/rhof-sync/src/lib.rs:447`, `crates/rhof-sync/src/lib.rs:495`, `crates/rhof-sync/src/lib.rs:662`, `crates/rhof-sync/src/lib.rs:807`). Scheduler builder still logs and does not invoke sync (`crates/rhof-sync/src/lib.rs:525`). | Add a scheduler/daemon command that runs jobs for real. |
| `PROMPT_06.txt` | Dedup + YAML rules + tests | `crates/rhof-sync/src/lib.rs`, `rules/*.yaml` | Partial | Dedup thresholds/tests and YAML rules are implemented (`crates/rhof-sync/src/lib.rs:214`, `crates/rhof-sync/src/lib.rs:1053`, `crates/rhof-sync/src/lib.rs:326`). Review items are persisted when flagged (`crates/rhof-sync/src/lib.rs:884`), but dedup cluster tables are still unused. | Persist cluster proposals/members into `dedup_clusters` and `dedup_cluster_members`. |
| `PROMPT_07.txt` | Parquet exports + manifest + `report daily` CLI | `crates/rhof-sync/src/lib.rs`, `crates/rhof-cli/src/main.rs` | Delivered | Parquet snapshots + hash manifest exist (`crates/rhof-sync/src/lib.rs:639`, `crates/rhof-sync/src/lib.rs:994`) and `report daily` CLI is wired (`crates/rhof-cli/src/main.rs:49`, `crates/rhof-sync/src/lib.rs:1147`). | Document Parquet schemas in `docs/` for downstream consumers. |
| `PROMPT_08.txt` | Axum routes + Askama + HTMX + Plotly JSON + smoke tests | `crates/rhof-web/src/lib.rs`, `crates/rhof-web/templates/*` | Delivered | Routes/partials/tests are present (`crates/rhof-web/src/lib.rs:190`, `crates/rhof-web/src/lib.rs:247`, `crates/rhof-web/src/lib.rs:270`, `crates/rhof-web/src/lib.rs:619`). Web loader now prefers DB-backed sources/opportunities with report/YAML fallback (`crates/rhof-web/src/lib.rs:403`, `crates/rhof-web/src/lib.rs:441`, `crates/rhof-web/src/lib.rs:545`). | Add persisted review resolution and provenance-detail rendering. |
| `PROMPT_09.txt` | CLI commands + seed + CI + guardrails + port policy | `crates/rhof-cli/src/main.rs`, `.github/workflows/ci.yml`, `docs/ADAPTER_CHECKLIST.md`, `scripts/check_adapters.py`, `crates/rhof-sync/src/lib.rs`, web source badges | Delivered | `migrate` is now real (`crates/rhof-cli/src/main.rs:74`) and seed reuses the DB-backed sync path (`crates/rhof-sync/src/lib.rs:1124`). CI and guardrails remain in place (`.github/workflows/ci.yml:61`, `docs/ADAPTER_CHECKLIST.md:1`, `crates/rhof-sync/src/lib.rs:1215`, `crates/rhof-web/templates/sources.html:17`). | Add explicit sqlx CLI install docs if `just sqlx-prepare` should be a contributor expectation. |
| `PROMPT_10.txt` | Adapter generator + templates + contract CI + expansion docs | `crates/rhof-adapters/src/lib.rs`, `templates/adapter/*`, `scripts/check_adapters.py`, `docs/RUNBOOK.md`, `docs/SOURCES.md` | Partial | Generator/templates/docs/checker are implemented (`crates/rhof-adapters/src/lib.rs:328`, `templates/adapter/bundle.json.tmpl:1`, `docs/RUNBOOK.md:26`). CI checker remains presence-focused and does not enforce evidence coverage or parser quality (`scripts/check_adapters.py:72`, `scripts/check_adapters.py:94`). | Strengthen CI checks for evidence coverage threshold and parser execution quality. |

## 4. Completeness Score (0–100) + Rubric Breakdown

### Overall Score: **78 / 100**

### A) Core Functionality (0–25): **21 / 25**
Evidence:
- Happy-path sync persists DB rows and writes reports/parquet (`crates/rhof-sync/src/lib.rs:447`, `crates/rhof-sync/src/lib.rs:495`, `crates/rhof-sync/src/lib.rs:662`, `crates/rhof-sync/src/lib.rs:1013`).
- `rhof-cli migrate` and `rhof-cli sync` are runnable (`crates/rhof-cli/src/main.rs:74`, `crates/rhof-cli/src/main.rs:42`).
- Web UI now prefers DB-backed reads for sources/opportunities (`crates/rhof-web/src/lib.rs:403`, `crates/rhof-web/src/lib.rs:441`, `crates/rhof-web/src/lib.rs:545`).
- Remaining core gap: scheduler jobs do not execute sync, and adapters do not yet parse raw artifacts directly (`crates/rhof-sync/src/lib.rs:525`, `crates/rhof-adapters/src/lib.rs:234`).

### B) Developer Experience (0–20): **16 / 20**
Evidence:
- Quickstart uses `just migrate` and current real commands (`README.md:19`, `README.md:22`).
- `just tailwind-install` closes the standalone binary bootstrap gap (`justfile:23`, `scripts/install-tailwind.sh:31`).
- `justfile` command surface is coherent (`justfile:3`).
- Remaining friction: `just sqlx-prepare` assumes contributors installed `cargo-sqlx`, and this is not documented in README/runbook (`justfile:20`).

### C) Tests + Quality Gates (0–15): **12 / 15**
Evidence:
- 16 Rust tests across storage/adapters/sync/web, all passing.
- CI runs `fmt`, `clippy`, adapter contract check, and tests (`.github/workflows/ci.yml:16`, `.github/workflows/ci.yml:27`, `.github/workflows/ci.yml:61`, `.github/workflows/ci.yml:64`).
- Remaining gap: no automated integration test for migration + DB persistence idempotency path (verified manually in this audit).

### D) Docs + Examples (0–15): **10 / 15**
Evidence:
- README quickstart and runbook workflows are accurate and improved (`README.md:13`, `docs/RUNBOOK.md:5`).
- Source/adapters docs and checklist exist (`docs/SOURCES.md:1`, `docs/ADAPTER_CHECKLIST.md:1`).
- `docs/ARCHITECTURE.md` and `docs/DATA_MODEL.md` are still placeholders (`docs/ARCHITECTURE.md:3`, `docs/DATA_MODEL.md:3`).

### E) Operability + Safety (0–15): **13 / 15**
Evidence:
- Config is environment-driven (`.env.example:1`, `crates/rhof-sync/src/lib.rs:66`).
- Artifact storage is hash-addressed + atomic (`crates/rhof-storage/src/lib.rs:65`).
- Missing evidence warnings exist (`crates/rhof-sync/src/lib.rs:1215`).
- Seed/sync version insertion is idempotent by stable keys/content-derived rows in practice (verified with repeated sync; `opportunity_versions` remained at 5 rows).
- Remaining gap: scheduler config exists but no executing daemon mode (`crates/rhof-sync/src/lib.rs:525`).

### F) Packaging + Release Readiness (0–10): **6 / 10**
Evidence:
- Workspace version/license metadata present (`Cargo.toml:12`, `Cargo.toml:15`).
- No `LICENSE` file or changelog file in repo root.
- Distribution story is still source-first (`cargo run`) with no release checklist.

### Biggest reason the score is not higher
The adapter layer is still fixture-replay based (no proven raw HTML/JSON parser implementation in the adapters), so the repo demonstrates architecture and regression discipline more than real-world scraping resilience (`crates/rhof-adapters/src/lib.rs:234`, `crates/rhof-adapters/src/lib.rs:243`).

### Single most leverage improvement to raise it fastest
Implement one real adapter parser (raw fixture -> parsed canonical fields) and keep the existing snapshot tests, then promote that pattern as the standard for all sources.

## 5. General Excellence Rating (1–10) + Evidence

### Rating: **7 / 10** (solid, credible project with meaningful runtime behavior; still prototype-level in adapter realism and ops automation)

Evidence:
- Strong modular workspace boundaries with clear crate roles (`Cargo.toml:2`).
- Provenance-first data model is explicit and reusable (`crates/rhof-core/src/lib.rs:20`, `crates/rhof-core/src/lib.rs:43`).
- Artifact storage implementation is notably robust for an early-stage project (`crates/rhof-storage/src/lib.rs:44`, `crates/rhof-storage/src/lib.rs:117`, `crates/rhof-storage/src/lib.rs:344`).
- Fixture-first adapters + snapshot tests provide deterministic regression coverage (`crates/rhof-adapters/src/lib.rs:82`, `crates/rhof-adapters/src/lib.rs:561`).
- Sync pipeline now performs real DB persistence + versioning path, not just report generation (`crates/rhof-sync/src/lib.rs:662`, `crates/rhof-sync/src/lib.rs:807`).
- `rhof-cli migrate` is implemented via sqlx embedded migrations and validated (`crates/rhof-cli/src/main.rs:74`, `crates/rhof-sync/src/lib.rs:1107`).
- Web UI is complete for the prompt’s route/HTMX scope and now reads DB-backed data when available (`crates/rhof-web/src/lib.rs:190`, `crates/rhof-web/src/lib.rs:403`).
- CI gates are meaningful and include non-Rust adapter contract validation (`.github/workflows/ci.yml:61`).
- Scheduler is still a non-executing scaffold (`crates/rhof-sync/src/lib.rs:525`).
- Architecture/data model docs remain placeholders (`docs/ARCHITECTURE.md:3`, `docs/DATA_MODEL.md:3`).
- Adapter implementations are still fixture replay, not raw parser logic (`crates/rhof-adapters/src/lib.rs:243`).

## 6. Priority Issues (P0–P3) (Prompt ID, Problem, Impact, Suggested Fix)

No `P0` or `P1` issues remain from the prior audit round after the applied fixes.

| Issue ID | Priority | Prompt ID | Problem | Evidence | Impact | Suggested Fix |
|---|---|---|---|---|---|---|
| PFN-006 | P2 | `PROMPT_05` | Scheduler is scaffolded but does not run sync jobs even when enabled. | `crates/rhof-sync/src/lib.rs:525` | `RHOF_SCHEDULER_ENABLED` suggests automation that is not actually operational. | Add a `daemon`/`scheduler` CLI mode that starts jobs and invokes sync on triggers. |
| PFN-007 | P2 | `PROMPT_04`, `PROMPT_10` | Adapters still replay `parsed_records` fixtures rather than parsing raw artifacts. | `crates/rhof-adapters/src/lib.rs:234`, `crates/rhof-adapters/src/lib.rs:243` | Reduces confidence in real-world source resilience and selector/parser quality. | Implement one real raw parser path (HTML/JSON fixture parsing) and use it as the pattern. |
| PFN-008 | P2 | `PROMPT_10` | Adapter contract CI checks are presence-based and do not enforce evidence coverage threshold/parser correctness. | `scripts/check_adapters.py:72`, `scripts/check_adapters.py:94` | Placeholder adapters can still pass CI. | Enforce `evidence_coverage_percent >= 90` and run source-specific snapshot tests in CI. |
| PFN-013 | P2 | `PROMPT_00_s`, `PROMPT_09` | `sqlx-prepare` target exists, but there is no documented `cargo-sqlx` setup and no query-macro workflow yet. | `.prompts/PROMPT_00_s.txt:109`, `justfile:20`, `crates/rhof-sync/Cargo.toml:15` | Contributor confusion around expected sqlx workflow and query safety posture. | Document `cargo install sqlx-cli ...`, or defer `sqlx-prepare` until query macros are adopted. |
| PFN-009 | P2 | `PROMPT_07`, `PROMPT_09` | Prompt/docs command naming is inconsistent (`rhof` vs `rhof-cli`). | `.prompts/PROMPT_07.txt:10`, `crates/rhof-cli/Cargo.toml:8` | Small onboarding friction. | Standardize on one binary name in prompts/docs (or rename the binary). |
| PFN-010 | P2 | `PROMPT_01`, `PROMPT_02` | `docs/ARCHITECTURE.md` and `docs/DATA_MODEL.md` are placeholders. | `docs/ARCHITECTURE.md:3`, `docs/DATA_MODEL.md:3` | Weakens design traceability and onboarding. | Replace placeholders with as-built architecture/data docs and note remaining gaps. |
| PFN-011 | P3 | `PROMPT_10` | Generated `sample-source` scaffold artifacts are committed and can look like a supported source. | `docs/SOURCES.md:18`, `fixtures/sample-source/sample/bundle.json:1` | Minor credibility/maintenance clutter. | Move sample output to `examples/` or remove after generator validation. |
| PFN-012 | P3 | `PROMPT_08` | Frontend is still visually minimal and CSS is placeholder until users run Tailwind build. | `crates/rhof-web/templates/index.html:11`, `assets/static/app.css:1` | Lower demo polish. | Add a small design pass and commit generated CSS after `just tailwind-install && just tailwind`. |

## 7. Overengineering / Complexity Risks (Complexity vs Value)

### Complexity vs Value (Top 10 hotspots)

| Hotspot | Risk | Value delivered | Simplification recommendation |
|---|---|---|---|
| Six-crate workspace (`crates/*`) before mature adapter logic | Med | Clear boundaries, future scalability | Keep current crate split; avoid further crate proliferation until adapter + scheduler maturity improves. |
| Scheduler abstraction without executable mode (`maybe_build_scheduler`) | Med | Future-ready scheduling seam | Add one concrete daemon mode or remove scheduler knobs from docs until implemented. |
| Hook abstraction (`DedupHook`, `EnrichmentHook`) + noop variants | Low | Clean Prompt 05/06 seam and testability | Keep; revisit only if no alternate hook implementations appear. |
| Dual outputs (DB + report JSON/parquet) | Low | Valuable audit/report exports and offline analytics | Keep both, but document DB as canonical and reports as generated artifacts. |
| Parquet export code embedded in sync crate | Med | Strong analytics/export value | Move to a dedicated module/file for maintainability, not a new crate yet. |
| Adapter generator + templates + checker before parser maturity | Low | Excellent contribution scaffolding | Keep, but strengthen checks so placeholders don’t masquerade as complete adapters. |
| Unified fixture/manual/seed bundle schema | Low | Long-term reuse and deterministic workflows | Keep; consider adding a JSON schema or validator command later. |
| sqlx migration + runtime wiring without query macros | Med | Real migrations/persistence now work | Either adopt query macros and `sqlx prepare`, or document runtime-query approach clearly for now. |
| Tailwind standalone binary management | Low | Node-free frontend workflow | Keep script-based install; add checksum verification and version pin docs. |
| Sample generated artifacts checked into main tree | Low | Demonstrates generator output | Move example scaffolds under `examples/` to reduce product-surface ambiguity. |

## 8. Naming / Structure / Consistency Findings

### Factual findings
- `sources.yaml` uses and CI-enforces the canonical top-level `sources:` shape (`sources.yaml:1`, `scripts/check_adapters.py:21`).
- Port policy is consistently `5401` in Docker/CI/env/docs (`docker-compose.yml:7`, `.env.example:1`, `.github/workflows/ci.yml:47`, `README.md:15`).
- Binary name remains `rhof-cli`, while some prompt text still uses `rhof` (`crates/rhof-cli/Cargo.toml:8`, `.prompts/PROMPT_07.txt:10`).
- Tailwind content glob mismatch was fixed (`assets/tailwind/tailwind.config.js:2`).
- `docs/SOURCES.md` still mixes implemented sources and generator sample scaffold docs (`docs/SOURCES.md:5`, `docs/SOURCES.md:18`).

### Recommendations
- Standardize CLI naming across prompts/docs/code (`rhof` vs `rhof-cli`).
- Separate generator demo artifacts from the authoritative source implementation list.
- Fill architecture/data model docs with as-built content and roadmap notes.
- Add one contributor-facing “tooling prerequisites” section (`cargo-sqlx`, Tailwind standalone installer, Docker, psql optional).

## 9. Highest-Leverage Next Steps (Top 10) + Estimated Effort (S/M/L)

| # | Next step | Why it matters | Evidence anchor | Effort |
|---|---|---|---|---|
| 1 | Implement one real raw artifact parser in an adapter (fixture raw HTML/JSON -> canonical fields) | Biggest trust/completeness gain after DB persistence | `crates/rhof-adapters/src/lib.rs:234`, `crates/rhof-adapters/src/lib.rs:243` | M |
| 2 | Add scheduler/daemon CLI mode that executes sync on cron triggers | Makes scheduler config operational | `crates/rhof-sync/src/lib.rs:525` | M |
| 3 | Strengthen adapter CI checks (coverage threshold + parser/snapshot execution) | Prevents placeholder adapter regressions | `scripts/check_adapters.py:72`, `scripts/check_adapters.py:94` | M |
| 4 | Fill `docs/ARCHITECTURE.md` and `docs/DATA_MODEL.md` with as-built details | Improves onboarding and design traceability | `docs/ARCHITECTURE.md:3`, `docs/DATA_MODEL.md:3` | M |
| 5 | Persist dedup clusters/members into dedicated tables | Completes Prompt 06 data model usage | `migrations/20260223210000_init_schema.up.sql:106`, `crates/rhof-sync/src/lib.rs:214` | M |
| 6 | Implement durable review resolution semantics (DB-backed POST /review/:id/resolve) | Aligns review UI with persisted `review_items` | `crates/rhof-web/src/lib.rs:339`, `crates/rhof-sync/src/lib.rs:884` | M |
| 7 | Document and validate `sqlx-prepare` contributor workflow (or defer it) | Reduces sqlx tooling confusion | `justfile:20`, `.prompts/PROMPT_00_s.txt:109` | S |
| 8 | Standardize binary naming (`rhof` vs `rhof-cli`) | Cleans docs/prompt consistency | `crates/rhof-cli/Cargo.toml:8`, `.prompts/PROMPT_07.txt:10` | S |
| 9 | Move/remove `sample-source` generated scaffold from primary docs surface | Improves perceived completeness of supported source list | `docs/SOURCES.md:18` | S |
| 10 | Add release/package hygiene (`LICENSE` file, changelog stub, release checklist) | Improves external credibility and packaging readiness | `Cargo.toml:15` | S |
