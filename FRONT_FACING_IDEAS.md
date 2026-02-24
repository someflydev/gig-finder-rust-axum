# Front-Facing Ideas

## 1. Audience Positioning Options (2–3)

### Option A: Builders / OSS Operators
Target audience:
- Rust developers, data pipeline builders, scraping/ETL practitioners who want a provenance-first architecture and real fixture discipline.

Positioning angle:
- “A credible Rust/Axum reference for source adapters, immutable artifacts, and traceable extraction workflows.”

Why this fits the repo (evidence):
- Strong fixture-first adapter workflow (`crates/rhof-adapters/src/lib.rs:82`, `docs/ADAPTER_CHECKLIST.md:7`).
- Artifact immutability and hash-addressing are implemented (`crates/rhof-storage/src/lib.rs:65`).
- CI quality gates are real (`.github/workflows/ci.yml:8`, `.github/workflows/ci.yml:61`).

### Option B: Data Operations / Research Ops Teams
Target audience:
- Small teams that need repeatable lead/opportunity ingestion with auditability, not a black-box scraper.

Positioning angle:
- “Operationally safe opportunity intelligence with evidence-bearing fields, review queues, and repeatable snapshots.”

Why this fits the repo (evidence):
- Provenance-bearing field model (`crates/rhof-core/src/lib.rs:20`, `crates/rhof-core/src/lib.rs:66`).
- Review queue concept and UI route exist (`migrations/20260223210000_init_schema.up.sql:114`, `crates/rhof-web/src/lib.rs:196`).
- Parquet snapshot manifest with file hashes exists (`crates/rhof-sync/src/lib.rs:661`, `crates/rhof-sync/src/lib.rs:994`).

### Option C: Learning / Prompt-Driven System Builders
Target audience:
- People experimenting with prompt-sequenced repo generation and iterative agent workflows.

Positioning angle:
- “A prompt-built system with traceable evolution from spec to implementation, including adapter generation and quality checks.”

Why this fits the repo (evidence):
- Full prompt sequence is present in `.prompts/PROMPT_00_s.txt` through `.prompts/PROMPT_10.txt`.
- Preflight improvement log exists (`.prompts/improvements-before-initial-run.txt`).
- Generator/template + contract-check loop is visible and inspectable (`templates/adapter/*`, `scripts/check_adapters.py:83`).

## 2. README Final-Copy Directions (2–3 variants)

### A. Hacker/Builder Version (quickstart-first, gritty credibility)
Target audience:
- Developers who want to run code in 5 minutes and inspect artifacts.

One-liner value prop:
- “Run a provenance-first, fixture-driven RHOF pipeline locally and inspect artifacts, reports, and a live HTMX UI.”

Why this is different (5–7 bullets):
- Immutable raw artifact storage with hash-addressed paths (`crates/rhof-storage/src/lib.rs:50`).
- Evidence-bearing canonical fields via `Field<T>` + `EvidenceRef` (`crates/rhof-core/src/lib.rs:11`, `crates/rhof-core/src/lib.rs:22`).
- Deterministic fixture bundles for adapters/manual/seed reuse (`crates/rhof-adapters/src/lib.rs:82`, `manual/prolific/sample.json:1`).
- Dedup + rule enrichment with tests and YAML configs (`crates/rhof-sync/src/lib.rs:208`, `rules/tags.yaml:1`).
- Parquet snapshots + manifest with hashes per run (`crates/rhof-sync/src/lib.rs:639`, `crates/rhof-sync/src/lib.rs:671`).
- HTMX/Askama UI for fast inspection of recent runs (`crates/rhof-web/src/lib.rs:188`).
- Adapter scaffold generator + CI contract checks (`crates/rhof-adapters/src/lib.rs:328`, `scripts/check_adapters.py:56`).

Demo story (3–6 steps):
- `just db-up`
- Apply migration with `just migrate` (or `cargo run -p rhof-cli -- migrate`)
- `cargo run -p rhof-cli -- sync`
- `cargo run -p rhof-cli -- report daily --runs 3`
- `cargo run -p rhof-cli -- serve`
- Inspect `/reports`, `/sources`, and generated `reports/<run_id>/snapshots/manifest.json`

Proof points to include:
- `cargo test --workspace` output snippet.
- CI badges (`fmt`, `clippy`, `test`).
- Sample `daily_brief.md` and `manifest.json` excerpt.
- Screenshot/GIF of HTMX facets + reports page.

What NOT to promise yet:
- Fully DB-backed persistence/version UI.
- Live crawling robustness across changing production sites.
- Production scheduler/daemon behavior.

### B. Enterprise/Platform Version (safety, operability, guarantees)
Target audience:
- Teams evaluating whether RHOF can become a governed data ingestion component.

