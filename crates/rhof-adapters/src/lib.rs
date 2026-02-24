//! Source adapter contracts + fixture-first adapter implementations.

use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use rhof_core::{EvidenceRef, Field, OpportunityDraft};
use rhof_storage::HttpFetcher;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

pub const CRATE_NAME: &str = "rhof-adapters";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Crawlability {
    PublicHtml,
    Api,
    Rss,
    Gated,
    ManualOnly,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FetchedPage {
    pub url: String,
    pub content_type: String,
    pub body: Vec<u8>,
    pub fetched_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdapterContext {
    pub run_id: Uuid,
    pub fetched_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ListingTarget {
    pub url: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DetailTarget {
    pub url: String,
}

#[derive(Debug, Error)]
pub enum AdapterError {
    #[error("{0}")]
    Message(String),
    #[error(transparent)]
    Anyhow(#[from] anyhow::Error),
}

#[async_trait]
pub trait SourceAdapter: Send + Sync {
    fn source_id(&self) -> &'static str;
    fn crawlability(&self) -> Crawlability;

    async fn fetch_listing(
        &self,
        _http: &HttpFetcher,
        _ctx: &AdapterContext,
        _targets: &[ListingTarget],
    ) -> Result<Vec<FetchedPage>, AdapterError>;

    fn parse_listing(&self, bundle: &FixtureBundle) -> Result<Vec<OpportunityDraft>, AdapterError>;

    async fn fetch_detail(
        &self,
        _http: &HttpFetcher,
        _ctx: &AdapterContext,
        _targets: &[DetailTarget],
    ) -> Result<Vec<FetchedPage>, AdapterError>;

    fn parse_detail(&self, bundle: &FixtureBundle) -> Result<Vec<OpportunityDraft>, AdapterError>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FixtureBundle {
    pub fixture_id: String,
    pub source_id: String,
    pub crawlability: Crawlability,
    pub captured_from_url: String,
    pub fetched_at: DateTime<Utc>,
    pub extractor_version: String,
    pub raw_artifact: FixtureRawArtifact,
    pub parsed_records: Vec<FixtureParsedRecord>,
    pub evidence_coverage_percent: f64,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FixtureRawArtifact {
    pub content_type: String,
    pub path: Option<String>,
    pub inline_text: Option<String>,
    pub sha256: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FixtureParsedRecord {
    pub title: FixtureField<String>,
    pub description: FixtureField<String>,
    pub pay_model: FixtureField<String>,
    pub pay_rate_min: FixtureField<f64>,
    pub pay_rate_max: FixtureField<f64>,
    pub currency: FixtureField<String>,
    pub min_hours_per_week: FixtureField<f64>,
    pub verification_requirements: FixtureField<String>,
    pub geo_constraints: FixtureField<String>,
    pub one_off_vs_ongoing: FixtureField<String>,
    pub payment_methods: FixtureField<Vec<String>>,
    pub apply_url: FixtureField<String>,
    pub requirements: FixtureField<Vec<String>>,
    pub listing_url: Option<String>,
    pub detail_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FixtureField<T> {
    pub value: Option<T>,
    pub selector_or_pointer: String,
    pub snippet: String,
}

impl<T> Default for FixtureField<T> {
    fn default() -> Self {
        Self {
            value: None,
            selector_or_pointer: String::new(),
            snippet: String::new(),
        }
    }
}

pub fn load_fixture_bundle(path: impl AsRef<Path>) -> Result<FixtureBundle> {
    read_json_file(path)
}

pub fn load_manual_fixture_bundle(path: impl AsRef<Path>) -> Result<FixtureBundle> {
    read_json_file(path)
}

fn read_json_file<T: DeserializeOwned>(path: impl AsRef<Path>) -> Result<T> {
    let path = path.as_ref();
    let data = fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    serde_json::from_str(&data).with_context(|| format!("parsing {}", path.display()))
}

fn deterministic_raw_artifact_id(bundle: &FixtureBundle) -> Uuid {
    let source = format!(
        "{}:{}:{}",
        bundle.source_id,
        bundle.fixture_id,
        bundle
            .raw_artifact
            .path
            .as_deref()
            .unwrap_or("<inline-artifact>")
    );
    Uuid::new_v5(&Uuid::NAMESPACE_URL, source.as_bytes())
}

fn fixture_field_to_core<T: Clone>(
    fixture: &FixtureField<T>,
    bundle: &FixtureBundle,
) -> Field<T> {
    match &fixture.value {
        Some(value) => Field::with_value_and_evidence(
            value.clone(),
            EvidenceRef {
                raw_artifact_id: deterministic_raw_artifact_id(bundle),
                source_url: bundle.captured_from_url.clone(),
                selector_or_pointer: fixture.selector_or_pointer.clone(),
                snippet: fixture.snippet.clone(),
                fetched_at: bundle.fetched_at,
                extractor_version: bundle.extractor_version.clone(),
            },
        ),
        None => Field::empty(),
    }
}

fn bundle_to_drafts(bundle: &FixtureBundle) -> Vec<OpportunityDraft> {
    bundle
        .parsed_records
        .iter()
        .map(|record| OpportunityDraft {
            source_id: bundle.source_id.clone(),
            listing_url: record.listing_url.clone(),
            detail_url: record.detail_url.clone(),
            fetched_at: bundle.fetched_at,
            extractor_version: bundle.extractor_version.clone(),
            title: fixture_field_to_core(&record.title, bundle),
            description: fixture_field_to_core(&record.description, bundle),
            pay_model: fixture_field_to_core(&record.pay_model, bundle),
            pay_rate_min: fixture_field_to_core(&record.pay_rate_min, bundle),
            pay_rate_max: fixture_field_to_core(&record.pay_rate_max, bundle),
            currency: fixture_field_to_core(&record.currency, bundle),
            min_hours_per_week: fixture_field_to_core(&record.min_hours_per_week, bundle),
            verification_requirements: fixture_field_to_core(
                &record.verification_requirements,
                bundle,
            ),
            geo_constraints: fixture_field_to_core(&record.geo_constraints, bundle),
            one_off_vs_ongoing: fixture_field_to_core(&record.one_off_vs_ongoing, bundle),
            payment_methods: fixture_field_to_core(&record.payment_methods, bundle),
            apply_url: fixture_field_to_core(&record.apply_url, bundle),
            requirements: fixture_field_to_core(&record.requirements, bundle),
        })
        .collect()
}

#[derive(Debug, Clone, Copy)]
struct FixtureFirstAdapter {
    source_id: &'static str,
    crawlability: Crawlability,
}

#[async_trait]
impl SourceAdapter for FixtureFirstAdapter {
    fn source_id(&self) -> &'static str {
        self.source_id
    }

    fn crawlability(&self) -> Crawlability {
        self.crawlability
    }

    async fn fetch_listing(
        &self,
        _http: &HttpFetcher,
        _ctx: &AdapterContext,
        _targets: &[ListingTarget],
    ) -> Result<Vec<FetchedPage>, AdapterError> {
        Ok(Vec::new())
    }

    fn parse_listing(&self, bundle: &FixtureBundle) -> Result<Vec<OpportunityDraft>, AdapterError> {
        if bundle.source_id != self.source_id {
            return Err(AdapterError::Message(format!(
                "bundle source_id={} does not match adapter source_id={}",
                bundle.source_id, self.source_id
            )));
        }
        Ok(bundle_to_drafts(bundle))
    }

    async fn fetch_detail(
        &self,
        _http: &HttpFetcher,
        _ctx: &AdapterContext,
        _targets: &[DetailTarget],
    ) -> Result<Vec<FetchedPage>, AdapterError> {
        Ok(Vec::new())
    }

    fn parse_detail(&self, bundle: &FixtureBundle) -> Result<Vec<OpportunityDraft>, AdapterError> {
        self.parse_listing(bundle)
    }
}

pub fn appen_crowdgen_adapter() -> impl SourceAdapter {
    FixtureFirstAdapter {
        source_id: "appen-crowdgen",
        crawlability: Crawlability::PublicHtml,
    }
}

pub fn clickworker_adapter() -> impl SourceAdapter {
    FixtureFirstAdapter {
        source_id: "clickworker",
        crawlability: Crawlability::PublicHtml,
    }
}

pub fn oneforma_jobs_adapter() -> impl SourceAdapter {
    FixtureFirstAdapter {
        source_id: "oneforma-jobs",
        crawlability: Crawlability::PublicHtml,
    }
}

pub fn telus_ai_community_adapter() -> impl SourceAdapter {
    FixtureFirstAdapter {
        source_id: "telus-ai-community",
        crawlability: Crawlability::PublicHtml,
    }
}

pub fn prolific_manual_adapter() -> impl SourceAdapter {
    FixtureFirstAdapter {
        source_id: "prolific",
        crawlability: Crawlability::ManualOnly,
    }
}

pub fn adapter_for_source(source_id: &str) -> Option<Box<dyn SourceAdapter>> {
    match source_id {
        "appen-crowdgen" => Some(Box::new(FixtureFirstAdapter {
            source_id: "appen-crowdgen",
            crawlability: Crawlability::PublicHtml,
        })),
        "clickworker" => Some(Box::new(FixtureFirstAdapter {
            source_id: "clickworker",
            crawlability: Crawlability::PublicHtml,
        })),
        "oneforma-jobs" => Some(Box::new(FixtureFirstAdapter {
            source_id: "oneforma-jobs",
            crawlability: Crawlability::PublicHtml,
        })),
        "telus-ai-community" => Some(Box::new(FixtureFirstAdapter {
            source_id: "telus-ai-community",
            crawlability: Crawlability::PublicHtml,
        })),
        "prolific" => Some(Box::new(FixtureFirstAdapter {
            source_id: "prolific",
            crawlability: Crawlability::ManualOnly,
        })),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    struct GoldenDraft {
        title: Option<String>,
        apply_url: Option<String>,
        pay_model: Option<String>,
        pay_rate_min: Option<f64>,
        pay_rate_max: Option<f64>,
        currency: Option<String>,
        crawlability: Crawlability,
    }

    fn workspace_root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .canonicalize()
            .expect("workspace root")
    }

    fn fixture_bundle_path(source_id: &str) -> PathBuf {
        workspace_root()
            .join("fixtures")
            .join(source_id)
            .join("sample")
            .join("bundle.json")
    }

    fn manual_fixture_bundle_path(source_id: &str) -> PathBuf {
        workspace_root()
            .join("manual")
            .join(source_id)
            .join("sample.json")
    }

    fn expected_snapshot_path(source_id: &str) -> PathBuf {
        workspace_root()
            .join("fixtures")
            .join(source_id)
            .join("sample")
            .join("snapshot.json")
    }

    fn drafts_to_golden(drafts: &[OpportunityDraft], crawlability: Crawlability) -> Vec<GoldenDraft> {
        drafts
            .iter()
            .map(|d| GoldenDraft {
                title: d.title.value.clone(),
                apply_url: d.apply_url.value.clone(),
                pay_model: d.pay_model.value.clone(),
                pay_rate_min: d.pay_rate_min.value,
                pay_rate_max: d.pay_rate_max.value,
                currency: d.currency.value.clone(),
                crawlability,
            })
            .collect()
    }

    fn read_snapshot(path: &Path) -> Vec<GoldenDraft> {
        let text = fs::read_to_string(path).expect("read snapshot");
        serde_json::from_str(&text).expect("parse snapshot")
    }

    fn assert_all_populated_fields_have_evidence(drafts: &[OpportunityDraft]) {
        for draft in drafts {
            if draft.title.value.is_some() {
                assert!(draft.title.evidence.is_some(), "title missing evidence");
            }
            if draft.description.value.is_some() {
                assert!(draft.description.evidence.is_some(), "description missing evidence");
            }
            if draft.pay_model.value.is_some() {
                assert!(draft.pay_model.evidence.is_some(), "pay_model missing evidence");
            }
            if draft.currency.value.is_some() {
                assert!(draft.currency.evidence.is_some(), "currency missing evidence");
            }
            if draft.apply_url.value.is_some() {
                assert!(draft.apply_url.evidence.is_some(), "apply_url missing evidence");
            }
        }
    }

    #[tokio::test]
    async fn golden_json_snapshot_test_appen_crowdgen() {
        let adapter = appen_crowdgen_adapter();
        let bundle = load_fixture_bundle(fixture_bundle_path("appen-crowdgen")).unwrap();
        let drafts = adapter.parse_listing(&bundle).unwrap();
        assert_all_populated_fields_have_evidence(&drafts);
        let actual = drafts_to_golden(&drafts, adapter.crawlability());
        let expected = read_snapshot(&expected_snapshot_path("appen-crowdgen"));
        assert_eq!(actual, expected);
    }

    #[tokio::test]
    async fn golden_json_snapshot_test_clickworker() {
        let adapter = clickworker_adapter();
        let bundle = load_fixture_bundle(fixture_bundle_path("clickworker")).unwrap();
        let drafts = adapter.parse_listing(&bundle).unwrap();
        assert_all_populated_fields_have_evidence(&drafts);
        let actual = drafts_to_golden(&drafts, adapter.crawlability());
        let expected = read_snapshot(&expected_snapshot_path("clickworker"));
        assert_eq!(actual, expected);
    }

    #[tokio::test]
    async fn golden_json_snapshot_test_oneforma_jobs() {
        let adapter = oneforma_jobs_adapter();
        let bundle = load_fixture_bundle(fixture_bundle_path("oneforma-jobs")).unwrap();
        let drafts = adapter.parse_listing(&bundle).unwrap();
        assert_all_populated_fields_have_evidence(&drafts);
        let actual = drafts_to_golden(&drafts, adapter.crawlability());
        let expected = read_snapshot(&expected_snapshot_path("oneforma-jobs"));
        assert_eq!(actual, expected);
    }

    #[tokio::test]
    async fn golden_json_snapshot_test_telus_ai_community() {
        let adapter = telus_ai_community_adapter();
        let bundle = load_fixture_bundle(fixture_bundle_path("telus-ai-community")).unwrap();
        let drafts = adapter.parse_listing(&bundle).unwrap();
        assert_all_populated_fields_have_evidence(&drafts);
        let actual = drafts_to_golden(&drafts, adapter.crawlability());
        let expected = read_snapshot(&expected_snapshot_path("telus-ai-community"));
        assert_eq!(actual, expected);
    }

    #[tokio::test]
    async fn golden_json_snapshot_test_prolific_manual_ingestion() {
        let adapter = prolific_manual_adapter();
        let bundle = load_manual_fixture_bundle(manual_fixture_bundle_path("prolific")).unwrap();
        let drafts = adapter.parse_listing(&bundle).unwrap();
        assert_all_populated_fields_have_evidence(&drafts);
        let actual = drafts_to_golden(&drafts, adapter.crawlability());
        let expected = read_snapshot(&expected_snapshot_path("prolific"));
        assert_eq!(actual, expected);
    }
}
