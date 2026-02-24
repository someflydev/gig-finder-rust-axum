# Post-Flight Report

## 1. Repo Summary (What it is, what it promises, how to run)

### What It Is (factual)
RHOF is a Rust workspace for a provenance-first remote opportunity discovery system with a CLI, sync/report pipeline, adapter framework, and an Axum/Askama web UI. The repo declares six crates in a Cargo workspace (`crates/rhof-core`, `crates/rhof-storage`, `crates/rhof-adapters`, `crates/rhof-sync`, `crates/rhof-web`, `crates/rhof-cli`) in `Cargo.toml:1` and `Cargo.toml:2`.

Primary executable entrypoint is the CLI binary `rhof-cli` in `crates/rhof-cli/src/main.rs:37` with commands `sync`, `report daily`, `new-adapter`, `seed`, `debug`, `migrate`, and `serve` in `crates/rhof-cli/src/main.rs:13`.

### What It Promises (factual from prompt plan)
The prompt system defines RHOF as a production-grade Rust/Axum implementation that should fetch -> store immutable artifacts -> parse -> normalize -> dedupe -> enrich -> version -> persist, expose provenance-first UI, schedule twice daily, and export Parquet snapshots (`.prompts/PROMPT_00_s.txt:89`, `.prompts/PROMPT_00_s.txt:92`, `.prompts/PROMPT_00_s.txt:95`).

### Current Build/Run Signals (factual)
- Local Postgres via Docker Compose on host port `5401` in `docker-compose.yml:7`.
- Env defaults documented in `.env.example:1`.
- Quickstart commands documented in `README.md:13`.
- `justfile` includes `db-up`, `sync`, `serve`, `test`, `fmt`, `lint`, `migrate`, and `tailwind` in `justfile:11`, `justfile:23`, `justfile:26`, `justfile:29`, `justfile:32`, `justfile:35`, `justfile:17`, `justfile:38`.
- CI runs `fmt`, `clippy`, adapter contract check, and `cargo test --workspace` in `.github/workflows/ci.yml:8`, `.github/workflows/ci.yml:19`, `.github/workflows/ci.yml:30`, `.github/workflows/ci.yml:61`, `.github/workflows/ci.yml:64`.

### How To Run (as-built, factual)
- Start Postgres: `just db-up` (`justfile:11`, `docker-compose.yml:1`).
- Apply migration manually with `psql` as documented because CLI migrate is stubbed (`README.md:19`, `crates/rhof-cli/src/main.rs:74`).
- Run sync: `cargo run -p rhof-cli -- sync` (`README.md:22`, `crates/rhof-cli/src/main.rs:42`).
- Run reports: `cargo run -p rhof-cli -- report daily --runs 3` (`README.md:24`, `crates/rhof-cli/src/main.rs:50`).
- Run web UI: `cargo run -p rhof-cli -- serve` (`README.md:26`, `crates/rhof-cli/src/main.rs:77`, `crates/rhof-web/src/lib.rs:204`).

### Dependencies / Services (factual)
- Rust workspace (`Cargo.toml:1`).
- Postgres service (`docker-compose.yml:2`).
- Python + PyYAML used in CI adapter contract script (`.github/workflows/ci.yml:57`, `scripts/check_adapters.py:7`).
- Tailwind standalone binary is assumed but not vendored (`.prompts/PROMPT_00_s.txt:37`, `justfile:38`, `assets/static/app.css:1`).

### Demo / Fixture Mode (factual)
The current happy path is fixture-driven. Sync loads checked-in fixture bundles and manual JSON bundles rather than crawling live sites in the pipeline (`crates/rhof-sync/src/lib.rs:450`, `crates/rhof-sync/src/lib.rs:452`, `crates/rhof-sync/src/lib.rs:454`, `crates/rhof-adapters/src/lib.rs:140`, `crates/rhof-adapters/src/lib.rs:144`).

## 2. Prompt Intent Map (compressed)

### Scope / Vision (extracted)
- Build a production-grade Rust/Axum RHOF system with provenance-first extraction, dedup/review, rules-based enrichment, Parquet snapshots, CLI + web UI, and ToS-safe adapters (`.prompts/PROMPT_00_s.txt:13`, `.prompts/PROMPT_00_s.txt:89`, `.prompts/PROMPT_00_s.txt:99`).

