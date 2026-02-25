//! Source adapter contracts + fixture-first adapter implementations.

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use rhof_core::{EvidenceRef, Field, OpportunityDraft};
use rhof_storage::HttpFetcher;
use scraper::{Html, Selector};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::Value as JsonValue;
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
    let path = path.as_ref();
    let mut bundle: FixtureBundle = read_json_file(path)?;
    hydrate_inline_raw_artifact(path, &mut bundle)?;
    Ok(bundle)
}

pub fn load_manual_fixture_bundle(path: impl AsRef<Path>) -> Result<FixtureBundle> {
    let path = path.as_ref();
    let mut bundle: FixtureBundle = read_json_file(path)?;
    hydrate_inline_raw_artifact(path, &mut bundle)?;
    Ok(bundle)
}

fn read_json_file<T: DeserializeOwned>(path: impl AsRef<Path>) -> Result<T> {
    let path = path.as_ref();
    let data = fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    serde_json::from_str(&data).with_context(|| format!("parsing {}", path.display()))
}

fn hydrate_inline_raw_artifact(bundle_path: &Path, bundle: &mut FixtureBundle) -> Result<()> {
    if bundle.raw_artifact.inline_text.is_some() {
        return Ok(());
    }
    let Some(rel_path) = &bundle.raw_artifact.path else {
        return Ok(());
    };
    let raw_path = bundle_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join(rel_path);
    if !raw_path.exists() {
        return Ok(());
    }
    let raw = fs::read_to_string(&raw_path)
        .with_context(|| format!("reading fixture raw artifact {}", raw_path.display()))?;
    bundle.raw_artifact.inline_text = Some(raw);
    Ok(())
}

pub fn deterministic_raw_artifact_id_for_bundle(bundle: &FixtureBundle) -> Uuid {
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
                raw_artifact_id: deterministic_raw_artifact_id_for_bundle(bundle),
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
struct HtmlTitleLinkFixtureAdapter {
    source_id: &'static str,
    crawlability: Crawlability,
}

#[derive(Debug, Clone, Copy)]
struct JsonTitleApplyFixtureAdapter {
    source_id: &'static str,
    crawlability: Crawlability,
}

fn override_field_value<T>(field: &mut Field<T>, value: Option<T>) {
    if let Some(value) = value {
        field.value = Some(value);
    }
}

fn text_or_none(value: String) -> Option<String> {
    let trimmed = value.trim().to_string();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed)
    }
}

fn select_first_text(document: &Html, selector: &str) -> Result<Option<String>, AdapterError> {
    let sel = Selector::parse(selector).map_err(|e| AdapterError::Message(e.to_string()))?;
    Ok(document
        .select(&sel)
        .next()
        .and_then(|n| text_or_none(n.text().collect::<String>())))
}

fn select_all_texts(document: &Html, selector: &str) -> Result<Vec<String>, AdapterError> {
    let sel = Selector::parse(selector).map_err(|e| AdapterError::Message(e.to_string()))?;
    Ok(document
        .select(&sel)
        .filter_map(|n| text_or_none(n.text().collect::<String>()))
        .collect())
}

fn select_first_attr(document: &Html, selector: &str, attr: &str) -> Result<Option<String>, AdapterError> {
    let sel = Selector::parse(selector).map_err(|e| AdapterError::Message(e.to_string()))?;
    Ok(document
        .select(&sel)
        .next()
        .and_then(|n| n.value().attr(attr))
        .and_then(|s| text_or_none(s.to_string())))
}

fn extract_numbers(text: &str) -> Vec<f64> {
    let mut out = Vec::new();
    let mut current = String::new();
    let mut seen_dot = false;
    for ch in text.chars() {
        if ch.is_ascii_digit() {
            current.push(ch);
            continue;
        }
        if ch == '.' && !seen_dot && !current.is_empty() {
            current.push(ch);
            seen_dot = true;
            continue;
        }
        if !current.is_empty() {
            if let Ok(v) = current.parse::<f64>() {
                out.push(v);
            }
            current.clear();
            seen_dot = false;
        }
    }
    if !current.is_empty() {
        if let Ok(v) = current.parse::<f64>() {
            out.push(v);
        }
    }
    out
}

