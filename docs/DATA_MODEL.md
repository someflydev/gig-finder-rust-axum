# Data Model

## Core Provenance Types (`rhof-core`)

### `Field<T>`

Canonical field wrapper used on extracted values:

- `value: Option<T>`
- `evidence: Option<EvidenceRef>`

This makes each populated canonical value traceable back to a source artifact.

### `EvidenceRef`

Provenance pointer for extracted values:

- `raw_artifact_id: UUID`
- `source_url`
- `selector_or_pointer`
- `snippet`
- `fetched_at`
- `extractor_version`

### `OpportunityDraft`

Adapter -> sync handoff type containing source metadata and field-wrapped canonical values before persistence/versioning.

## Postgres Tables (Current Usage)

### Actively used in runtime sync path

- `sources`
- `fetch_runs`
- `raw_artifacts`
- `opportunities`
- `opportunity_versions`
- `tags`
- `opportunity_tags`
- `risk_flags`
- `opportunity_risk_flags`
- `review_items` (created for review-required dedup outcomes)

### Created by migration but not yet fully used

- `dedup_clusters`
- `dedup_cluster_members`

## Versioning Behavior (Current)

- `opportunities` are keyed by normalized `canonical_key`.
- Sync upserts the canonical row and updates `last_seen_at`.
- `opportunity_versions` stores a JSON snapshot of the staged opportunity payload (`data_json`) plus evidence payload (`evidence_json`).
- A new version row is inserted only when `data_json` changes from the latest persisted version (idempotent repeated syncs do not duplicate versions).
- `current_version_id` on `opportunities` points to the latest persisted version.

## Review Queue Semantics (Current)

- Dedup logic flags borderline matches with `review_required = true`.
- Sync creates an open `review_items` row (`item_type='dedup_review'`) if one is not already open for the same opportunity.
- Web `/review` displays review-required opportunities.
- Web `POST /review/:id/resolve` currently returns a UI partial only and does not update `review_items` in Postgres yet.

## Artifact / Fixture Relationship

- `raw_artifacts` rows reference immutable on-disk storage paths in `ARTIFACTS_DIR`.
- Fixture bundles embed deterministic metadata and provenance-compatible parsed records.
- For fixture-driven sync, raw artifact IDs are deterministic (derived from source + fixture path) to keep repeated runs stable.

## Gaps / Future Tightening

- Replace JSON `data_json` snapshot comparison with more explicit canonical schema fields and structured diffs (`diff_json`).
- Persist dedup cluster proposals and membership rows.
- Add documented schemas for report JSON and Parquet files for downstream consumers.