### Constraints / Quality Bars (extracted)
- Repo-root generation, no nested git repo (`.prompts/PROMPT_00_s.txt:16`).
- Postgres host port `5401` only (`.prompts/PROMPT_00_s.txt:45`, `.prompts/PROMPT_00_s.txt:80`).
- Tailwind standalone binary, no Node (`.prompts/PROMPT_00_s.txt:37`, `.prompts/PROMPT_01.txt:74`).
- Fixtures + snapshot tests required for adapters (`.prompts/PROMPT_04.txt:34`, `.prompts/PROMPT_04.txt:35`).
- Data-driven rules (`.prompts/PROMPT_06.txt:14`).
- CI checks + adapter quality guardrails (`.prompts/PROMPT_09.txt:20`, `.prompts/PROMPT_10.txt:45`).

### Sequencing Assumptions / Hazards (observed in prompt plan)
- Prompt 05 wires pipeline and leaves dedup/rules behavior to Prompt 06 by design (`.prompts/PROMPT_05.txt:3`, `.prompts/PROMPT_05.txt:5`).
- Prompt 07 introduces `report`; Prompt 09 extends the same CLI tree (`.prompts/PROMPT_07.txt:12`, `.prompts/PROMPT_09.txt:10`).
- Prompt 10 depends on authoritative `sources.yaml` schema and chunking discipline (`.prompts/PROMPT_10.txt:23`, `.prompts/PROMPT_10.txt:24`, `.prompts/PROMPT_10.txt:33`).

### Prompt-by-Prompt Intent Map
- `PROMPT_00_s.txt`: Execution contract, fixed stack, mission, non-negotiables, port policy.
- `PROMPT_01.txt`: Create repo-root Cargo workspace, layout, `sources.yaml`, Docker/Just/env/Tailwind scaffolding.
- `PROMPT_02.txt`: Canonical domain model (`Field`, `EvidenceRef`, `OpportunityDraft`, `Opportunity`) and Postgres schema migrations.
- `PROMPT_03.txt`: Immutable artifact store + HTTP fetcher with retries/rate limits + tests.
- `PROMPT_04.txt`: `SourceAdapter` trait, crawlability enum, 5 initial adapters, unified fixture/manual schema, snapshot tests.
- `PROMPT_05.txt`: End-to-end sync orchestration, `sources.yaml` loading, reports, scheduler hooks, idempotency.
- `PROMPT_06.txt`: Dedup (Jaro-Winkler thresholds/review queue) + YAML-driven rules + tests.
- `PROMPT_07.txt`: Parquet exports + manifest and CLI `report daily --runs N`.
- `PROMPT_08.txt`: Axum web routes, Askama, HTMX partials, Plotly JSON, smoke tests.
- `PROMPT_09.txt`: Full CLI command surface, CI jobs, guardrails, port policy enforcement, `seed` semantics.
- `PROMPT_10.txt`: Adapter generator/templates, CI contract checks, chunked source expansion workflow, docs.

## 3. Traceability: Prompt -> Artifact Delivery Table