fn parse_pay_fields(pay_text: &str) -> (Option<String>, Option<f64>, Option<f64>, Option<String>) {
    let lower = pay_text.to_ascii_lowercase();
    let pay_model = if lower.contains("per task") || lower.contains("task-based") {
        Some("task-based".to_string())
    } else if lower.contains("fixed") {
        Some("fixed".to_string())
    } else if lower.contains("/hr") || lower.contains("hourly") {
        Some("hourly".to_string())
    } else {
        None
    };
    let nums = extract_numbers(pay_text);
    let pay_rate_min = nums.first().copied();
    let pay_rate_max = nums.get(1).copied().or(pay_rate_min);
    let currency = if lower.contains("usd") || pay_text.contains('$') {
        Some("USD".to_string())
    } else {
        None
    };
    (pay_model, pay_rate_min, pay_rate_max, currency)
}

fn normalize_duration(value: &str) -> Option<String> {
    let lower = value.to_ascii_lowercase();
    if lower.contains("one-off") || lower.contains("one off") {
        Some("one_off".to_string())
    } else if lower.contains("ongoing") {
        Some("ongoing".to_string())
    } else {
        None
    }
}

fn apply_extended_html_overrides(bundle: &FixtureBundle, drafts: &mut [OpportunityDraft]) -> Result<bool, AdapterError> {
    let Some(html_text) = bundle.raw_artifact.inline_text.as_deref() else {
        return Ok(false);
    };
    let Some(first) = drafts.get_mut(0) else {
        return Ok(false);
    };
    let document = Html::parse_document(html_text);

    let title = select_first_text(&document, "h1")?;
    let apply = select_first_attr(&document, "a[href]", "href")?;
    let description = select_first_text(&document, ".job-description")?
        .or(select_first_text(&document, ".summary")?);
    let pay_text = select_first_text(&document, ".pay")?;
    let hours_text = select_first_text(&document, ".hours")?;
    let verification = select_first_text(&document, ".verification")?
        .or(select_first_text(&document, ".requirements .verification")?);
    let geo = select_first_text(&document, ".geo")?;
    let duration = select_first_text(&document, ".duration")?;
    let mut payment_methods = select_all_texts(&document, ".payments li")?;
    if payment_methods.is_empty() {
        if let Some(payments_text) = select_first_text(&document, ".payments")? {
            payment_methods = payments_text
                .split(',')
                .filter_map(|s| text_or_none(s.to_string()))
                .collect();
        }
    }
    let requirements = select_all_texts(&document, ".requirements li")?;

    let mut applied = false;
    if let Some(t) = title {
        first.title.value = Some(t);
        applied = true;
    }
    if let Some(url) = apply {
        first.apply_url.value = Some(url);
        applied = true;
    }
    if let Some(desc) = description {
        first.description.value = Some(desc);
        applied = true;
    }
    if let Some(pay) = pay_text.as_deref() {
        let (pay_model, pay_min, pay_max, currency) = parse_pay_fields(pay);
        override_field_value(&mut first.pay_model, pay_model);
        override_field_value(&mut first.pay_rate_min, pay_min);
        override_field_value(&mut first.pay_rate_max, pay_max);
        override_field_value(&mut first.currency, currency);
        applied = true;
    }
    if let Some(hours) = hours_text.as_deref() {
        override_field_value(&mut first.min_hours_per_week, extract_numbers(hours).first().copied());
        applied = true;
    }
    if let Some(v) = verification {
        first.verification_requirements.value = Some(v);
        applied = true;
    }
    if let Some(g) = geo {
        first.geo_constraints.value = Some(g);
        applied = true;
    }
    if let Some(d) = duration.as_deref() {
        override_field_value(&mut first.one_off_vs_ongoing, normalize_duration(d));
        applied = true;
    }
    if !payment_methods.is_empty() {
        first.payment_methods.value = Some(payment_methods);
        applied = true;
    }
    if !requirements.is_empty() {
        first.requirements.value = Some(requirements);
        applied = true;
    }

    Ok(applied)
}