One-liner value prop:
- “A provenance-first ingestion system with immutable artifacts, reproducible fixtures, and auditable enrichment rules.”

Why this is different:
- Canonical field-level provenance modeled in code (`crates/rhof-core/src/lib.rs:20`).
- ToS-safe adapter guidelines and checklist baked into docs (`docs/ADAPTER_CHECKLIST.md:5`).
- Data-driven enrichment rules in YAML (`crates/rhof-sync/src/lib.rs:326`, `rules/risk.yaml:1`).
- Contract checks for source registry + fixture bundles (`scripts/check_adapters.py:17`, `scripts/check_adapters.py:56`).
- Snapshot exports with manifest hashes for offline analytics (`crates/rhof-sync/src/lib.rs:661`).
- Explicit port/config policy and Dockerized Postgres (`.env.example:1`, `docker-compose.yml:7`).

Demo story:
- Show `sources.yaml` as authoritative registry.
- Show one fixture bundle + raw artifact + snapshot test.
- Run sync to generate artifacts/reports/parquet.
- Open reports UI and sources UI with manual/gated badges.
- Show CI contract script preventing incomplete adapter additions.

Proof points to include:
- Schema table list from migration SQL.
- Adapter checklist excerpt.
- `manifest.json` hash example.
- CI workflow summary.

What NOT to promise yet:
- Complete DB persistence and governance workflows in runtime path.
- Multi-tenant auth/access control.
- SLA-backed source monitoring.

### C. Educator/Community Version (learning journey, examples)
Target audience:
- Learners studying Rust systems design, HTMX/Axum patterns, and prompt-driven repo generation.

One-liner value prop:
- “Learn how a prompt-built Rust system grows from workspace scaffold to adapters, sync, reports, UI, and CI.”

Why this is different:
- Ordered prompt plan is preserved and inspectable (`.prompts/PROMPT_00_s.txt`, `.prompts/PROMPT_10.txt`).
- Clear modular crates with focused responsibilities (`Cargo.toml:2`).
- Fixture-first adapters make tests deterministic and easy to inspect (`crates/rhof-adapters/src/lib.rs:475`).
- HTMX + Askama examples are compact and readable (`crates/rhof-web/templates/opportunities.html:12`).
- CI checks include both Rust and Python contract verification (`.github/workflows/ci.yml:57`).
- Generator templates demonstrate scalable contribution paths (`templates/adapter/adapter.rs.tmpl:1`).

Demo story:
- Read prompt flow (`.prompts/`).
- Run tests.
- Run sync.
- Explore generated artifacts/reports.
- Add a sample adapter scaffold via `new-adapter`.
- Inspect docs/checklist updates.

Proof points to include:
- Prompt-to-artifact map diagram.
- “Before/after” generated adapter scaffold files.
- Test count and CI jobs.

What NOT to promise yet:
- Production-ready scraping fleet.
- Stable public API contract.
- Fully polished UX.

## 3. Productized Demo Flows (how someone experiences value fast)

### Demo Flow 1: “Trust the Data” (5 minutes)
- Start DB and run one fixture-driven sync (`README.md:15`, `README.md:22`).
- Open `reports/<run_id>/daily_brief.md` and `opportunities_delta.json` (`docs/RUNBOOK.md:15`).
- Inspect `artifacts/<timestamp>/<source_id>/<hash>.ext` to show immutable capture (`crates/rhof-storage/src/lib.rs:50`).
- Open web UI and compare Sources/Review/Reports pages (`crates/rhof-web/src/lib.rs:190`).
- Show `Field<T> + EvidenceRef` model in `crates/rhof-core/src/lib.rs:20` to explain provenance-first design.

### Demo Flow 2: “Add a Source Safely” (10 minutes)
- Add a source entry to `sources.yaml` (`docs/RUNBOOK.md:28`).
- Run `cargo run -p rhof-cli -- new-adapter <source_id>` (`crates/rhof-cli/src/main.rs:55`).
- Show generated templates + fixtures + docs stub (`crates/rhof-adapters/src/lib.rs:346`, `crates/rhof-adapters/src/lib.rs:393`).
- Run `python3 scripts/check_adapters.py` to fail/pass contract basics (`scripts/check_adapters.py:83`).
- Fill fixture bundle and snapshot test, then rerun tests.

### Demo Flow 3: “Analyze Outputs Offline” (8 minutes)
- Run sync and locate Parquet files + `manifest.json` (`crates/rhof-sync/src/lib.rs:651`, `crates/rhof-sync/src/lib.rs:671`).
- Show `rhof report daily --runs 3` output for recent run summaries (`crates/rhof-sync/src/lib.rs:713`).
- Optionally open Parquet in DuckDB/Polars to highlight portability (DuckDB is explicitly optional in prompt; Parquet is the guaranteed output in `.prompts/PROMPT_07.txt:14`).