| Prompt ID | Intended artifacts | Found artifacts | Status | Notes | Suggested follow-up |
|---|---|---|---|---|---|
| `PROMPT_00_s.txt` | Execution contract, stack/mission constraints, compile/run quality bar | `.prompts/PROMPT_00_s.txt`, repo generated at root, Cargo workspace, prompt preservation | Partial | Repo-root + no nested git honored (`.prompts/PROMPT_00_s.txt:16`), but key mission pieces “version -> persist” are not fully implemented in runtime path (`crates/rhof-sync/src/lib.rs:602`). | Treat as contract debt tracker: finish DB persistence/versioning and `migrate` command. |
| `PROMPT_01.txt` | Workspace layout, Docker, justfile, `.env.example`, `README.md`, `sources.yaml`, rules | `Cargo.toml`, `docker-compose.yml`, `justfile`, `.env.example`, `README.md`, `sources.yaml`, `rules/*.yaml`, `docs/*`, `assets/*`, `crates/*` | Partial | Layout exists and port 5401 is consistent (`docker-compose.yml:7`, `.env.example:1`). `just tailwind` assumes missing `./bin/tailwindcss` (`justfile:38`). | Add tailwind binary bootstrap/download step or vendored binary instructions/script. |
| `PROMPT_02.txt` | Domain model + Postgres schema migration | `crates/rhof-core/src/lib.rs`, `migrations/20260223210000_init_schema.*.sql` | Partial | Core types delivered (`crates/rhof-core/src/lib.rs:11`, `crates/rhof-core/src/lib.rs:45`). Migration SQL defines requested tables/indexes (`migrations/20260223210000_init_schema.up.sql:3`, `migrations/20260223210000_init_schema.up.sql:125`). CLI migrate path remains stubbed (`crates/rhof-cli/src/main.rs:74`). | Implement `rhof-cli migrate` or remove `just migrate` until wired. |
| `PROMPT_03.txt` | Immutable artifact store + HTTP fetcher/retries/rate limits + tests | `crates/rhof-storage/src/lib.rs` | Delivered | Hashing, atomic rename, dedupe pathing, retry/backoff classification, semaphores, token bucket, tracing spans are present (`crates/rhof-storage/src/lib.rs:44`, `crates/rhof-storage/src/lib.rs:65`, `crates/rhof-storage/src/lib.rs:155`, `crates/rhof-storage/src/lib.rs:272`, `crates/rhof-storage/src/lib.rs:344`). Tests exist (`crates/rhof-storage/src/lib.rs:397`). | Add integration test against a local test server for retryable/non-retryable HTTP statuses. |
| `PROMPT_04.txt` | Adapter contract + crawlability + 5 adapters + fixtures/manual schema + snapshot tests | `crates/rhof-adapters/src/lib.rs`, `fixtures/*`, `manual/prolific/sample.json` | Partial | Trait + schema + evidence mapping + 5 adapters + snapshot tests exist (`crates/rhof-adapters/src/lib.rs:58`, `crates/rhof-adapters/src/lib.rs:82`, `crates/rhof-adapters/src/lib.rs:302`, `crates/rhof-adapters/src/lib.rs:475`). Fetch methods are no-op and parsing reads pre-parsed fixture records (`crates/rhof-adapters/src/lib.rs:234`, `crates/rhof-adapters/src/lib.rs:243`). | Implement real parse-from-raw extraction for at least one public source to prove adapter shape beyond fixture replay. |
| `PROMPT_05.txt` | Full sync orchestration, source iteration, dedup/enrich hooks, reports, scheduler, idempotency | `crates/rhof-sync/src/lib.rs`, `reports/*`, `artifacts/*` | Partial | End-to-end fixture pipeline runs and writes reports/parquet (`crates/rhof-sync/src/lib.rs:436`, `crates/rhof-sync/src/lib.rs:485`, `crates/rhof-sync/src/lib.rs:639`). DB upsert/version tracking not implemented; persistence explicitly marked staged only (`crates/rhof-sync/src/lib.rs:602`). Scheduler builder exists but does not run sync jobs (`crates/rhof-sync/src/lib.rs:503`, `crates/rhof-sync/src/lib.rs:512`). | Implement DB persistence + make scheduler execute `run_sync_once_from_env()` when enabled. |
| `PROMPT_06.txt` | Dedup engine + YAML rules + tests | `crates/rhof-sync/src/lib.rs`, `rules/*.yaml` | Partial | Jaro-Winkler dedup thresholds and tests exist (`crates/rhof-sync/src/lib.rs:208`, `crates/rhof-sync/src/lib.rs:1053`). YAML rules load from files and apply tags/risk/pay hints (`crates/rhof-sync/src/lib.rs:326`, `crates/rhof-sync/src/lib.rs:348`). Dedup clusters/review queue are not persisted to DB tables yet. | Persist dedup proposals/review items to schema introduced in Prompt 02. |
| `PROMPT_07.txt` | Parquet exports + manifest + CLI report daily | `crates/rhof-sync/src/lib.rs`, `crates/rhof-cli/src/main.rs` | Delivered | Parquet files and manifest generation are implemented (`crates/rhof-sync/src/lib.rs:651`, `crates/rhof-sync/src/lib.rs:671`); `report daily --runs` is wired (`crates/rhof-cli/src/main.rs:49`, `crates/rhof-sync/src/lib.rs:713`). | Add schema docs/examples for Parquet output and manifest fields. |
| `PROMPT_08.txt` | Axum server routes, Askama, HTMX, Plotly JSON, CSS asset, smoke tests | `crates/rhof-web/src/lib.rs`, `crates/rhof-web/templates/*` | Delivered | All requested routes and CSS asset route exist (`crates/rhof-web/src/lib.rs:190`, `crates/rhof-web/src/lib.rs:200`, `crates/rhof-web/src/lib.rs:350`). HTMX partial routes and smoke tests exist (`crates/rhof-web/src/lib.rs:247`, `crates/rhof-web/src/lib.rs:270`, `crates/rhof-web/src/lib.rs:543`). | Add provenance detail panels and visual polish; current UI is functional but minimal. |
| `PROMPT_09.txt` | CLI commands + CI + guardrails + seed semantics | `crates/rhof-cli/src/main.rs`, `.github/workflows/ci.yml`, `docs/ADAPTER_CHECKLIST.md`, `scripts/check_adapters.py`, `crates/rhof-sync/src/lib.rs`, `crates/rhof-web/templates/sources.html` | Partial | `sync/serve/report/seed/debug` work (`crates/rhof-cli/src/main.rs:41`, `crates/rhof-cli/src/main.rs:62`, `crates/rhof-cli/src/main.rs:70`, `crates/rhof-cli/src/main.rs:77`), `migrate` is stub (`crates/rhof-cli/src/main.rs:74`). CI and guardrails exist (`.github/workflows/ci.yml:61`, `docs/ADAPTER_CHECKLIST.md:1`, `crates/rhof-sync/src/lib.rs:781`, `crates/rhof-web/templates/sources.html:17`). | Implement `migrate` and verify `just dev`/`just migrate` as a documented green path. |
| `PROMPT_10.txt` | Adapter generator/templates, CI rule, chunked adapter expansion, runbook/docs | `crates/rhof-adapters/src/lib.rs`, `templates/adapter/*`, `scripts/check_adapters.py`, `docs/RUNBOOK.md`, `docs/SOURCES.md` | Partial | Generator and templates work (`crates/rhof-adapters/src/lib.rs:328`, `templates/adapter/bundle.json.tmpl:1`); CI check script validates baseline fixture/test presence (`scripts/check_adapters.py:56`). Contract checks do not enforce evidence coverage >=90 or parser correctness beyond presence (`scripts/check_adapters.py:72`). | Strengthen adapter CI checks (coverage threshold, snapshot parse pass per source, registration completeness). |

