DROP INDEX IF EXISTS idx_opportunity_versions_created_at;
DROP INDEX IF EXISTS idx_raw_artifacts_content_hash;
DROP INDEX IF EXISTS idx_opportunities_apply_url;
DROP INDEX IF EXISTS idx_opportunities_canonical_key;

DROP TABLE IF EXISTS review_items;
DROP TABLE IF EXISTS dedup_cluster_members;
DROP TABLE IF EXISTS dedup_clusters;
DROP TABLE IF EXISTS opportunity_risk_flags;
DROP TABLE IF EXISTS risk_flags;
DROP TABLE IF EXISTS opportunity_tags;
DROP TABLE IF EXISTS tags;

ALTER TABLE opportunities DROP CONSTRAINT IF EXISTS opportunities_current_version_fk;

DROP TABLE IF EXISTS opportunity_versions;
DROP TABLE IF EXISTS opportunities;
DROP TABLE IF EXISTS raw_artifacts;
DROP TABLE IF EXISTS fetch_runs;
DROP TABLE IF EXISTS sources;
