//! Sync pipeline orchestration (PROMPT_05 staged implementation).

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::Duration;

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use rhof_adapters::{adapter_for_source, load_fixture_bundle, load_manual_fixture_bundle, Crawlability, FixtureBundle};
use rhof_core::OpportunityDraft;
use rhof_storage::{ArtifactStore, HttpClientConfig, HttpFetcher};
use serde::{Deserialize, Serialize};
use tokio::fs;
use tokio_cron_scheduler::{Job, JobScheduler};
use tracing::warn;
use uuid::Uuid;

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

        Ok(SyncRunSummary {
            run_id,
            started_at,
            finished_at,
            enabled_sources: enabled_sources.len(),
            fetched_artifacts,
            parsed_drafts,
            persisted_versions,
            reports_dir: reports_dir.display().to_string(),
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
}

pub async fn run_sync_once_from_env() -> Result<SyncRunSummary> {
    let pipeline = SyncPipeline::new(SyncConfig::from_env())?;
    pipeline.run_once().await
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