## 4. Completeness Score (0–100) + Rubric Breakdown

### Overall Score: **68 / 100**

### A) Core Functionality (0–25): **17 / 25**
Evidence:
- Happy-path fixture sync runs, writes artifacts, reports, and Parquet manifests (`crates/rhof-cli/src/main.rs:42`, `crates/rhof-sync/src/lib.rs:485`, `crates/rhof-sync/src/lib.rs:639`).
- Web UI and report flows are runnable (`crates/rhof-cli/src/main.rs:77`, `crates/rhof-web/src/lib.rs:204`).
- Main architectural promise is not fully delivered because sync is explicitly `staged-report-only` and not persisting canonical/versioned records (`crates/rhof-sync/src/lib.rs:602`).

### B) Developer Experience (0–20): **13 / 20**
Evidence:
- Clear workspace layout and `justfile` commands (`README.md:5`, `justfile:3`).
- Quickstart reflects actual current flow with manual `psql` migration (`README.md:19`).
- `just migrate` calls a stubbed CLI command (`justfile:17`, `crates/rhof-cli/src/main.rs:74`).
- `just tailwind` depends on missing `./bin/tailwindcss` (`justfile:38`).

### C) Tests + Quality Gates (0–15): **12 / 15**
Evidence:
- 16 Rust tests detected across storage/adapters/sync/web.
- Storage tests cover hashing, atomic writes, backoff (`crates/rhof-storage/src/lib.rs:402`).
- Adapter snapshot tests exist for all 5 initial sources (`crates/rhof-adapters/src/lib.rs:561`).
- Dedup threshold tests exist (`crates/rhof-sync/src/lib.rs:1053`).
- CI runs fmt/clippy/tests and adapter contract check (`.github/workflows/ci.yml:16`, `.github/workflows/ci.yml:27`, `.github/workflows/ci.yml:63`).
- Missing integration tests for DB migrations/persistence and live CLI workflow.