## 4. Frontend Vision (MVP + v2 + anti-scope)

### MVP (1–2 weeks)
Goal:
- Turn the current functional web UI into a compelling “artifact-to-report-to-review” demo surface without changing core backend architecture.

Scope:
- Polished dashboard shell and navigation using existing routes (`crates/rhof-web/src/lib.rs:190`).
- Visual cards for run health, source counts, review queue counts using current `load_dashboard_data` outputs (`crates/rhof-web/src/lib.rs:401`).
- Reports page chart rendered client-side from `/reports/chart` JSON (`crates/rhof-web/src/lib.rs:350`).
- Opportunity detail provenance panel populated from current delta JSON fields (and future-proof placeholders for DB evidence).
- Source badges and manual/gated indicators styled clearly (currently plain text in `crates/rhof-web/templates/sources.html:17`).
- Screenshot-ready CSS pipeline (fix Tailwind binary install and generate real `assets/static/app.css`).

### v2 (4–8 weeks)
Goal:
- Productize RHOF as an inspectable operations console with lineage and schema exploration.

Scope:
- Interactive fixture bundle explorer (`fixtures/<source_id>/<fixture_id>/bundle.json`) with raw artifact preview and parsed record diffing.
- Run lineage viewer that links `.prompts` intent -> generated artifacts -> reports/manifests.
- Parquet manifest/browser with schema introspection and hash verification UI.
- Adapter coverage dashboard (fixtures present, snapshot tests, evidence coverage, source status).
- Read-model-backed UI expansion (real review queue resolution, version history, deeper provenance drill-down) building on the new DB-backed loaders.

### Don’t Build Yet (anti-scope)
- Full auth/multi-tenant RBAC.
- Live scraping control center with distributed workers.
- Realtime websockets/event streaming.
- Heavy client-side state framework or design system package before the Tailwind pipeline is reliable.
- Embedded SQL editors or notebook environments.

## 5. 5 Frontend Languages Considered (why + 2+ frameworks each)

### 1) TypeScript
Why it’s a strong fit:
- Best ecosystem for polished docs/product packaging, charts, and deployment ergonomics.
- Easy to consume existing JSON outputs (`reports/*/opportunities_delta.json`, `/reports/chart`) and call backend routes.

Elegant/excellent frameworks:
- Next.js (App Router) for hybrid SSR/static docs and demos.
- SvelteKit for lean, elegant file-based routing and minimal boilerplate.
- Astro for content-heavy docs + islands (excellent for showcasing artifacts and runbooks).

Integration with this repo:
- Strong choice for a separate `frontend/` app that reads generated report JSON or proxies to `rhof-web`/CLI-backed endpoints.
- Can be static-first (Astro) or SSR (Next/SvelteKit) depending whether DB-backed APIs land soon.

### 2) Elm
Why it’s a strong fit:
- Excellent for highly trustworthy, deterministic UI state around filtering/facets/review workflows.
- Strong fit for “artifact explorer” and “run lineage viewer” interfaces where correctness and clarity matter.

Elegant/excellent frameworks/tooling:
- Elm + `elm-pages` (content-heavy static/SSR-ish sites with strong typing).
- Elm + Vite (`vite-plugin-elm`) for SPA/dashboard development.
- Elm UI (library) for expressive, maintainable UI composition without CSS sprawl.

Integration with this repo:
- Best as a frontend module consuming JSON from `rhof-web` routes or static reports/manifests.
- Practical if the goal is a high-trust operations console and teaching value, less ideal for fast broad ecosystem integrations.

### 3) Dart (Flutter Web)
Why it’s a strong fit:
- Great for a polished cross-platform UI if RHOF eventually wants desktop/web parity.
- Strong component model and charting options for dashboards and inspectors.

Elegant/excellent frameworks/tooling:
- Flutter Web (Material 3 or custom design system).
- Jaspr (Dart web framework with Flutter-like declarative style; useful if staying web-first).
- `go_router` + Riverpod (for structured app navigation/state in Flutter).

Integration with this repo:
- Best as a separate SPA consuming JSON/REST endpoints exposed by a thin Rust server wrapper.
- Heavier build/runtime footprint than needed for the current MVP, but viable for a future product console.

### 4) Kotlin (Compose Multiplatform Web / Kotlin/JS)
Why it’s a strong fit:
- Great typed UI and shared domain modeling potential if the project ever adds JVM-side tooling or analytics.
- Strong developer ergonomics for teams already invested in Kotlin.

Elegant/excellent frameworks:
- Compose Multiplatform Web (Wasm/JS UI with modern declarative patterns).
- KVision (Kotlin full-stack/web framework with dashboards/forms).
- fritz2 (reactive Kotlin/JS, functional flavor).

