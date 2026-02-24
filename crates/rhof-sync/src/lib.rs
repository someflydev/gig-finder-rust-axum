//! Sync pipeline orchestration (PROMPT_05 staged implementation).

use std::collections::BTreeMap;
use std::fs::File;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use arrow_array::{BooleanArray, Float64Array, RecordBatch, StringArray, UInt32Array};
use arrow_schema::{DataType, Field as ArrowField, Schema};
use chrono::{DateTime, Utc};
use parquet::arrow::ArrowWriter;
use rhof_adapters::{adapter_for_source, load_fixture_bundle, load_manual_fixture_bundle, Crawlability, FixtureBundle};
use rhof_core::OpportunityDraft;
use rhof_storage::{ArtifactStore, HttpClientConfig, HttpFetcher};
use serde::{Deserialize, Serialize};
use strsim::jaro_winkler;
use tokio::fs;
use tokio_cron_scheduler::{Job, JobScheduler};
use tracing::warn;
use uuid::Uuid;
use sha2::{Digest, Sha256};

pub const CRATE_NAME: &str = "rhof-sync";

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

#[derive(Debug, Clone, Serialize)]
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

            self.store_fixture_raw_artifact(&bundle).await?;
            fetched_artifacts += 1;

            let drafts = adapter.parse_listing(&bundle)?;
            parsed_drafts += drafts.len();
            for draft in drafts {
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
        let persisted_versions = staged.len();

        let finished_at = Utc::now();
        let reports_dir = self.write_reports(run_id, started_at, finished_at, &enabled_sources, &staged).await?;
        let manifest_path = self
            .export_parquet_snapshots(&reports_dir, run_id, &enabled_sources, &staged)
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
            let job = Job::new_async(cron, |_uuid, _l| {
                Box::pin(async move {
                    warn!("scheduler job triggered; prompt05 scaffold does not auto-run sync yet");
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

    async fn store_fixture_raw_artifact(&self, bundle: &FixtureBundle) -> Result<()> {
        let bytes = if let Some(inline_text) = &bundle.raw_artifact.inline_text {
            inline_text.as_bytes().to_vec()
        } else if let Some(rel_path) = &bundle.raw_artifact.path {
            let bundle_base = if bundle.source_id == "prolific" {
                self.config
                    .workspace_root
                    .join("fixtures")
                    .join(&bundle.source_id)
                    .join("sample")
            } else {
                self.config
                    .workspace_root
                    .join("fixtures")
                    .join(&bundle.source_id)
                    .join("sample")
            };
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
        let _stored = self
            .artifact_store
            .store_bytes(bundle.fetched_at, &bundle.source_id, ext, &bytes)
            .await?;
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
            persistence_mode: "staged-report-only (DB persistence wiring lands in later prompts)".to_string(),
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

pub async fn run_sync_once_from_env() -> Result<SyncRunSummary> {
    let config = SyncConfig::from_env();
    let enrichment = YamlRuleEnrichmentHook::from_workspace_root(&config.workspace_root)?;
    let dedup = DedupHookEngine::new(DedupEngine::new(DedupConfig::default()));
    let pipeline = SyncPipeline::new(config)?.with_hooks(Box::new(dedup), Box::new(enrichment));
    pipeline.run_once().await
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
}