### D) Docs + Examples (0–15): **9 / 15**
Evidence:
- README Quickstart and runbook workflows are usable (`README.md:13`, `docs/RUNBOOK.md:3`).
- Source and adapter checklist docs exist (`docs/SOURCES.md:1`, `docs/ADAPTER_CHECKLIST.md:1`).
- `docs/ARCHITECTURE.md` and `docs/DATA_MODEL.md` remain placeholders (`docs/ARCHITECTURE.md:1`, `docs/DATA_MODEL.md:1`).

### E) Operability + Safety (0–15): **11 / 15**
Evidence:
- Config via env with defaults (`crates/rhof-sync/src/lib.rs:60`, `.env.example:1`).
- Artifact hashing + atomic writes reduce duplication and corruption risk (`crates/rhof-storage/src/lib.rs:65`).
- Runtime warning for missing evidence exists (`crates/rhof-sync/src/lib.rs:781`).
- Seed path is idempotent only in fixture/artifact sense and currently reuses sync path (`crates/rhof-sync/src/lib.rs:690`).
- Scheduler is scaffolded but inactive (`crates/rhof-sync/src/lib.rs:503`).

### F) Packaging + Release Readiness (0–10): **6 / 10**
Evidence:
- Workspace version and license metadata are present (`Cargo.toml:12`, `Cargo.toml:15`).
- No `LICENSE` file or changelog found (repo root scan).
- Distribution story is Cargo source + `cargo run`; no release checklist or binaries packaging docs yet.

### Biggest reason the score is not higher
The canonical persistence/versioning path is not implemented in the runtime sync flow, which diverges from the repo’s stated core promise (`.prompts/PROMPT_00_s.txt:92`, `crates/rhof-sync/src/lib.rs:602`).

### Highest-leverage improvement to raise the score fastest
Implement real DB persistence + version tracking in `rhof-sync` and wire `rhof-cli migrate` so the documented happy path becomes canonical and DB-backed (`crates/rhof-cli/src/main.rs:74`, `crates/rhof-sync/src/lib.rs:482`, `migrations/20260223210000_init_schema.up.sql:38`).

## 5. General Excellence Rating (1–10) + Evidence

### Rating: **7 / 10** (solid, credible project; still uneven in core integration)

Evidence:
- The workspace is cleanly modularized with coherent crate boundaries (`Cargo.toml:2`, `crates/rhof-core/Cargo.toml:1`, `crates/rhof-sync/Cargo.toml:1`).
- Prompt-driven design intent is reflected in real code and docs structure, not just stubs (`docs/RUNBOOK.md:26`, `scripts/check_adapters.py:83`).
- Provenance-first field model is implemented consistently in `rhof-core` and adapter fixture conversion (`crates/rhof-core/src/lib.rs:20`, `crates/rhof-adapters/src/lib.rs:168`).
- Artifact storage implementation is strong and production-minded for its scope (hashing, atomic rename, dedupe, retry, semaphores, tracing) (`crates/rhof-storage/src/lib.rs:44`, `crates/rhof-storage/src/lib.rs:117`, `crates/rhof-storage/src/lib.rs:344`).
- The fixture-first adapter strategy is well suited for deterministic regression testing and makes early progress real (`crates/rhof-adapters/src/lib.rs:82`, `crates/rhof-adapters/src/lib.rs:475`).
- CI quality gates are meaningful and not superficial (`.github/workflows/ci.yml:8`, `.github/workflows/ci.yml:19`, `.github/workflows/ci.yml:30`).
- The web UI route surface and HTMX partials are complete enough to demonstrate the interaction model (`crates/rhof-web/src/lib.rs:188`, `crates/rhof-web/src/lib.rs:247`, `crates/rhof-web/src/lib.rs:270`).
- The adapter expansion workflow (generator + templates + contract check + docs) is a strong scalability investment (`crates/rhof-adapters/src/lib.rs:328`, `templates/adapter/adapter.rs.tmpl:1`, `scripts/check_adapters.py:56`).
- Core architectural promise is undercut by staged-report-only persistence and a stubbed migration command (`crates/rhof-sync/src/lib.rs:602`, `crates/rhof-cli/src/main.rs:74`).
- Documentation quality is uneven: README and runbook are practical, but architecture/data model docs are still placeholders (`README.md:13`, `docs/RUNBOOK.md:3`, `docs/ARCHITECTURE.md:3`, `docs/DATA_MODEL.md:3`).