fn json_str<'a>(value: &'a JsonValue, path: &[&str]) -> Option<&'a str> {
    let mut cur = value;
    for segment in path {
        cur = cur.get(*segment)?;
    }
    cur.as_str()
}

fn json_f64(value: &JsonValue, path: &[&str]) -> Option<f64> {
    let mut cur = value;
    for segment in path {
        cur = cur.get(*segment)?;
    }
    cur.as_f64()
}

fn json_string_vec(value: &JsonValue, path: &[&str]) -> Option<Vec<String>> {
    let mut cur = value;
    for segment in path {
        cur = cur.get(*segment)?;
    }
    let arr = cur.as_array()?;
    let vals = arr
        .iter()
        .filter_map(|v| v.as_str().map(ToString::to_string))
        .collect::<Vec<_>>();
    if vals.is_empty() {
        None
    } else {
        Some(vals)
    }
}

fn apply_extended_json_overrides(bundle: &FixtureBundle, drafts: &mut [OpportunityDraft]) -> Result<bool, AdapterError> {
    let Some(text) = bundle.raw_artifact.inline_text.as_deref() else {
        return Ok(false);
    };
    let Some(first) = drafts.get_mut(0) else {
        return Ok(false);
    };
    let value: JsonValue = serde_json::from_str(text)
        .map_err(|e| AdapterError::Message(format!("invalid raw JSON fixture: {e}")))?;

    let title = json_str(&value, &["title"]).map(ToString::to_string);
    let apply = json_str(&value, &["apply_url"]).map(ToString::to_string);
    let description = json_str(&value, &["description"]).map(ToString::to_string);
    let pay_model = json_str(&value, &["reward", "model"])
        .or_else(|| json_str(&value, &["pay_model"]))
        .map(|s| {
            if s.eq_ignore_ascii_case("one-off") {
                "one_off".to_string()
            } else {
                s.to_string()
            }
        });
    let pay_rate_min = json_f64(&value, &["reward", "min"]).or_else(|| json_f64(&value, &["reward_min"]));
    let pay_rate_max = json_f64(&value, &["reward", "max"])
        .or_else(|| json_f64(&value, &["reward_max"]))
        .or(pay_rate_min);
    let currency = json_str(&value, &["reward", "currency"])
        .or_else(|| json_str(&value, &["currency"]))
        .map(ToString::to_string);
    let min_hours_per_week = json_f64(&value, &["hours_per_week_min"]).or_else(|| json_f64(&value, &["hours"]));
    let verification = json_str(&value, &["verification_requirements"])
        .or_else(|| json_str(&value, &["requirements"]))
        .map(ToString::to_string);
    let geo = json_str(&value, &["audience", "country"])
        .or_else(|| json_str(&value, &["geo"]))
        .map(ToString::to_string);
    let duration = json_str(&value, &["type"]).and_then(normalize_duration);
    let payment_methods = json_string_vec(&value, &["payment_methods"]).or_else(|| {
        json_str(&value, &["payment"]).map(|s| vec![s.to_string()])
    });
    let requirements = json_string_vec(&value, &["eligibility"])
        .or_else(|| json_string_vec(&value, &["requirements_list"]))
        .or_else(|| json_str(&value, &["eligibility"]).map(|s| vec![s.to_string()]));

    let mut applied = false;
    if let Some(t) = title {
        first.title.value = Some(t);
        applied = true;
    }
    if let Some(url) = apply {
        first.apply_url.value = Some(url);
        applied = true;
    }
    if let Some(desc) = description {
        first.description.value = Some(desc);
        applied = true;
    }
    override_field_value(&mut first.pay_model, pay_model);
    override_field_value(&mut first.pay_rate_min, pay_rate_min);
    override_field_value(&mut first.pay_rate_max, pay_rate_max);
    override_field_value(&mut first.currency, currency);
    override_field_value(&mut first.min_hours_per_week, min_hours_per_week);
    if let Some(v) = verification {
        first.verification_requirements.value = Some(v);
        applied = true;
    }
    if let Some(g) = geo {
        first.geo_constraints.value = Some(g);
        applied = true;
    }
    override_field_value(&mut first.one_off_vs_ongoing, duration);
    if let Some(v) = payment_methods {
        first.payment_methods.value = Some(v);
        applied = true;
    }
    if let Some(v) = requirements {
        first.requirements.value = Some(v);
        applied = true;
    }
    if first.pay_model.value.is_some()
        || first.pay_rate_min.value.is_some()
        || first.pay_rate_max.value.is_some()
        || first.currency.value.is_some()
        || first.min_hours_per_week.value.is_some()
    {
        applied = true;
    }

    Ok(applied)
}