Integration with this repo:
- Separate frontend app consuming JSON outputs or backend endpoints.
- Strong long-term type safety, but smaller ecosystem and more setup friction than TS for a front-facing docs/demo site.

### 5) Rust (Yew or Leptos)
Why it’s a strong fit:
- Keeps language alignment with the existing repo; shared types are possible later.
- Strong story for a Rust-first audience and highly cohesive branding.

Elegant/excellent frameworks:
- Leptos (SSR + islands + signals, very strong for modern Rust web apps).
- Yew (mature Rust SPA framework).
- Dioxus (desktop/web/fullstack options, good ergonomics).

Integration with this repo:
- Could coexist with Axum backend cleanly, especially via Leptos SSR or a separate WASM frontend.
- More friction for fast docs/marketing polish than Astro/Next unless the team explicitly wants Rust-only frontend stack.

## 6. Recommended Frontend Stack (one clear pick) + Integration Plan

### Recommended stack: **TypeScript + Astro (content shell) + selective HTMX/Axum reuse**

Why this is the best current fit:
- The repo already has strong content artifacts (reports, manifests, fixtures, `.prompts`, docs) that map naturally to a content-first site.
- Astro is excellent for documentation/showcase pages and can progressively add interactive islands for charts and artifact explorers.
- It avoids overcommitting to a heavy SPA while the backend continues evolving from mixed report JSON/DB views toward richer DB-backed read models.

Integration plan (grounded in current assets)
- Deployment shape (MVP): Static Astro site built from checked-in artifacts/docs + screenshots/GIFs; no auth.
- Backend integration (MVP): Parse and present generated artifacts as static JSON examples (e.g., `reports/*/snapshots/manifest.json`, fixture bundles under `fixtures/`), with links to run the local CLI.
- Backend integration (v2): Add lightweight proxy endpoints (or reuse `rhof-web` JSON routes) for live local demo mode (`/reports/chart`, future DB-backed read APIs).
- CLI integration: Provide a small server wrapper or build step that runs `rhof-cli debug`/`report daily` and writes JSON snapshots consumed by Astro pages (feasible because CLI already produces deterministic text/json outputs in `crates/rhof-cli/src/main.rs:49`, `crates/rhof-cli/src/main.rs:70`).
- Static vs SSR vs SPA: Start static-first with islands; introduce SSR only if live local demo mode becomes a primary experience.
- Auth: None for MVP. Reassess only after DB-backed UI + multi-user workflows exist.
- Hosting: Netlify or Vercel for the frontend shell; keep the Rust app runnable locally and optionally deployable separately (Fly.io or self-hosted container) later.

## 7. Assets/Artifacts to Showcase (what the repo already has)

- Prompt lineage: `.prompts/PROMPT_00_s.txt` through `.prompts/PROMPT_10.txt`.
- Preflight process log: `.prompts/improvements-before-initial-run.txt`.
- Workspace architecture and crate boundaries: `Cargo.toml`, `crates/*/Cargo.toml`.
- Canonical provenance model: `crates/rhof-core/src/lib.rs` (`Field<T>`, `EvidenceRef`).
- Immutable artifact pathing + hash logic: `crates/rhof-storage/src/lib.rs:50`.
- Real fixture bundle examples: `fixtures/appen-crowdgen/sample/bundle.json`, `manual/prolific/sample.json`.
- Adapter snapshot tests and checklist: `crates/rhof-adapters/src/lib.rs:561`, `docs/ADAPTER_CHECKLIST.md`.
- Sync outputs: `reports/<run_id>/daily_brief.md`, `reports/<run_id>/opportunities_delta.json`, `reports/<run_id>/snapshots/manifest.json` (format written in `crates/rhof-sync/src/lib.rs:583` and `crates/rhof-sync/src/lib.rs:639`).
- Web routes and HTMX partials: `crates/rhof-web/src/lib.rs`, `crates/rhof-web/templates/*`.
- CI and adapter contract enforcement: `.github/workflows/ci.yml`, `scripts/check_adapters.py`.

## 8. Packaging Polish Checklist (screenshots, gifs, examples, site deploy)

- Create 3 screenshots: dashboard, opportunities facet/table flow, reports page chart JSON -> rendered chart.
- Record one short GIF: `sync` -> `report` -> `serve` -> web walkthrough.
- Add a checked-in sample `reports/demo-run/` folder (or a scripted demo generator) for stable docs screenshots.
- Publish one “artifact anatomy” example page (raw artifact path, fixture bundle, snapshot test, manifest entry).
- Add README badges (CI, Rust version, license once `LICENSE` file exists).
- Add a “Current status” box that explicitly states fixture-driven demo mode and DB persistence roadmap.
- Add a one-command local demo script (`db-up` + `migrate` + `sync` + `serve`) to reduce onboarding friction.
- Add a docs site deploy workflow (Astro static build) only after content stabilizes.