## 6. Priority Issues (P0–P3) (Prompt ID, Problem, Impact, Suggested Fix)

| Issue ID | Priority | Prompt ID | Problem | Evidence | Impact | Suggested Fix |
|---|---|---|---|---|---|---|
| PFN-001 | P1 | `PROMPT_02`, `PROMPT_09` | `rhof-cli migrate` is still scaffolded, but `just migrate` and runbook workflows point to it. | `crates/rhof-cli/src/main.rs:74`, `justfile:17`, `docs/RUNBOOK.md:8` | Core setup path is confusing/broken for new contributors; Quickstart requires a manual `psql` workaround. | Implement `migrate` (sqlx CLI subprocess or embedded migrator) and update README/runbook to make it canonical. |
| PFN-002 | P1 | `PROMPT_05`, `PROMPT_06` | Sync pipeline does not persist to Postgres or version rows; it writes staged reports/parquet only. | `crates/rhof-sync/src/lib.rs:482`, `crates/rhof-sync/src/lib.rs:596`, `crates/rhof-sync/src/lib.rs:602` | Biggest gap versus product promise (canonical DB, version tracking, review queue). Limits UI and operability. | Add DB repository layer and implement upsert/version persistence into `sources`, `opportunities`, `opportunity_versions`, tags/risk/review tables. |
| PFN-003 | P1 | `PROMPT_08` | Web UI reads latest `reports/*/opportunities_delta.json` and `sources.yaml`, not canonical DB/provenance views. | `crates/rhof-web/src/lib.rs:401`, `crates/rhof-web/src/lib.rs:412`, `crates/rhof-web/src/lib.rs:454` | UI cannot reflect true persistence state, review resolution durability, or field-level evidence details from DB. | After DB persistence lands, add DB-backed read models and keep report JSON fallback only for demo mode. |
| PFN-004 | P1 | `PROMPT_01`, `PROMPT_09` | `just tailwind` depends on a non-existent `./bin/tailwindcss` binary and there is no install/bootstrap path. | `justfile:38`, `assets/static/app.css:1`, `.prompts/PROMPT_01.txt:74` | Frontend styling workflow is not reproducible; users can’t regenerate CSS without guessing setup. | Add `scripts/install-tailwind.sh` (or `just tailwind-install`) and document checksumed standalone binary download. |
| PFN-005 | P1 | `PROMPT_00_s`, `PROMPT_02`, `PROMPT_09` | `sqlx prepare workflow documented` is a non-negotiable principle, but sqlx is not wired in code and `just sqlx-prepare` is likely non-functional. | `.prompts/PROMPT_00_s.txt:109`, `justfile:20`, `crates/rhof-sync/Cargo.toml:7` | Misleading command surface and hidden debt for compile-time SQL safety. | Either wire `sqlx` + migrations into runtime now or remove/defer `sqlx-prepare` target until real queries exist. |
| PFN-006 | P2 | `PROMPT_05` | Scheduler is scaffolded but does not run sync jobs even when enabled. | `crates/rhof-sync/src/lib.rs:503`, `crates/rhof-sync/src/lib.rs:512` | `RHOF_SCHEDULER_ENABLED` gives a false sense of automation readiness. | Add a `daemon`/`scheduler` CLI mode that starts the scheduler and invokes sync on triggers. |
| PFN-007 | P2 | `PROMPT_04`, `PROMPT_10` | Adapter implementations are fixture replay adapters; they do not parse raw HTML/JSON into canonical fields yet. | `crates/rhof-adapters/src/lib.rs:188`, `crates/rhof-adapters/src/lib.rs:243`, `crates/rhof-adapters/src/lib.rs:250` | Limits confidence that adapter contracts/generalization hold against real page changes. | Implement one real parser path (e.g., TELUS or Appen) reading `raw/listing.html` and producing drafts, while keeping fixture snapshots. |
| PFN-008 | P2 | `PROMPT_10` | Adapter contract CI checks verify presence and some fields, but not evidence coverage threshold or parser correctness. | `scripts/check_adapters.py:56`, `scripts/check_adapters.py:72`, `scripts/check_adapters.py:94` | New adapters can pass CI while being low-quality placeholders. | Extend check script to enforce `evidence_coverage_percent >= 90` and run per-source snapshot tests in CI. |
| PFN-009 | P2 | `PROMPT_07`, `PROMPT_09` | Prompt/docs command naming is inconsistent (`rhof` in prompt examples vs binary `rhof-cli` in code/docs). | `.prompts/PROMPT_07.txt:10`, `crates/rhof-cli/Cargo.toml:8`, `README.md:22` | Minor confusion for contributors following prompt text directly. | Standardize on `rhof-cli` or rename binary to `rhof` and update prompt/docs references. |
| PFN-010 | P2 | `PROMPT_01`, `PROMPT_02` | `docs/ARCHITECTURE.md` and `docs/DATA_MODEL.md` are still placeholders. | `docs/ARCHITECTURE.md:3`, `docs/DATA_MODEL.md:3` | Weakens onboarding and design traceability despite strong prompt intent. | Fill with current as-built pipeline/data model and clearly mark planned DB persistence gaps. |
| PFN-011 | P3 | `PROMPT_10` | Sample generated scaffold (`sample-source`) is committed into docs/fixtures/tests and can be mistaken for a supported source. | `docs/SOURCES.md:18`, `fixtures/sample-source/sample/bundle.json:1` | Small credibility/maintenance clutter. | Move sample generator output to `examples/` or delete after generator validation. |
| PFN-012 | P3 | `PROMPT_08` | Frontend is functional but visually bare; Tailwind output is placeholder CSS. | `crates/rhof-web/templates/index.html:11`, `assets/static/app.css:1` | Reduces front-facing polish and demo impact. | Establish minimal design tokens + generated Tailwind asset pipeline and add one polished dashboard layout pass. |