fn parse_title_apply_from_raw_html(bundle: &FixtureBundle) -> Result<Option<Vec<OpportunityDraft>>, AdapterError> {
    let mut drafts = bundle_to_drafts(bundle);
    if !apply_extended_html_overrides(bundle, &mut drafts)? {
        return Ok(None);
    }
    Ok(Some(drafts))
}

fn parse_title_apply_from_raw_json(bundle: &FixtureBundle) -> Result<Option<Vec<OpportunityDraft>>, AdapterError> {
    let mut drafts = bundle_to_drafts(bundle);
    if !apply_extended_json_overrides(bundle, &mut drafts)? {
        return Ok(None);
    }
    Ok(Some(drafts))
}

#[async_trait]
impl SourceAdapter for HtmlTitleLinkFixtureAdapter {
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
        if let Some(drafts) = parse_title_apply_from_raw_html(bundle)? {
            return Ok(drafts);
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

#[async_trait]
impl SourceAdapter for JsonTitleApplyFixtureAdapter {
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
        if let Some(drafts) = parse_title_apply_from_raw_json(bundle)? {
            return Ok(drafts);
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
    HtmlTitleLinkFixtureAdapter {
        source_id: "appen-crowdgen",
        crawlability: Crawlability::PublicHtml,
    }
}

pub fn clickworker_adapter() -> impl SourceAdapter {
    HtmlTitleLinkFixtureAdapter {
        source_id: "clickworker",
        crawlability: Crawlability::PublicHtml,
    }
}

pub fn oneforma_jobs_adapter() -> impl SourceAdapter {
    HtmlTitleLinkFixtureAdapter {
        source_id: "oneforma-jobs",
        crawlability: Crawlability::PublicHtml,
    }
}

pub fn telus_ai_community_adapter() -> impl SourceAdapter {
    HtmlTitleLinkFixtureAdapter {
        source_id: "telus-ai-community",
        crawlability: Crawlability::PublicHtml,
    }
}

pub fn prolific_manual_adapter() -> impl SourceAdapter {
    JsonTitleApplyFixtureAdapter {
        source_id: "prolific",
        crawlability: Crawlability::ManualOnly,
    }
}

pub fn adapter_for_source(source_id: &str) -> Option<Box<dyn SourceAdapter>> {
    match source_id {
        "appen-crowdgen" => Some(Box::new(HtmlTitleLinkFixtureAdapter {
            source_id: "appen-crowdgen",
            crawlability: Crawlability::PublicHtml,
        })),
        "clickworker" => Some(Box::new(HtmlTitleLinkFixtureAdapter {
            source_id: "clickworker",
            crawlability: Crawlability::PublicHtml,
        })),
        "oneforma-jobs" => Some(Box::new(HtmlTitleLinkFixtureAdapter {
            source_id: "oneforma-jobs",
            crawlability: Crawlability::PublicHtml,
        })),
        "telus-ai-community" => Some(Box::new(HtmlTitleLinkFixtureAdapter {
            source_id: "telus-ai-community",
            crawlability: Crawlability::PublicHtml,
        })),
        "prolific" => Some(Box::new(JsonTitleApplyFixtureAdapter {
            source_id: "prolific",
            crawlability: Crawlability::ManualOnly,
        })),
        _ => None,
    }
}

pub fn generate_adapter_scaffold(
    workspace_root: impl AsRef<Path>,
    source_id: &str,
) -> Result<Vec<PathBuf>> {
    let workspace_root = workspace_root.as_ref();
    let slug = normalize_source_id(source_id);
    let template_dir = workspace_root.join("templates/adapter");
    let fixture_dir = workspace_root.join("fixtures").join(&slug).join("sample");
    let raw_dir = fixture_dir.join("raw");
    let tests_dir = workspace_root.join("crates/rhof-adapters/tests");
    let generated_src_dir = workspace_root.join("crates/rhof-adapters/src/generated");
    let docs_sources = workspace_root.join("docs/SOURCES.md");

    std::fs::create_dir_all(&raw_dir).with_context(|| format!("creating {}", raw_dir.display()))?;
    std::fs::create_dir_all(&tests_dir).with_context(|| format!("creating {}", tests_dir.display()))?;
    std::fs::create_dir_all(&generated_src_dir)
        .with_context(|| format!("creating {}", generated_src_dir.display()))?;

    let adapter_rs = generated_src_dir.join(format!("{slug}.rs"));
    let test_rs = tests_dir.join(format!("{slug}_snapshot.rs"));
    let bundle_json = fixture_dir.join("bundle.json");
    let raw_listing = raw_dir.join("listing.html");
    let snapshot_json = fixture_dir.join("snapshot.json");

    let mut created = Vec::new();
    write_from_template_if_missing(
        &adapter_rs,
        &template_dir.join("adapter.rs.tmpl"),
        &slug,
        source_id,
    )?;
    created.push(adapter_rs.clone());

    write_from_template_if_missing(
        &test_rs,
        &template_dir.join("adapter_test.rs.tmpl"),
        &slug,
        source_id,
    )?;
    created.push(test_rs.clone());

    write_from_template_if_missing(
        &bundle_json,
        &template_dir.join("bundle.json.tmpl"),
        &slug,
        source_id,
    )?;
    created.push(bundle_json.clone());

    write_from_template_if_missing(
        &raw_listing,
        &template_dir.join("raw_listing.html.tmpl"),
        &slug,
        source_id,
    )?;
    created.push(raw_listing.clone());

    write_from_template_if_missing(
        &snapshot_json,
        &template_dir.join("snapshot.json.tmpl"),
        &slug,
        source_id,
    )?;
    created.push(snapshot_json.clone());

    append_docs_source_stub_if_missing(&docs_sources, &slug, source_id)?;
    created.push(docs_sources);

    Ok(created)
}

fn normalize_source_id(input: &str) -> String {
    input
        .trim()
        .to_ascii_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect::<String>()
        .split('-')
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

fn write_from_template_if_missing(
    dest: &Path,
    template_path: &Path,
    slug: &str,
    display_name_input: &str,
) -> Result<()> {
    if dest.exists() {
        return Ok(());
    }
    let template = fs::read_to_string(template_path)
        .with_context(|| format!("reading template {}", template_path.display()))?;
    let display_name = display_name_input.replace('-', " ");
    let rendered = template
        .replace("{{source_id}}", slug)
        .replace("{{display_name}}", &display_name)
        .replace("{{source_id_pascal}}", &to_pascal_case(slug));
    fs::write(dest, rendered).with_context(|| format!("writing {}", dest.display()))?;
    Ok(())
}

fn to_pascal_case(slug: &str) -> String {
    slug.split('-')
        .filter(|p| !p.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(first) => {
                    let mut s = String::new();
                    s.extend(first.to_uppercase());
                    s.push_str(chars.as_str());
                    s
                }
                None => String::new(),
            }
        })
        .collect::<String>()
}

fn append_docs_source_stub_if_missing(path: &Path, slug: &str, display_name_input: &str) -> Result<()> {
    let mut current = if path.exists() {
        fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?
    } else {
        String::new()
    };
    let marker = format!("## Source: {slug}");
    if current.contains(&marker) {
        return Ok(());
    }
    if !current.ends_with('\n') {
        current.push('\n');
    }
    current.push_str(&format!(
        "\n## Source: {}\n\n- Display name: {}\n- Crawlability: TODO\n- Status: scaffold generated by `rhof-cli new-adapter {}`\n- Fixtures: `fixtures/{}/sample/`\n- Tests: `crates/rhof-adapters/tests/{}_snapshot.rs`\n",
        slug,
        display_name_input,
        slug,
        slug,
        slug
    ));
    fs::write(path, current).with_context(|| format!("writing {}", path.display()))?;
    Ok(())
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

    #[test]
    fn raw_html_parser_overrides_description_and_requirements_values() {
        let adapter = clickworker_adapter();
        let mut bundle = load_fixture_bundle(fixture_bundle_path("clickworker")).unwrap();
        let rec = bundle.parsed_records.get_mut(0).unwrap();
        rec.description.value = Some("WRONG DESCRIPTION".to_string());
        rec.pay_model.value = Some("fixed".to_string());
        rec.pay_rate_min.value = Some(99.0);
        rec.pay_rate_max.value = Some(100.0);
        rec.currency.value = Some("EUR".to_string());
        rec.min_hours_per_week.value = Some(99.0);
        rec.geo_constraints.value = Some("Mars".to_string());
        rec.payment_methods.value = Some(vec!["Wire".to_string()]);
        rec.requirements.value = Some(vec!["Wrong".to_string()]);

        let drafts = adapter.parse_listing(&bundle).unwrap();
        let first = drafts.first().unwrap();
        assert_eq!(first.description.value.as_deref(), Some("Contribute labeled data for AI systems."));
        assert_eq!(first.pay_model.value.as_deref(), Some("hourly"));
        assert_eq!(first.pay_rate_min.value, Some(12.0));
        assert_eq!(first.pay_rate_max.value, Some(16.0));
        assert_eq!(first.currency.value.as_deref(), Some("USD"));
        assert_eq!(first.min_hours_per_week.value, Some(5.0));
        assert_eq!(
            first.geo_constraints.value.as_deref(),
            Some("Global (country-dependent tasks)")
        );
        assert_eq!(first.payment_methods.value.clone().unwrap(), vec!["PayPal".to_string()]);
        assert_eq!(
            first.requirements.value.clone().unwrap(),
            vec!["Smartphone".to_string(), "English".to_string()]
        );
    }

    #[test]
    fn raw_json_parser_overrides_manual_prolific_values() {
        let adapter = prolific_manual_adapter();
        let mut bundle = load_manual_fixture_bundle(manual_fixture_bundle_path("prolific")).unwrap();
        let rec = bundle.parsed_records.get_mut(0).unwrap();
        rec.description.value = Some("WRONG".to_string());
        rec.pay_model.value = Some("hourly".to_string());
        rec.pay_rate_min.value = Some(1.0);
        rec.pay_rate_max.value = Some(2.0);
        rec.currency.value = Some("GBP".to_string());
        rec.verification_requirements.value = Some("Wrong verification".to_string());
        rec.geo_constraints.value = Some("CA".to_string());
        rec.one_off_vs_ongoing.value = Some("ongoing".to_string());
        rec.payment_methods.value = Some(vec!["WrongPay".to_string()]);
        rec.requirements.value = Some(vec!["WrongReq".to_string()]);

        let drafts = adapter.parse_listing(&bundle).unwrap();
        let first = drafts.first().unwrap();
        assert_eq!(first.description.value.as_deref(), Some("Manual ingestion of a gated study listing."));
        assert_eq!(first.pay_model.value.as_deref(), Some("fixed"));
        assert_eq!(first.pay_rate_min.value, Some(6.0));
        assert_eq!(first.pay_rate_max.value, Some(6.0));
        assert_eq!(first.currency.value.as_deref(), Some("USD"));
        assert_eq!(first.verification_requirements.value.as_deref(), Some("Prolific account"));
        assert_eq!(first.geo_constraints.value.as_deref(), Some("US"));
        assert_eq!(first.one_off_vs_ongoing.value.as_deref(), Some("one_off"));
        assert_eq!(
            first.payment_methods.value.clone().unwrap(),
            vec!["Prolific payout".to_string()]
        );
        assert_eq!(first.requirements.value.clone().unwrap(), vec!["Age 18+".to_string()]);
    }
}
