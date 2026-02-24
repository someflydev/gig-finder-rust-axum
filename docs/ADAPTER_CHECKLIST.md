# Adapter Checklist

Use this checklist when adding or updating a source adapter.

- ToS-safe and public-page access only (no login bypass)
- Crawlability declared correctly (`PublicHtml`, `Api`, `Rss`, `Gated`, `ManualOnly`)
- At least one checked-in fixture bundle (`fixtures/<source_id>/<fixture_id>/bundle.json`)
- Raw artifact fixture files checked in under `fixtures/<source_id>/<fixture_id>/raw/`
- Golden snapshot parsing test exists and passes
- `extractor_version` is set and tracked in fixture bundles
- Populated canonical fields include provenance-compatible evidence
- Manual/gated fallback documented (if source is gated/manual)
- Source entry added/updated in `sources.yaml`
- Local sync + report run validated after adapter changes