## 7. Overengineering / Complexity Risks (Complexity vs Value)

### Complexity vs Value (Top 10 Hotspots)

| Hotspot | Risk | Value delivered | Simplification recommendation |
|---|---|---|---|
| Six-crate workspace before DB-backed vertical slice (`crates/*`) | Med | Clear boundaries and future scalability | Keep crates, but postpone further crate splitting; implement DB persistence in existing crates first. |
| Dual data planes (DB schema exists, UI reads report JSON) | High | Rapid demo without DB reads | Consolidate on DB-backed read models; keep JSON report export as an optional output, not primary UI source. |
| Scheduler abstraction without executable mode (`maybe_build_scheduler`) | Med | Shows intended architecture | Add one CLI daemon mode or remove scheduler config knobs until operational. |
| Dedup/enrichment hook indirection + noop implementations | Low | Clean seam between Prompt 05 and 06 | Keep hooks, but inline `Noop*` once real DB persistence is added if no alternate implementations emerge. |
| Arrow/Parquet export complexity inside sync crate | Med | Valuable snapshots and manifests | Keep feature, but isolate export module (`parquet_export.rs`) for clarity and compile-time scope. |
| Adapter generator + templates + contract checker before mature adapters | Low | Strong long-term payoff, good scaffolding discipline | Keep, but strengthen checks to prevent placeholder adapters passing as “done.” |
| Unified fixture/manual/seed bundle schema | Low | Excellent reuse and determinism | Keep; document one canonical schema file/example and validation script to reduce ambiguity. |
| `sqlx-prepare` command surface before actual sqlx queries | Med | Signals intended future workflow | Remove/defer `sqlx-prepare` recipe until sqlx query macros are present, or implement a minimal sqlx-backed module immediately. |
| Tailwind standalone binary workflow without bootstrap tooling | Med | Node-free frontend build | Add a tiny installer script; avoid adding more frontend tooling until this path is reliable. |
| Generated sample-source artifacts committed in main tree | Low | Demonstrates generator output | Move to `examples/` or make generation validation ephemeral in CI/test rather than checked-in scaffolding. |

