//! Sync pipeline orchestration (PROMPT_05 staged implementation).

use std::collections::{BTreeMap, HashMap};
use std::fs::File;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use arrow_array::{BooleanArray, Float64Array, RecordBatch, StringArray, UInt32Array};
use arrow_schema::{DataType, Field as ArrowField, Schema};
use chrono::{DateTime, Utc};
use parquet::arrow::ArrowWriter;
use rhof_adapters::{
    adapter_for_source, deterministic_raw_artifact_id_for_bundle, load_fixture_bundle,
    load_manual_fixture_bundle, Crawlability, FixtureBundle,
};
use rhof_core::OpportunityDraft;
use rhof_storage::{ArtifactStore, HttpClientConfig, HttpFetcher};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::{migrate::Migrator, PgPool, Row};
use strsim::jaro_winkler;
use tokio::fs;
use tokio_cron_scheduler::{Job, JobScheduler};
use tracing::{info, warn};
use uuid::Uuid;
use sha2::{Digest, Sha256};

pub const CRATE_NAME: &str = "rhof-sync";
static MIGRATOR: Migrator = sqlx::migrate!("../../migrations");

#[derive(Debug, Clone, Deserialize)]
pub struct SourceRegistry {
    pub sources: Vec<SourceConfig>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SourceConfig {
    pub source_id: String,
    pub display_name: String,
    pub enabled: bool,
    pub crawlability: Crawlability,
    pub mode: String,
    #[serde(default)]
    pub listing_urls: Vec<String>,
    #[serde(default)]
    pub detail_url_patterns: Vec<String>,
    #[serde(default)]
    pub notes: Option<String>,
}

#[derive(Debug, Clone)]
pub struct SyncConfig {
    pub database_url: String,
    pub artifacts_dir: PathBuf,
    pub scheduler_enabled: bool,
    pub sync_cron_1: String,
    pub sync_cron_2: String,
    pub user_agent: String,
    pub http_timeout_secs: u64,
    pub workspace_root: PathBuf,
}

impl SyncConfig {
    pub fn from_env() -> Self {
        Self {
            database_url: std::env::var("DATABASE_URL")
                .unwrap_or_else(|_| "postgres://rhof:rhof@localhost:5401/rhof".to_string()),
            artifacts_dir: std::env::var("ARTIFACTS_DIR")
                .map(PathBuf::from)
                .unwrap_or_else(|_| PathBuf::from("./artifacts")),
            scheduler_enabled: std::env::var("RHOF_SCHEDULER_ENABLED")
                .map(|v| matches!(v.as_str(), "1" | "true" | "TRUE" | "True"))
                .unwrap_or(false),
            sync_cron_1: std::env::var("SYNC_CRON_1").unwrap_or_else(|_| "0 6 * * *".to_string()),
            sync_cron_2: std::env::var("SYNC_CRON_2").unwrap_or_else(|_| "0 18 * * *".to_string()),
            user_agent: std::env::var("RHOF_USER_AGENT")
                .unwrap_or_else(|_| "rhof-bot/0.1".to_string()),
            http_timeout_secs: std::env::var("RHOF_HTTP_TIMEOUT_SECS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(20),
            workspace_root: PathBuf::from("."),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct FetchRunRecord {
    pub run_id: Uuid,
    pub started_at: DateTime<Utc>,
    pub finished_at: DateTime<Utc>,
    pub status: String,
    pub database_url: String,
    pub persistence_mode: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StagedOpportunity {
    pub source_id: String,
    pub canonical_key: String,
    pub version_no: u32,
    pub dedup_confidence: Option<f64>,
    pub review_required: bool,
    pub tags: Vec<String>,
    pub risk_flags: Vec<String>,
    pub draft: OpportunityDraft,
}

#[derive(Debug, Clone, Serialize)]
pub struct SyncRunSummary {
    pub run_id: Uuid,
    pub started_at: DateTime<Utc>,
    pub finished_at: DateTime<Utc>,
    pub enabled_sources: usize,
    pub fetched_artifacts: usize,
    pub parsed_drafts: usize,
    pub persisted_versions: usize,
    pub reports_dir: String,
    pub parquet_manifest: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ParquetManifest {
    pub schema_version: u32,
    pub files: Vec<ParquetManifestFile>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ParquetManifestFile {
    pub name: String,
    pub path: String,
    pub sha256: String,
    pub bytes: u64,
}

pub trait DedupHook: Send + Sync {
    fn apply(&self, items: Vec<StagedOpportunity>) -> Result<Vec<StagedOpportunity>>;
}

pub trait EnrichmentHook: Send + Sync {
    fn apply(&self, items: Vec<StagedOpportunity>) -> Result<Vec<StagedOpportunity>>;
}

#[derive(Default)]
pub struct NoopDedupHook;

impl DedupHook for NoopDedupHook {
    fn apply(&self, items: Vec<StagedOpportunity>) -> Result<Vec<StagedOpportunity>> {
        Ok(items)
    }
}

#[derive(Default)]
pub struct NoopEnrichmentHook;

impl EnrichmentHook for NoopEnrichmentHook {
    fn apply(&self, items: Vec<StagedOpportunity>) -> Result<Vec<StagedOpportunity>> {
        Ok(items)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DedupReviewItem {
    pub canonical_key_a: String,
    pub canonical_key_b: String,
    pub confidence_score: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DedupClusterProposal {
    pub cluster_id: String,
    pub confidence_score: f64,
    pub members: Vec<String>,
    pub review_required: bool,
}

#[derive(Debug, Clone, Copy)]
pub struct DedupConfig {
    pub auto_cluster_threshold: f64,
    pub review_threshold: f64,
}

impl Default for DedupConfig {
    fn default() -> Self {
        Self {
            auto_cluster_threshold: 0.95,
            review_threshold: 0.85,
        }
    }
}

pub struct DedupEngine {
    config: DedupConfig,
}

impl DedupEngine {
    pub fn new(config: DedupConfig) -> Self {
        Self { config }
    }

    pub fn normalize_key_fragment(input: &str) -> String {
        input
            .to_ascii_lowercase()
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() { c } else { ' ' })
            .collect::<String>()
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
    }

    pub fn similarity(&self, a: &StagedOpportunity, b: &StagedOpportunity) -> f64 {
        let ka = Self::normalize_key_fragment(&a.canonical_key);
        let kb = Self::normalize_key_fragment(&b.canonical_key);
        let title_a = a.draft.title.value.as_deref().unwrap_or_default();
        let title_b = b.draft.title.value.as_deref().unwrap_or_default();
        let title_score = jaro_winkler(title_a, title_b);
        let key_score = jaro_winkler(&ka, &kb);
        (title_score * 0.7) + (key_score * 0.3)
    }

    pub fn apply(
        &self,
        mut items: Vec<StagedOpportunity>,
    ) -> (Vec<StagedOpportunity>, Vec<DedupClusterProposal>, Vec<DedupReviewItem>) {
        let mut clusters = Vec::new();
        let mut review_items = Vec::new();

        for i in 0..items.len() {
            for j in (i + 1)..items.len() {
                let score = self.similarity(&items[i], &items[j]);
                if score >= self.config.auto_cluster_threshold {
                    let cluster_id = format!(
                        "cluster-{}-{}",
                        items[i].canonical_key.replace(':', "_"),
                        items[j].canonical_key.replace(':', "_")
                    );
                    clusters.push(DedupClusterProposal {
                        cluster_id,
                        confidence_score: score,
                        members: vec![items[i].canonical_key.clone(), items[j].canonical_key.clone()],
                        review_required: false,
                    });
                    items[i].dedup_confidence = Some(score);
                    items[j].dedup_confidence = Some(score);
                } else if score >= self.config.review_threshold {
                    review_items.push(DedupReviewItem {
                        canonical_key_a: items[i].canonical_key.clone(),
                        canonical_key_b: items[j].canonical_key.clone(),
                        confidence_score: score,
                    });
                    items[i].review_required = true;
                    items[j].review_required = true;
                    items[i].dedup_confidence = Some(score);
                    items[j].dedup_confidence = Some(score);
                }
            }
        }

        (items, clusters, review_items)
    }
}

pub struct DedupHookEngine {
    engine: DedupEngine,
}

impl DedupHookEngine {
    pub fn new(engine: DedupEngine) -> Self {
        Self { engine }
    }
}

impl DedupHook for DedupHookEngine {
    fn apply(&self, items: Vec<StagedOpportunity>) -> Result<Vec<StagedOpportunity>> {
        let (items, _clusters, _review_items) = self.engine.apply(items);
        Ok(items)
    }
}

#[derive(Debug, Clone, Deserialize)]
struct TagRulesFile {
    #[allow(dead_code)]
    version: u32,
    #[serde(default)]
    rules: Vec<TagRule>,
}

#[derive(Debug, Clone, Deserialize)]
struct TagRule {
    tag: String,
    contains_any: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct RiskRulesFile {
    #[allow(dead_code)]
    version: u32,
    #[serde(default)]
    rules: Vec<RiskRule>,
}

#[derive(Debug, Clone, Deserialize)]
struct RiskRule {
    risk_flag: String,
    contains_any: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct PayRulesFile {
    #[allow(dead_code)]
    version: u32,
    #[serde(default)]
    rules: Vec<PayRule>,
}

#[derive(Debug, Clone, Deserialize)]
struct PayRule {
    pay_model_hint: String,
    normalize_to: String,
}

pub struct YamlRuleEnrichmentHook {
    tag_rules: Vec<TagRule>,
    risk_rules: Vec<RiskRule>,
    pay_rules: Vec<PayRule>,
}

impl YamlRuleEnrichmentHook {
    pub fn from_workspace_root(root: &PathBuf) -> Result<Self> {
        let rules_dir = root.join("rules");
        let tags: TagRulesFile = serde_yaml::from_str(
            &std::fs::read_to_string(rules_dir.join("tags.yaml")).context("reading rules/tags.yaml")?,
        )
        .context("parsing rules/tags.yaml")?;
        let risks: RiskRulesFile = serde_yaml::from_str(
            &std::fs::read_to_string(rules_dir.join("risk.yaml")).context("reading rules/risk.yaml")?,
        )
        .context("parsing rules/risk.yaml")?;
        let pay: PayRulesFile = serde_yaml::from_str(
            &std::fs::read_to_string(rules_dir.join("pay.yaml")).context("reading rules/pay.yaml")?,
        )
        .context("parsing rules/pay.yaml")?;
        Ok(Self {
            tag_rules: tags.rules,
            risk_rules: risks.rules,
            pay_rules: pay.rules,
        })
    }
}

impl EnrichmentHook for YamlRuleEnrichmentHook {
    fn apply(&self, mut items: Vec<StagedOpportunity>) -> Result<Vec<StagedOpportunity>> {
        for item in &mut items {
            let title = item
                .draft
                .title
                .value
                .as_deref()
                .unwrap_or_default()
                .to_ascii_lowercase();
            let description = item
                .draft
                .description
                .value
                .as_deref()
                .unwrap_or_default()
                .to_ascii_lowercase();
            let combined = format!("{title} {description}");

            for rule in &self.tag_rules {
                if rule
                    .contains_any
                    .iter()
                    .any(|needle| combined.contains(&needle.to_ascii_lowercase()))
                    && !item.tags.contains(&rule.tag)
                {
                    item.tags.push(rule.tag.clone());
                }
            }

            for rule in &self.risk_rules {
                if rule
                    .contains_any
                    .iter()
                    .any(|needle| combined.contains(&needle.to_ascii_lowercase()))
                    && !item.risk_flags.contains(&rule.risk_flag)
                {
                    item.risk_flags.push(rule.risk_flag.clone());
                }
            }

            if let Some(pay_model) = item.draft.pay_model.value.clone() {
                for rule in &self.pay_rules {
                    if pay_model.eq_ignore_ascii_case(&rule.pay_model_hint) {
                        item.draft.pay_model.value = Some(rule.normalize_to.clone());
                    }
                }
            }
        }
        Ok(items)
    }
}

pub struct SyncPipeline {
    config: SyncConfig,
    artifact_store: ArtifactStore,
    http: HttpFetcher,
    dedup: Box<dyn DedupHook>,
    enrichment: Box<dyn EnrichmentHook>,
}

impl SyncPipeline {
    pub fn new(config: SyncConfig) -> Result<Self> {
        let artifact_store = ArtifactStore::new(config.artifacts_dir.clone());
        let http = HttpFetcher::new(HttpClientConfig {
            timeout: Duration::from_secs(config.http_timeout_secs),
            user_agent: Some(config.user_agent.clone()),
            ..Default::default()
        })?;
        Ok(Self {
            config,
            artifact_store,
            http,
            dedup: Box::<NoopDedupHook>::default(),
            enrichment: Box::<NoopEnrichmentHook>::default(),
        })
    }

    pub fn with_hooks(
        mut self,
        dedup: Box<dyn DedupHook>,
        enrichment: Box<dyn EnrichmentHook>,
    ) -> Self {
        self.dedup = dedup;
        self.enrichment = enrichment;
        self
    }

    pub async fn run_once(&self) -> Result<SyncRunSummary> {
        let started_at = Utc::now();
        let run_id = Uuid::new_v4();
        let registry = self.load_source_registry().await?;
        let pool = self.connect_db().await?;
        let source_ids = self.upsert_sources(&pool, &registry.sources).await?;
        self.insert_fetch_run_started(&pool, run_id, started_at).await?;
        let enabled_sources: Vec<_> = registry.sources.into_iter().filter(|s| s.enabled).collect();

        let mut fetched_artifacts = 0usize;
        let mut parsed_drafts = 0usize;
        let mut staged = Vec::new();

        for source in &enabled_sources {
            let adapter = adapter_for_source(&source.source_id)
                .with_context(|| format!("no adapter registered for {}", source.source_id))?;

            let bundle_path = self.bundle_path_for(source);
            let bundle = if source.mode == "manual" {
                load_manual_fixture_bundle(&bundle_path)?
            } else {
                load_fixture_bundle(&bundle_path)?
            };

            let source_db_id = *source_ids
                .get(&source.source_id)
                .with_context(|| format!("source_id missing from upsert map: {}", source.source_id))?;
            self.store_fixture_raw_artifact(&pool, run_id, source_db_id, &bundle)
                .await?;
            fetched_artifacts += 1;

            let drafts = adapter.parse_listing(&bundle)?;
            parsed_drafts += drafts.len();
            for draft in drafts {
                warn_if_evidence_missing(&draft);
                let canonical_key = normalize_canonical_key(&draft);
                staged.push(StagedOpportunity {
                    source_id: source.source_id.clone(),
                    canonical_key,
                    version_no: 1,
                    dedup_confidence: None,
                    review_required: false,
                    tags: Vec::new(),
                    risk_flags: Vec::new(),
                    draft,
                });
            }

            let _ = &self.http;
        }

        let staged = self.dedup.apply(staged)?;
        let staged = self.enrichment.apply(staged)?;
        let persisted_versions = self.persist_staged(&pool, &source_ids, &staged).await?;
        self.persist_dedup_clusters(&pool, &staged).await?;

        let finished_at = Utc::now();
        let reports_dir = self.write_reports(run_id, started_at, finished_at, &enabled_sources, &staged).await?;
        let manifest_path = self
            .export_parquet_snapshots(&reports_dir, run_id, &enabled_sources, &staged)
            .await?;
        self.insert_fetch_run_finished(
            &pool,
            run_id,
            finished_at,
            fetched_artifacts,
            parsed_drafts,
            persisted_versions,
        )
        .await?;

        Ok(SyncRunSummary {
            run_id,
            started_at,
            finished_at,
            enabled_sources: enabled_sources.len(),
            fetched_artifacts,
            parsed_drafts,
            persisted_versions,
            reports_dir: reports_dir.display().to_string(),
            parquet_manifest: manifest_path.display().to_string(),
        })
    }

    pub async fn maybe_build_scheduler(&self) -> Result<Option<JobScheduler>> {
        if !self.config.scheduler_enabled {
            return Ok(None);
        }

        let sched = JobScheduler::new().await.context("creating scheduler")?;
        for cron in [&self.config.sync_cron_1, &self.config.sync_cron_2] {
            let cfg = self.config.clone();
            let job = Job::new_async(cron, move |_uuid, _l| {
                let cfg = cfg.clone();
                Box::pin(async move {
                    match run_sync_once_with_config(cfg).await {
                        Ok(summary) => {
                            info!(
                                run_id = %summary.run_id,
                                sources = summary.enabled_sources,
                                drafts = summary.parsed_drafts,
                                "scheduler sync completed"
                            );
                        }
                        Err(err) => {
                            warn!(error = %err, "scheduler sync failed");
                        }
                    }
                })
            })
            .with_context(|| format!("creating scheduler job for cron {cron}"))?;
            sched.add(job).await.context("adding scheduler job")?;
        }
        Ok(Some(sched))
    }

    async fn load_source_registry(&self) -> Result<SourceRegistry> {
        let path = self.config.workspace_root.join("sources.yaml");
        let text = fs::read_to_string(&path)
            .await
            .with_context(|| format!("reading {}", path.display()))?;
        serde_yaml::from_str(&text).with_context(|| format!("parsing {}", path.display()))
    }

    fn bundle_path_for(&self, source: &SourceConfig) -> PathBuf {
        if source.mode == "manual" {
            self.config
                .workspace_root
                .join("manual")
                .join(&source.source_id)
                .join("sample.json")
        } else {
            self.config
                .workspace_root
                .join("fixtures")
                .join(&source.source_id)
                .join("sample")
                .join("bundle.json")
        }
    }

    async fn connect_db(&self) -> Result<PgPool> {
        PgPool::connect(&self.config.database_url)
            .await
            .with_context(|| format!("connecting to {}", self.config.database_url))
    }

    async fn upsert_sources(
        &self,
        pool: &PgPool,
        sources: &[SourceConfig],
    ) -> Result<HashMap<String, Uuid>> {
        let mut out = HashMap::new();
        for src in sources {
            let config_json = json!({
                "mode": src.mode,
                "listing_urls": src.listing_urls,
                "detail_url_patterns": src.detail_url_patterns,
                "notes": src.notes,
            });
            let row = sqlx::query(
                r#"
                INSERT INTO sources (source_id, display_name, crawlability, enabled, config_json, updated_at)
                VALUES ($1, $2, $3, $4, $5::jsonb, NOW())
                ON CONFLICT (source_id) DO UPDATE
                  SET display_name = EXCLUDED.display_name,
                      crawlability = EXCLUDED.crawlability,
                      enabled = EXCLUDED.enabled,
                      config_json = EXCLUDED.config_json,
                      updated_at = NOW()
                RETURNING id
                "#,
            )
            .bind(&src.source_id)
            .bind(&src.display_name)
            .bind(format!("{:?}", src.crawlability))
            .bind(src.enabled)
            .bind(config_json)
            .fetch_one(pool)
            .await
            .with_context(|| format!("upserting source {}", src.source_id))?;
            out.insert(src.source_id.clone(), row.try_get("id")?);
        }
        Ok(out)
    }

    async fn insert_fetch_run_started(&self, pool: &PgPool, run_id: Uuid, started_at: DateTime<Utc>) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO fetch_runs (id, started_at, status, summary_json, created_at)
            VALUES ($1, $2, 'started', '{}'::jsonb, NOW())
            ON CONFLICT (id) DO NOTHING
            "#,
        )
        .bind(run_id)
        .bind(started_at)
        .execute(pool)
        .await
        .context("inserting fetch_runs started row")?;
        Ok(())
    }

    async fn insert_fetch_run_finished(
        &self,
        pool: &PgPool,
        run_id: Uuid,
        finished_at: DateTime<Utc>,
        fetched_artifacts: usize,
        parsed_drafts: usize,
        persisted_versions: usize,
    ) -> Result<()> {
        let summary = json!({
            "fetched_artifacts": fetched_artifacts,
            "parsed_drafts": parsed_drafts,
            "persisted_versions": persisted_versions,
            "database_url": self.config.database_url,
        });
        sqlx::query(
            r#"
            UPDATE fetch_runs
               SET finished_at = $2,
                   status = 'completed',
                   summary_json = $3::jsonb
             WHERE id = $1
            "#,
        )
        .bind(run_id)
        .bind(finished_at)
        .bind(summary)
        .execute(pool)
        .await
        .context("updating fetch_runs finished row")?;
        Ok(())
    }

    async fn persist_staged(
        &self,
        pool: &PgPool,
        source_ids: &HashMap<String, Uuid>,
        staged: &[StagedOpportunity],
    ) -> Result<usize> {
        let mut inserted_versions = 0usize;
        for item in staged {
            let source_db_id = *source_ids
                .get(&item.source_id)
                .with_context(|| format!("missing source db id for {}", item.source_id))?;

            let op_row = sqlx::query(
                r#"
                SELECT id, current_version_id
                  FROM opportunities
                 WHERE canonical_key = $1
                 ORDER BY created_at ASC
                 LIMIT 1
                "#,
            )
            .bind(&item.canonical_key)
            .fetch_optional(pool)
            .await
            .with_context(|| format!("loading opportunity {}", item.canonical_key))?;

            let opportunity_id = if let Some(row) = op_row {
                let id: Uuid = row.try_get("id")?;
                sqlx::query(
                    r#"
                    UPDATE opportunities
                       SET source_id = $2,
                           apply_url = $3,
                           last_seen_at = NOW(),
                           updated_at = NOW()
                     WHERE id = $1
                    "#,
                )
                .bind(id)
                .bind(source_db_id)
                .bind(item.draft.apply_url.value.as_deref())
                .execute(pool)
                .await
                .with_context(|| format!("updating opportunity {}", item.canonical_key))?;
                id
            } else {
                let row = sqlx::query(
                    r#"
                    INSERT INTO opportunities (source_id, canonical_key, apply_url, status, first_seen_at, last_seen_at, created_at, updated_at)
                    VALUES ($1, $2, $3, 'active', NOW(), NOW(), NOW(), NOW())
                    RETURNING id
                    "#,
                )
                .bind(source_db_id)
                .bind(&item.canonical_key)
                .bind(item.draft.apply_url.value.as_deref())
                .fetch_one(pool)
                .await
                .with_context(|| format!("inserting opportunity {}", item.canonical_key))?;
                row.try_get("id")?
            };

            let raw_artifact_id = draft_raw_artifact_id(&item.draft);
            let data_json = serde_json::to_value(item).context("serializing staged opportunity")?;
            let evidence_json = serde_json::to_value(&item.draft).context("serializing evidence payload")?;

            let latest_version_row = sqlx::query(
                r#"
                SELECT id, version_no, data_json
                  FROM opportunity_versions
                 WHERE opportunity_id = $1
                 ORDER BY version_no DESC
                 LIMIT 1
                "#,
            )
            .bind(opportunity_id)
            .fetch_optional(pool)
            .await
            .with_context(|| format!("loading latest version for {}", item.canonical_key))?;

            let current_version_id: Option<Uuid> = if let Some(row) = latest_version_row {
                let existing_id: Uuid = row.try_get("id")?;
                let existing_data: serde_json::Value = row.try_get("data_json")?;
                if existing_data != data_json {
                    let latest_version_no: i32 = row.try_get("version_no")?;
                    let new_version_id = Uuid::new_v4();
                    sqlx::query(
                        r#"
                        INSERT INTO opportunity_versions (id, opportunity_id, raw_artifact_id, version_no, data_json, diff_json, evidence_json, created_at)
                        VALUES ($1, $2, $3, $4, $5::jsonb, '{}'::jsonb, $6::jsonb, NOW())
                        "#,
                    )
                    .bind(new_version_id)
                    .bind(opportunity_id)
                    .bind(raw_artifact_id)
                    .bind(latest_version_no + 1)
                    .bind(data_json.clone())
                    .bind(evidence_json.clone())
                    .execute(pool)
                    .await
                    .with_context(|| format!("inserting opportunity version {}", item.canonical_key))?;
                    inserted_versions += 1;
                    Some(new_version_id)
                } else {
                    Some(existing_id)
                }
            } else {
                let new_version_id = Uuid::new_v4();
                sqlx::query(
                    r#"
                    INSERT INTO opportunity_versions (id, opportunity_id, raw_artifact_id, version_no, data_json, diff_json, evidence_json, created_at)
                    VALUES ($1, $2, $3, 1, $4::jsonb, '{}'::jsonb, $5::jsonb, NOW())
                    "#,
                )
                .bind(new_version_id)
                .bind(opportunity_id)
                .bind(raw_artifact_id)
                .bind(data_json.clone())
                .bind(evidence_json.clone())
                .execute(pool)
                .await
                .with_context(|| format!("inserting first opportunity version {}", item.canonical_key))?;
                inserted_versions += 1;
                Some(new_version_id)
            };

            sqlx::query(
                r#"
                UPDATE opportunities
                   SET current_version_id = $2,
                       source_id = $3,
                       apply_url = $4,
                       last_seen_at = NOW(),
                       updated_at = NOW()
                 WHERE id = $1
                "#,
            )
            .bind(opportunity_id)
            .bind(current_version_id)
            .bind(source_db_id)
            .bind(item.draft.apply_url.value.as_deref())
            .execute(pool)
            .await
            .with_context(|| format!("updating current version for {}", item.canonical_key))?;

            self.persist_tags(pool, opportunity_id, &item.tags).await?;
            self.persist_risk_flags(pool, opportunity_id, &item.risk_flags).await?;
            self.persist_review_item(pool, opportunity_id, item).await?;
        }

        Ok(inserted_versions)
    }

    async fn persist_dedup_clusters(&self, pool: &PgPool, staged: &[StagedOpportunity]) -> Result<()> {
        if staged.len() < 2 {
            return Ok(());
        }
        let canonical_to_opportunity = self
            .load_opportunity_ids_by_canonical_keys(pool, staged)
            .await
            .context("loading opportunity ids for dedup cluster persistence")?;

        let engine = DedupEngine::new(DedupConfig::default());
        let (_items, auto_clusters, review_pairs) = engine.apply(staged.to_vec());

        for cluster in auto_clusters {
            self.upsert_cluster_and_members(
                pool,
                &canonical_to_opportunity,
                &cluster.cluster_id,
                "proposed",
                cluster.confidence_score,
                &cluster.members,
            )
            .await?;
        }

        for review in review_pairs {
            let mut members = vec![review.canonical_key_a.clone(), review.canonical_key_b.clone()];
            members.sort();
            members.dedup();
            let cluster_key = format!("review:{}|{}", members[0], members[1]);
            self.upsert_cluster_and_members(
                pool,
                &canonical_to_opportunity,
                &cluster_key,
                "needs_review",
                review.confidence_score,
                &members,
            )
            .await?;
        }

        Ok(())
    }

    async fn load_opportunity_ids_by_canonical_keys(
        &self,
        pool: &PgPool,
        staged: &[StagedOpportunity],
    ) -> Result<HashMap<String, Uuid>> {
        let mut out = HashMap::new();
        for item in staged {
            if out.contains_key(&item.canonical_key) {
                continue;
            }
            let row = sqlx::query(
                r#"
                SELECT id
                  FROM opportunities
                 WHERE canonical_key = $1
                 ORDER BY created_at ASC
                 LIMIT 1
                "#,
            )
            .bind(&item.canonical_key)
            .fetch_optional(pool)
            .await
            .with_context(|| format!("looking up opportunity id for {}", item.canonical_key))?;
            if let Some(row) = row {
                out.insert(item.canonical_key.clone(), row.try_get("id")?);
            }
        }
        Ok(out)
    }

    async fn upsert_cluster_and_members(
        &self,
        pool: &PgPool,
        canonical_to_opportunity: &HashMap<String, Uuid>,
        cluster_key: &str,
        status: &str,
        confidence_score: f64,
        members: &[String],
    ) -> Result<()> {
        let cluster_id = Uuid::new_v5(&Uuid::NAMESPACE_URL, cluster_key.as_bytes());
        sqlx::query(
            r#"
            INSERT INTO dedup_clusters (id, confidence_score, status, created_at, updated_at)
            VALUES ($1, $2, $3, NOW(), NOW())
            ON CONFLICT (id) DO UPDATE
              SET confidence_score = EXCLUDED.confidence_score,
                  status = EXCLUDED.status,
                  updated_at = NOW()
            "#,
        )
        .bind(cluster_id)
        .bind(confidence_score)
        .bind(status)
        .execute(pool)
        .await
        .with_context(|| format!("upserting dedup cluster {}", cluster_key))?;

        for canonical_key in members {
            let Some(opportunity_id) = canonical_to_opportunity.get(canonical_key).copied() else {
                continue;
            };
            sqlx::query(
                r#"
                INSERT INTO dedup_cluster_members (dedup_cluster_id, opportunity_id, member_score, is_primary, created_at)
                VALUES ($1, $2, $3, false, NOW())
                ON CONFLICT (dedup_cluster_id, opportunity_id) DO UPDATE
                  SET member_score = EXCLUDED.member_score
                "#,
            )
            .bind(cluster_id)
            .bind(opportunity_id)
            .bind(confidence_score)
            .execute(pool)
            .await
            .with_context(|| format!("upserting dedup cluster member {}", canonical_key))?;
        }

        Ok(())
    }

    async fn persist_tags(&self, pool: &PgPool, opportunity_id: Uuid, tags: &[String]) -> Result<()> {
        for tag in tags {
            let row = sqlx::query(
                r#"
                INSERT INTO tags (key, label, created_at)
                VALUES ($1, $2, NOW())
                ON CONFLICT (key) DO UPDATE SET label = EXCLUDED.label
                RETURNING id
                "#,
            )
            .bind(tag)
            .bind(tag)
            .fetch_one(pool)
            .await
            .with_context(|| format!("upserting tag {}", tag))?;
            let tag_id: Uuid = row.try_get("id")?;
            sqlx::query(
                r#"
                INSERT INTO opportunity_tags (opportunity_id, tag_id, created_at)
                VALUES ($1, $2, NOW())
                ON CONFLICT (opportunity_id, tag_id) DO NOTHING
                "#,
            )
            .bind(opportunity_id)
            .bind(tag_id)
            .execute(pool)
            .await
            .context("linking opportunity tag")?;
        }
        Ok(())
    }

    async fn persist_risk_flags(
        &self,
        pool: &PgPool,
        opportunity_id: Uuid,
        flags: &[String],
    ) -> Result<()> {
        for flag in flags {
            let row = sqlx::query(
                r#"
                INSERT INTO risk_flags (key, label, severity, created_at)
                VALUES ($1, $2, 'info', NOW())
                ON CONFLICT (key) DO UPDATE SET label = EXCLUDED.label
                RETURNING id
                "#,
            )
            .bind(flag)
            .bind(flag)
            .fetch_one(pool)
            .await
            .with_context(|| format!("upserting risk flag {}", flag))?;
            let flag_id: Uuid = row.try_get("id")?;
            sqlx::query(
                r#"
                INSERT INTO opportunity_risk_flags (opportunity_id, risk_flag_id, reason, created_at)
                VALUES ($1, $2, NULL, NOW())
                ON CONFLICT (opportunity_id, risk_flag_id) DO NOTHING
                "#,
            )
            .bind(opportunity_id)
            .bind(flag_id)
            .execute(pool)
            .await
            .context("linking opportunity risk flag")?;
        }
        Ok(())
    }

    async fn persist_review_item(&self, pool: &PgPool, opportunity_id: Uuid, item: &StagedOpportunity) -> Result<()> {
        if !item.review_required {
            return Ok(());
        }
        let existing = sqlx::query(
            r#"
            SELECT id
              FROM review_items
             WHERE opportunity_id = $1
               AND item_type = 'dedup_review'
               AND status = 'open'
             LIMIT 1
            "#,
        )
        .bind(opportunity_id)
        .fetch_optional(pool)
        .await
        .context("checking existing review item")?;
        if existing.is_some() {
            return Ok(());
        }
        let payload = json!({
            "canonical_key": item.canonical_key,
            "dedup_confidence": item.dedup_confidence,
            "source_id": item.source_id,
        });
        sqlx::query(
            r#"
            INSERT INTO review_items (item_type, status, opportunity_id, payload_json, created_at)
            VALUES ('dedup_review', 'open', $1, $2::jsonb, NOW())
            "#,
        )
        .bind(opportunity_id)
        .bind(payload)
        .execute(pool)
        .await
        .context("inserting review item")?;
        Ok(())
    }

    async fn store_fixture_raw_artifact(
        &self,
        pool: &PgPool,
        run_id: Uuid,
        source_db_id: Uuid,
        bundle: &FixtureBundle,
    ) -> Result<()> {
        let bytes = if let Some(inline_text) = &bundle.raw_artifact.inline_text {
            inline_text.as_bytes().to_vec()
        } else if let Some(rel_path) = &bundle.raw_artifact.path {
            let bundle_base = self
                .config
                .workspace_root
                .join("fixtures")
                .join(&bundle.source_id)
                .join("sample");
            let raw_path = bundle_base.join(rel_path);
            fs::read(&raw_path)
                .await
                .with_context(|| format!("reading raw artifact {}", raw_path.display()))?
        } else {
            Vec::new()
        };

        let ext = match bundle.raw_artifact.content_type.as_str() {
            "text/html" => "html",
            "application/json" => "json",
            _ => "bin",
        };
        let stored = self
            .artifact_store
            .store_bytes(bundle.fetched_at, &bundle.source_id, ext, &bytes)
            .await?;
        let raw_artifact_id = deterministic_raw_artifact_id_for_bundle(bundle);
        sqlx::query(
            r#"
            INSERT INTO raw_artifacts (
                id, fetch_run_id, source_id, source_url, storage_path, content_type, content_hash,
                http_status, byte_size, fetched_at, metadata_json, created_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, NULL, $8, $9, $10::jsonb, NOW())
            ON CONFLICT (id) DO UPDATE
              SET storage_path = EXCLUDED.storage_path,
                  content_type = EXCLUDED.content_type,
                  content_hash = EXCLUDED.content_hash,
                  byte_size = EXCLUDED.byte_size,
                  fetched_at = EXCLUDED.fetched_at,
                  metadata_json = EXCLUDED.metadata_json
            "#,
        )
        .bind(raw_artifact_id)
        .bind(run_id)
        .bind(source_db_id)
        .bind(&bundle.captured_from_url)
        .bind(stored.relative_path.display().to_string())
        .bind(&bundle.raw_artifact.content_type)
        .bind(&stored.content_hash)
        .bind(stored.byte_size as i64)
        .bind(bundle.fetched_at)
        .bind(json!({
            "fixture_id": bundle.fixture_id,
            "extractor_version": bundle.extractor_version,
            "evidence_coverage_percent": bundle.evidence_coverage_percent,
        }))
        .execute(pool)
        .await
        .with_context(|| format!("upserting raw artifact row for {}", bundle.source_id))?;
        Ok(())
    }

    async fn write_reports(
        &self,
        run_id: Uuid,
        started_at: DateTime<Utc>,
        finished_at: DateTime<Utc>,
        enabled_sources: &[SourceConfig],
        staged: &[StagedOpportunity],
    ) -> Result<PathBuf> {
        let reports_dir = self.config.workspace_root.join("reports").join(run_id.to_string());
        fs::create_dir_all(&reports_dir)
            .await
            .with_context(|| format!("creating {}", reports_dir.display()))?;

        let fetch_run = FetchRunRecord {
            run_id,
            started_at,
            finished_at,
            status: "completed".to_string(),
            database_url: self.config.database_url.clone(),
            persistence_mode: "db-persisted + reports/parquet export".to_string(),
        };

        let mut source_counts: BTreeMap<String, usize> = BTreeMap::new();
        for item in staged {
            *source_counts.entry(item.source_id.clone()).or_default() += 1;
        }

        let brief = format!(
            "# RHOF Daily Brief\n\n- Run ID: `{}`\n- Started: {}\n- Finished: {}\n- Enabled sources: {}\n- Parsed opportunities: {}\n\n## Source Counts\n{}\n",
            fetch_run.run_id,
            fetch_run.started_at,
            fetch_run.finished_at,
            enabled_sources.len(),
            staged.len(),
            source_counts
                .iter()
                .map(|(k, v)| format!("- {}: {}", k, v))
                .collect::<Vec<_>>()
                .join("\n")
        );
        fs::write(reports_dir.join("daily_brief.md"), brief)
            .await
            .context("writing daily_brief.md")?;

        let delta_json = serde_json::to_vec_pretty(&serde_json::json!({
            "fetch_run": fetch_run,
            "opportunities": staged,
        }))
        .context("serializing opportunities delta")?;
        fs::write(reports_dir.join("opportunities_delta.json"), delta_json)
            .await
            .context("writing opportunities_delta.json")?;

        Ok(reports_dir)
    }

    async fn export_parquet_snapshots(
        &self,
        reports_dir: &PathBuf,
        run_id: Uuid,
        enabled_sources: &[SourceConfig],
        staged: &[StagedOpportunity],
    ) -> Result<PathBuf> {
        let snapshot_dir = reports_dir.join("snapshots");
        fs::create_dir_all(&snapshot_dir)
            .await
            .with_context(|| format!("creating {}", snapshot_dir.display()))?;

        let opportunities_path = snapshot_dir.join("opportunities.parquet");
        let versions_path = snapshot_dir.join("opportunity_versions.parquet");
        let tags_path = snapshot_dir.join("tags.parquet");
        let sources_path = snapshot_dir.join("sources.parquet");

        write_opportunities_parquet(&opportunities_path, staged)?;
        write_opportunity_versions_parquet(&versions_path, staged)?;
        write_tags_parquet(&tags_path, staged)?;
        write_sources_parquet(&sources_path, enabled_sources)?;

        let manifest = ParquetManifest {
            schema_version: 1,
            files: vec![
                manifest_entry("opportunities", reports_dir, &opportunities_path)?,
                manifest_entry("opportunity_versions", reports_dir, &versions_path)?,
                manifest_entry("tags", reports_dir, &tags_path)?,
                manifest_entry("sources", reports_dir, &sources_path)?,
            ],
        };

        let manifest_path = snapshot_dir.join("manifest.json");
        let bytes = serde_json::to_vec_pretty(&manifest).context("serializing parquet manifest")?;
        fs::write(&manifest_path, bytes)
            .await
            .with_context(|| format!("writing {}", manifest_path.display()))?;

        let _ = run_id;
        Ok(manifest_path)
    }
}

pub async fn run_sync_once_with_config(config: SyncConfig) -> Result<SyncRunSummary> {
    let enrichment = YamlRuleEnrichmentHook::from_workspace_root(&config.workspace_root)?;
    let dedup = DedupHookEngine::new(DedupEngine::new(DedupConfig::default()));
    let pipeline = SyncPipeline::new(config)?.with_hooks(Box::new(dedup), Box::new(enrichment));
    pipeline.run_once().await
}

fn draft_raw_artifact_id(draft: &OpportunityDraft) -> Option<Uuid> {
    [
        &draft.title.evidence,
        &draft.description.evidence,
        &draft.pay_model.evidence,
        &draft.currency.evidence,
        &draft.apply_url.evidence,
    ]
    .into_iter()
    .flatten()
    .map(|e| e.raw_artifact_id)
    .next()
}

pub async fn apply_migrations_from_env() -> Result<()> {
    let cfg = SyncConfig::from_env();
    let pool = PgPool::connect(&cfg.database_url)
        .await
        .with_context(|| format!("connecting to {}", cfg.database_url))?;
    MIGRATOR.run(&pool).await.context("running sqlx migrations")?;
    Ok(())
}

pub async fn run_scheduler_forever_from_env() -> Result<()> {
    let config = SyncConfig::from_env();
    let enrichment = YamlRuleEnrichmentHook::from_workspace_root(&config.workspace_root)?;
    let dedup = DedupHookEngine::new(DedupEngine::new(DedupConfig::default()));
    let pipeline = SyncPipeline::new(config.clone())?.with_hooks(Box::new(dedup), Box::new(enrichment));
    let Some(mut sched) = pipeline.maybe_build_scheduler().await? else {
        anyhow::bail!("RHOF_SCHEDULER_ENABLED=false; enable it to run scheduler mode");
    };
    info!("scheduler started; waiting for cron triggers (Ctrl+C to stop)");
    sched.start().await.context("starting scheduler")?;
    tokio::signal::ctrl_c().await.context("waiting for Ctrl+C")?;
    info!("scheduler shutdown requested");
    sched.shutdown().await.context("shutting down scheduler")?;
    Ok(())
}

pub async fn run_sync_once_from_env() -> Result<SyncRunSummary> {
    run_sync_once_with_config(SyncConfig::from_env()).await
}

pub async fn seed_from_fixtures_from_env() -> Result<SyncRunSummary> {
    // Current seed behavior reuses the fixture-driven sync pipeline. It remains deterministic
    // because fixture bundles are checked in and artifact paths are hash-addressed.
    run_sync_once_from_env().await
}

pub fn debug_summary_from_env() -> Result<String> {
    let cfg = SyncConfig::from_env();
    let reports_md = report_daily_markdown(3, Some(cfg.workspace_root.clone()))
        .unwrap_or_else(|e| format!("(report summary unavailable: {e})"));
    Ok(format!(
        "RHOF Debug Summary\n\n- DATABASE_URL: {}\n- ARTIFACTS_DIR: {}\n- RHOF_SCHEDULER_ENABLED: {}\n- SYNC_CRON_1: {}\n- SYNC_CRON_2: {}\n- RHOF_HTTP_TIMEOUT_SECS: {}\n- RHOF_USER_AGENT: {}\n\n{}",
        cfg.database_url,
        cfg.artifacts_dir.display(),
        cfg.scheduler_enabled,
        cfg.sync_cron_1,
        cfg.sync_cron_2,
        cfg.http_timeout_secs,
        cfg.user_agent,
        reports_md
    ))
}

pub fn report_daily_markdown(runs: usize, workspace_root: Option<PathBuf>) -> Result<String> {
    let root = workspace_root.unwrap_or_else(|| PathBuf::from("."));
    let reports_root = root.join("reports");
    let mut dirs = std::fs::read_dir(&reports_root)
        .with_context(|| format!("reading {}", reports_root.display()))?
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false))
        .collect::<Vec<_>>();
    dirs.sort_by_key(|e| {
        e.metadata()
            .and_then(|m| m.modified())
            .ok()
    });
    dirs.reverse();
    let dirs = dirs.into_iter().take(runs.max(1)).collect::<Vec<_>>();

    let mut lines = vec!["# RHOF Report Daily".to_string(), String::new()];
    for dir in dirs {
        let run_id = dir.file_name().to_string_lossy().to_string();
        let delta_path = dir.path().join("opportunities_delta.json");
        let daily_path = dir.path().join("daily_brief.md");
        let manifest_path = dir.path().join("snapshots").join("manifest.json");

        let delta_value: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(&delta_path)
                .with_context(|| format!("reading {}", delta_path.display()))?,
        )
        .with_context(|| format!("parsing {}", delta_path.display()))?;
        let count = delta_value
            .get("opportunities")
            .and_then(|v| v.as_array())
            .map(|a| a.len())
            .unwrap_or(0);
        let sources = delta_value
            .get("fetch_run")
            .and_then(|v| v.get("database_url"))
            .and_then(|v| v.as_str())
            .unwrap_or("unknown-db");

        lines.push(format!("## Run `{run_id}`"));
        lines.push(format!("- opportunities: {count}"));
        lines.push(format!("- delta: `{}`", delta_path.display()));
        if manifest_path.exists() {
            lines.push(format!("- parquet manifest: `{}`", manifest_path.display()));
        }
        if daily_path.exists() {
            lines.push(format!("- daily brief: `{}`", daily_path.display()));
        }
        lines.push(format!("- persistence target: `{sources}`"));
        lines.push(String::new());
    }

    Ok(lines.join("\n"))
}

fn normalize_canonical_key(draft: &OpportunityDraft) -> String {
    let title = draft
        .title
        .value
        .as_deref()
        .unwrap_or("untitled")
        .to_ascii_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect::<String>();
    format!("{}:{}", draft.source_id, title.trim_matches('-'))
}

fn warn_if_evidence_missing(draft: &OpportunityDraft) {
    let checks = [
        ("title", draft.title.value.is_some(), draft.title.evidence.is_some()),
        (
            "description",
            draft.description.value.is_some(),
            draft.description.evidence.is_some(),
        ),
        (
            "pay_model",
            draft.pay_model.value.is_some(),
            draft.pay_model.evidence.is_some(),
        ),
        (
            "currency",
            draft.currency.value.is_some(),
            draft.currency.evidence.is_some(),
        ),
        (
            "apply_url",
            draft.apply_url.value.is_some(),
            draft.apply_url.evidence.is_some(),
        ),
    ];

    for (field, populated, has_evidence) in checks {
        if populated && !has_evidence {
            warn!(source_id = %draft.source_id, field, "populated canonical field missing evidence");
        }
    }
}

fn write_parquet(path: &PathBuf, batch: RecordBatch) -> Result<()> {
    let file = File::create(path).with_context(|| format!("creating {}", path.display()))?;
    let mut writer = ArrowWriter::try_new(file, batch.schema(), None)
        .with_context(|| format!("opening parquet writer {}", path.display()))?;
    writer
        .write(&batch)
        .with_context(|| format!("writing record batch {}", path.display()))?;
    writer
        .close()
        .with_context(|| format!("closing parquet writer {}", path.display()))?;
    Ok(())
}

fn write_opportunities_parquet(path: &PathBuf, staged: &[StagedOpportunity]) -> Result<()> {
    let schema = Arc::new(Schema::new(vec![
        ArrowField::new("source_id", DataType::Utf8, false),
        ArrowField::new("canonical_key", DataType::Utf8, false),
        ArrowField::new("title", DataType::Utf8, true),
        ArrowField::new("apply_url", DataType::Utf8, true),
        ArrowField::new("review_required", DataType::Boolean, false),
        ArrowField::new("dedup_confidence", DataType::Float64, true),
    ]));

    let source_ids = StringArray::from(
        staged
            .iter()
            .map(|s| Some(s.source_id.as_str()))
            .collect::<Vec<_>>(),
    );
    let canonical_keys = StringArray::from(
        staged
            .iter()
            .map(|s| Some(s.canonical_key.as_str()))
            .collect::<Vec<_>>(),
    );
    let titles = StringArray::from(
        staged
            .iter()
            .map(|s| s.draft.title.value.as_deref())
            .collect::<Vec<_>>(),
    );
    let apply_urls = StringArray::from(
        staged
            .iter()
            .map(|s| s.draft.apply_url.value.as_deref())
            .collect::<Vec<_>>(),
    );
    let reviews = BooleanArray::from(staged.iter().map(|s| s.review_required).collect::<Vec<_>>());
    let confidences = Float64Array::from(staged.iter().map(|s| s.dedup_confidence).collect::<Vec<_>>());

    let batch = RecordBatch::try_new(
        schema,
        vec![
            Arc::new(source_ids),
            Arc::new(canonical_keys),
            Arc::new(titles),
            Arc::new(apply_urls),
            Arc::new(reviews),
            Arc::new(confidences),
        ],
    )
    .context("building opportunities record batch")?;
    write_parquet(path, batch)
}

fn write_opportunity_versions_parquet(path: &PathBuf, staged: &[StagedOpportunity]) -> Result<()> {
    let schema = Arc::new(Schema::new(vec![
        ArrowField::new("canonical_key", DataType::Utf8, false),
        ArrowField::new("version_no", DataType::UInt32, false),
        ArrowField::new("extractor_version", DataType::Utf8, false),
        ArrowField::new("fetched_at", DataType::Utf8, false),
    ]));

    let canonical_keys = StringArray::from(
        staged
            .iter()
            .map(|s| Some(s.canonical_key.as_str()))
            .collect::<Vec<_>>(),
    );
    let version_nos = UInt32Array::from(staged.iter().map(|s| s.version_no).collect::<Vec<_>>());
    let extractor_versions = StringArray::from(
        staged
            .iter()
            .map(|s| Some(s.draft.extractor_version.as_str()))
            .collect::<Vec<_>>(),
    );
    let fetched_at = StringArray::from(
        staged
            .iter()
            .map(|s| Some(s.draft.fetched_at.to_rfc3339()))
            .collect::<Vec<_>>(),
    );

    let batch = RecordBatch::try_new(
        schema,
        vec![
            Arc::new(canonical_keys),
            Arc::new(version_nos),
            Arc::new(extractor_versions),
            Arc::new(fetched_at),
        ],
    )
    .context("building opportunity_versions record batch")?;
    write_parquet(path, batch)
}

fn write_tags_parquet(path: &PathBuf, staged: &[StagedOpportunity]) -> Result<()> {
    let rows = staged
        .iter()
        .flat_map(|s| {
            s.tags
                .iter()
                .map(|tag| (s.canonical_key.clone(), tag.clone()))
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();

    let schema = Arc::new(Schema::new(vec![
        ArrowField::new("canonical_key", DataType::Utf8, false),
        ArrowField::new("tag", DataType::Utf8, false),
    ]));
    let canonical_keys = StringArray::from(
        rows.iter()
            .map(|(k, _)| Some(k.as_str()))
            .collect::<Vec<_>>(),
    );
    let tags = StringArray::from(rows.iter().map(|(_, t)| Some(t.as_str())).collect::<Vec<_>>());
    let batch = RecordBatch::try_new(schema, vec![Arc::new(canonical_keys), Arc::new(tags)])
        .context("building tags record batch")?;
    write_parquet(path, batch)
}

fn write_sources_parquet(path: &PathBuf, sources: &[SourceConfig]) -> Result<()> {
    let schema = Arc::new(Schema::new(vec![
        ArrowField::new("source_id", DataType::Utf8, false),
        ArrowField::new("display_name", DataType::Utf8, false),
        ArrowField::new("crawlability", DataType::Utf8, false),
        ArrowField::new("enabled", DataType::Boolean, false),
        ArrowField::new("mode", DataType::Utf8, false),
    ]));

    let source_ids = StringArray::from(
        sources
            .iter()
            .map(|s| Some(s.source_id.as_str()))
            .collect::<Vec<_>>(),
    );
    let display_names = StringArray::from(
        sources
            .iter()
            .map(|s| Some(s.display_name.as_str()))
            .collect::<Vec<_>>(),
    );
    let crawlability = StringArray::from(
        sources
            .iter()
            .map(|s| Some(format!("{:?}", s.crawlability)))
            .collect::<Vec<_>>(),
    );
    let enabled = BooleanArray::from(sources.iter().map(|s| s.enabled).collect::<Vec<_>>());
    let modes = StringArray::from(
        sources
            .iter()
            .map(|s| Some(s.mode.as_str()))
            .collect::<Vec<_>>(),
    );

    let batch = RecordBatch::try_new(
        schema,
        vec![
            Arc::new(source_ids),
            Arc::new(display_names),
            Arc::new(crawlability),
            Arc::new(enabled),
            Arc::new(modes),
        ],
    )
    .context("building sources record batch")?;
    write_parquet(path, batch)
}

fn manifest_entry(name: &str, reports_dir: &PathBuf, path: &PathBuf) -> Result<ParquetManifestFile> {
    let bytes = std::fs::read(path).with_context(|| format!("reading {}", path.display()))?;
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    let sha256 = hex::encode(hasher.finalize());
    let rel = path
        .strip_prefix(reports_dir)
        .unwrap_or(path)
        .display()
        .to_string();
    Ok(ParquetManifestFile {
        name: name.to_string(),
        path: rel,
        sha256,
        bytes: bytes.len() as u64,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use rhof_core::Field;
    use sqlx::Row;
    use std::path::Path;
    use tempfile::tempdir;

    fn mk_item(source_id: &str, title: &str) -> StagedOpportunity {
        StagedOpportunity {
            source_id: source_id.to_string(),
            canonical_key: format!("{}:{}", source_id, DedupEngine::normalize_key_fragment(title)),
            version_no: 1,
            dedup_confidence: None,
            review_required: false,
            tags: vec![],
            risk_flags: vec![],
            draft: OpportunityDraft {
                source_id: source_id.to_string(),
                listing_url: None,
                detail_url: None,
                fetched_at: Utc
                    .with_ymd_and_hms(2026, 2, 24, 12, 0, 0)
                    .single()
                    .unwrap(),
                extractor_version: "test".into(),
                title: Field { value: Some(title.to_string()), evidence: None },
                description: Field { value: Some(title.to_string()), evidence: None },
                pay_model: Field::empty(),
                pay_rate_min: Field::empty(),
                pay_rate_max: Field::empty(),
                currency: Field::empty(),
                min_hours_per_week: Field::empty(),
                verification_requirements: Field::empty(),
                geo_constraints: Field::empty(),
                one_off_vs_ongoing: Field::empty(),
                payment_methods: Field::empty(),
                apply_url: Field::empty(),
                requirements: Field::empty(),
            },
        }
    }

    fn copy_dir_recursive(src: &Path, dst: &Path) {
        std::fs::create_dir_all(dst).unwrap();
        for entry in std::fs::read_dir(src).unwrap() {
            let entry = entry.unwrap();
            let src_path = entry.path();
            let dst_path = dst.join(entry.file_name());
            if src_path.is_dir() {
                copy_dir_recursive(&src_path, &dst_path);
            } else {
                if let Some(parent) = dst_path.parent() {
                    std::fs::create_dir_all(parent).unwrap();
                }
                std::fs::copy(&src_path, &dst_path).unwrap();
            }
        }
    }

    fn set_json_path_str(value: &mut serde_json::Value, path: &[&str], new_value: &str) {
        let mut cursor = value;
        for segment in &path[..path.len() - 1] {
            cursor = cursor.get_mut(*segment).unwrap();
        }
        *cursor.get_mut(path[path.len() - 1]).unwrap() = serde_json::Value::String(new_value.to_string());
    }

    fn rewrite_single_record_html_bundle(bundle_path: &Path, raw_html_path: &Path, title: &str, apply_url: &str) {
        let mut bundle: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(bundle_path).unwrap()).unwrap();
        let first = bundle["parsed_records"][0].clone();
        let mut record = first;
        set_json_path_str(&mut record, &["title", "value"], title);
        set_json_path_str(&mut record, &["title", "snippet"], title);
        set_json_path_str(&mut record, &["description", "value"], &format!("Description for {title}"));
        set_json_path_str(&mut record, &["description", "snippet"], title);
        set_json_path_str(&mut record, &["apply_url", "value"], apply_url);
        set_json_path_str(&mut record, &["apply_url", "snippet"], apply_url);
        set_json_path_str(&mut record, &["listing_url"], apply_url);
        set_json_path_str(&mut record, &["detail_url"], apply_url);
        bundle["parsed_records"] = serde_json::Value::Array(vec![record]);
        std::fs::write(bundle_path, serde_json::to_string_pretty(&bundle).unwrap()).unwrap();

        let html = format!(
            "<!doctype html><html><body><h1>{}</h1><a href=\"{}\">Apply</a></body></html>",
            title, apply_url
        );
        std::fs::write(raw_html_path, html).unwrap();
    }

    fn write_single_source_yaml(path: &Path) {
        let yaml = r#"sources:
  - source_id: clickworker
    display_name: Clickworker
    enabled: true
    crawlability: PublicHtml
    mode: fixture
    listing_urls:
      - https://www.clickworker.com/jobs
"#;
        std::fs::write(path, yaml).unwrap();
    }

    #[test]
    fn true_match_clusters() {
        let engine = DedupEngine::new(DedupConfig {
            auto_cluster_threshold: 0.93,
            review_threshold: 0.85,
        });
        let items = vec![
            mk_item("clickworker", "AI Data Contributor"),
            mk_item("clickworker", "AI Data Contributer"),
        ];
        let (_items, clusters, review) = engine.apply(items);
        assert_eq!(clusters.len(), 1);
        assert!(review.is_empty());
        assert!(clusters[0].confidence_score >= 0.93);
    }

    #[test]
    fn false_positive_does_not_cluster() {
        let engine = DedupEngine::new(DedupConfig::default());
        let items = vec![
            mk_item("appen-crowdgen", "Search Relevance Rater"),
            mk_item("prolific", "Paid Academic Study"),
        ];
        let (_items, clusters, review) = engine.apply(items);
        assert!(clusters.is_empty());
        assert!(review.is_empty());
    }

    #[test]
    fn borderline_cluster_goes_to_review_queue() {
        let engine = DedupEngine::new(DedupConfig {
            auto_cluster_threshold: 0.97,
            review_threshold: 0.88,
        });
        let items = vec![
            mk_item("telus-ai-community", "Internet Assessor - US"),
            mk_item("telus-ai-community", "Internet Assessor US (Part-Time)"),
        ];
        let (_items, clusters, review) = engine.apply(items);
        assert!(clusters.is_empty());
        assert_eq!(review.len(), 1);
        assert!(review[0].confidence_score >= 0.88);
    }

    #[tokio::test]
    async fn db_migrate_and_repeated_sync_are_idempotent() {
        let db_url = "postgres://rhof:rhof@localhost:5401/rhof";
        let pool = match PgPool::connect(db_url).await {
            Ok(pool) => pool,
            Err(_) => {
                eprintln!("skipping DB idempotency integration test; local Postgres unavailable");
                return;
            }
        };
        MIGRATOR.run(&pool).await.unwrap();

        let marker = format!(
            "syncit{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        let title = format!("Clickworker Data Task {}", marker);
        let apply_url = format!("https://example.test/{marker}/clickworker");

        let temp = tempdir().unwrap();
        let root = temp.path().to_path_buf();
        std::fs::create_dir_all(root.join("fixtures")).unwrap();
        std::fs::create_dir_all(root.join("rules")).unwrap();
        copy_dir_recursive(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").join("rules").as_path(),
            &root.join("rules"),
        );
        copy_dir_recursive(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../..")
                .join("fixtures/clickworker")
                .as_path(),
            &root.join("fixtures/clickworker"),
        );
        write_single_source_yaml(&root.join("sources.yaml"));
        rewrite_single_record_html_bundle(
            &root.join("fixtures/clickworker/sample/bundle.json"),
            &root.join("fixtures/clickworker/sample/raw/listing.html"),
            &title,
            &apply_url,
        );

        let cfg = SyncConfig {
            database_url: db_url.to_string(),
            artifacts_dir: root.join("artifacts"),
            scheduler_enabled: false,
            sync_cron_1: "0 6 * * *".to_string(),
            sync_cron_2: "0 18 * * *".to_string(),
            user_agent: "rhof-sync-test/0.1".to_string(),
            http_timeout_secs: 5,
            workspace_root: root.clone(),
        };

        let first = run_sync_once_with_config(cfg.clone()).await.unwrap();
        let second = run_sync_once_with_config(cfg).await.unwrap();
        assert_eq!(first.enabled_sources, 1);
        assert_eq!(first.parsed_drafts, 1);
        assert_eq!(second.enabled_sources, 1);
        assert_eq!(second.parsed_drafts, 1);
        assert_eq!(second.persisted_versions, 0, "second sync should not create a new version");

        let opportunity_count: i64 = sqlx::query(
            r#"
            SELECT COUNT(*) AS count
              FROM opportunities
             WHERE apply_url = $1
            "#,
        )
        .bind(&apply_url)
        .fetch_one(&pool)
        .await
        .unwrap()
        .try_get("count")
        .unwrap();
        assert_eq!(opportunity_count, 1);

        let version_count: i64 = sqlx::query(
            r#"
            SELECT COUNT(*) AS count
              FROM opportunity_versions ov
              JOIN opportunities o ON o.id = ov.opportunity_id
             WHERE o.apply_url = $1
            "#,
        )
        .bind(&apply_url)
        .fetch_one(&pool)
        .await
        .unwrap()
        .try_get("count")
        .unwrap();
        assert_eq!(version_count, 1, "idempotent sync should keep one version for unchanged fixture data");

        let completed_runs: i64 = sqlx::query(
            r#"
            SELECT COUNT(*) AS count
              FROM fetch_runs
             WHERE id = ANY($1)
               AND status = 'completed'
            "#,
        )
        .bind(vec![first.run_id, second.run_id])
        .fetch_one(&pool)
        .await
        .unwrap()
        .try_get("count")
        .unwrap();
        assert_eq!(completed_runs, 2);
    }
}