## 8. Naming / Structure / Consistency Findings

### Factual findings
- `sources.yaml` canonical top-level `sources:` shape is implemented and validated by `scripts/check_adapters.py` (`sources.yaml:1`, `scripts/check_adapters.py:21`).
- Port policy is consistently updated to `5401` in Docker/CI/env/docs (`docker-compose.yml:7`, `.env.example:1`, `.github/workflows/ci.yml:47`, `README.md:15`).
- Binary name is `rhof-cli` while some prompt examples still say `rhof` (`crates/rhof-cli/Cargo.toml:8`, `.prompts/PROMPT_07.txt:10`).
- `docs/SOURCES.md` mixes implemented source registry documentation with generated sample scaffold docs (`docs/SOURCES.md:5`, `docs/SOURCES.md:18`).
- Tailwind config content glob references `./templates/**/*.html`, but templates actually live under `crates/rhof-web/templates` (`assets/tailwind/tailwind.config.js:2`, `crates/rhof-web/templates/index.html:1`).

### Recommendations
- Standardize CLI naming to one binary label in prompts, docs, and examples.
- Separate real source registry status from generator-example output (e.g., `docs/SOURCES.md` vs `docs/GENERATOR_EXAMPLES.md`).
- Fix Tailwind content globs to include `crates/rhof-web/templates/**/*.html` so generated CSS is accurate once the binary is installed.
- Replace placeholder architecture/data model docs with as-built documentation plus clearly marked roadmap gaps.

## 9. Highest-Leverage Next Steps (Top 10) + Estimated Effort (S/M/L)

| # | Next step | Why it matters | Evidence anchor | Effort |
|---|---|---|---|---|
| 1 | Implement DB persistence + version tracking in sync pipeline | Closes biggest gap against core promise and unlocks DB-backed UI/review flows | `crates/rhof-sync/src/lib.rs:602`, `migrations/20260223210000_init_schema.up.sql:38` | L |
| 2 | Implement `rhof-cli migrate` and make `just migrate` real | Fixes setup friction and removes quickstart workaround | `crates/rhof-cli/src/main.rs:74`, `justfile:17` | M |
| 3 | Switch web read path to DB-backed models (keep JSON fallback as demo mode) | Aligns UI with canonical data and review persistence | `crates/rhof-web/src/lib.rs:401`, `crates/rhof-web/src/lib.rs:454` | L |
| 4 | Add Tailwind binary bootstrap/install script + docs | Makes frontend styling reproducible and unlocks UI polish | `justfile:38`, `assets/static/app.css:1` | S |
| 5 | Fill `docs/ARCHITECTURE.md` and `docs/DATA_MODEL.md` with as-built diagrams/tables | Improves onboarding and auditability | `docs/ARCHITECTURE.md:3`, `docs/DATA_MODEL.md:3` | M |
| 6 | Add one real raw-to-parse adapter implementation (not fixture replay) | Validates adapter abstraction against actual selectors/HTML parsing | `crates/rhof-adapters/src/lib.rs:243`, `crates/rhof-adapters/src/lib.rs:250` | M |
| 7 | Strengthen adapter CI checks (coverage threshold + snapshot execution) | Prevents placeholder adapters from passing contract checks | `scripts/check_adapters.py:56`, `scripts/check_adapters.py:94` | M |
| 8 | Add scheduler daemon mode and integration test | Makes automation settings meaningful | `crates/rhof-sync/src/lib.rs:503`, `.env.example:4` | M |
| 9 | Clean up sample generator artifacts or move to examples | Reduces ambiguity in supported sources | `docs/SOURCES.md:18`, `fixtures/sample-source/sample/bundle.json:1` | S |
| 10 | Add packaging/release basics (LICENSE file, release checklist, changelog stub) | Improves credibility and distribution readiness | `Cargo.toml:15`, root scan (no `LICENSE*`/`CHANGELOG*`) | S |
