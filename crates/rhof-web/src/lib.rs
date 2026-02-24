//! Axum + Askama web UI for RHOF (PROMPT_08).

use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use askama::Template;
use axum::{
    extract::{Path as AxumPath, Query, State},
    http::{header, StatusCode},
    response::{Html, IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use rhof_sync::StagedOpportunity;
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Row};
use tokio::net::TcpListener;

pub const CRATE_NAME: &str = "rhof-web";

#[derive(Clone)]
pub struct AppState {
    pub workspace_root: PathBuf,
}

impl AppState {
    pub fn new(workspace_root: impl Into<PathBuf>) -> Self {
        Self {
            workspace_root: workspace_root.into(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
struct SourcesYaml {
    sources: Vec<SourceRow>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SourceRow {
    pub source_id: String,
    pub display_name: String,
    pub enabled: bool,
    pub crawlability: String,
    pub mode: String,
    #[serde(default)]
    pub listing_urls: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WebOpportunity {
    pub id: String,
    pub source_id: String,
    pub title: String,
    pub pay_model: Option<String>,
    pub pay_rate_min: Option<f64>,
    pub pay_rate_max: Option<f64>,
    pub currency: Option<String>,
    pub apply_url: Option<String>,
    pub review_required: bool,
    pub dedup_confidence: Option<f64>,
    pub tags: Vec<String>,
    pub risk_flags: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct OpportunitiesDelta {
    opportunities: Vec<DeltaOpportunity>,
}

#[derive(Debug, Clone, Deserialize)]
struct DeltaOpportunity {
    source_id: String,
    canonical_key: String,
    review_required: bool,
    dedup_confidence: Option<f64>,
    tags: Vec<String>,
    risk_flags: Vec<String>,
    draft: DeltaDraft,
}

#[derive(Debug, Clone, Deserialize)]
struct DeltaDraft {
    title: DeltaField<String>,
    pay_model: DeltaField<String>,
    pay_rate_min: DeltaField<f64>,
    pay_rate_max: DeltaField<f64>,
    currency: DeltaField<String>,
    apply_url: DeltaField<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct DeltaField<T> {
    value: Option<T>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RunReportRow {
    pub run_id: String,
    pub opportunities: usize,
    pub has_chart: bool,
    pub has_parquet_manifest: bool,
}

#[derive(Debug, Clone)]
struct DashboardData {
    sources: Vec<SourceRow>,
    opportunities: Vec<WebOpportunity>,
    runs: Vec<RunReportRow>,
}

#[derive(Debug, Deserialize, Default)]
struct OpportunitiesQuery {
    source: Option<String>,
    page: Option<usize>,
    per_page: Option<usize>,
}

#[derive(Template)]
#[template(path = "index.html")]
struct IndexTemplate {
    total_sources: usize,
    total_opportunities: usize,
    total_review_items: usize,
    latest_run_id: String,
}

#[derive(Template)]
#[template(path = "opportunities.html")]
struct OpportunitiesPageTemplate {
    selected_source: String,
    page: usize,
}

#[derive(Template)]
#[template(path = "opportunities_table_partial.html")]
struct OpportunitiesTablePartialTemplate {
    opportunities: Vec<WebOpportunity>,
    page: usize,
    total_pages: usize,
}

#[derive(Template)]
#[template(path = "opportunities_facets_partial.html")]
struct OpportunitiesFacetsPartialTemplate {
    source_counts: Vec<FacetCountRow>,
    all_selected: bool,
}

#[derive(Debug, Clone)]
struct FacetCountRow {
    source_id: String,
    count: usize,
    selected: bool,
}

#[derive(Template)]
#[template(path = "opportunity_detail.html")]
struct OpportunityDetailTemplate {
    opportunity: WebOpportunity,
    tags_text: String,
    risk_flags_text: String,
}

#[derive(Template)]
#[template(path = "sources.html")]
struct SourcesTemplate {
    sources: Vec<SourceRow>,
}

#[derive(Template)]
#[template(path = "review.html")]
struct ReviewTemplate {
    review_items: Vec<WebOpportunity>,
}

#[derive(Template)]
#[template(path = "reports.html")]
struct ReportsTemplate {
    runs: Vec<RunReportRow>,
}

#[derive(Template)]
#[template(path = "review_resolve_partial.html")]
struct ReviewResolvePartialTemplate {
    review_id: String,
}

pub fn app(state: AppState) -> Router {
    Router::new()
        .route("/", get(index_handler))
        .route("/opportunities", get(opportunities_page_handler))
        .route("/opportunities/table", get(opportunities_table_handler))
        .route("/opportunities/facets", get(opportunities_facets_handler))
        .route("/opportunities/{id}", get(opportunity_detail_handler))
        .route("/sources", get(sources_handler))
        .route("/review", get(review_handler))
        .route("/review/{id}/resolve", post(review_resolve_handler))
        .route("/reports", get(reports_handler))
        .route("/reports/chart", get(reports_chart_handler))
        .route("/assets/static/app.css", get(app_css_handler))
        .with_state(Arc::new(state))
}

pub async fn serve_from_env() -> anyhow::Result<()> {
    let port: u16 = std::env::var("RHOF_WEB_PORT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(8000);
    let state = AppState::new(".");
    let listener = TcpListener::bind(("0.0.0.0", port)).await?;
    axum::serve(listener, app(state)).await?;
    Ok(())
}

async fn index_handler(State(state): State<Arc<AppState>>) -> Response {
    match load_dashboard_data(&state.workspace_root).await {
        Ok(data) => {
            let tpl = IndexTemplate {
                total_sources: data.sources.len(),
                total_opportunities: data.opportunities.len(),
                total_review_items: data.opportunities.iter().filter(|o| o.review_required).count(),
                latest_run_id: data.runs.first().map(|r| r.run_id.clone()).unwrap_or_else(|| "n/a".into()),
            };
            render_html(tpl)
        }
        Err(err) => server_error(err),
    }
}

async fn opportunities_page_handler(
    State(state): State<Arc<AppState>>,
    Query(query): Query<OpportunitiesQuery>,
) -> Response {
    match load_dashboard_data(&state.workspace_root).await {
        Ok(data) => {
            let (_page_rows, _source_counts, selected_source, page, _total_pages) =
                filtered_paginated_opportunities(&data.opportunities, &query);
            render_html(OpportunitiesPageTemplate {
                selected_source,
                page,
            })
        }
        Err(err) => server_error(err),
    }
}

async fn opportunities_table_handler(
    State(state): State<Arc<AppState>>,
    Query(query): Query<OpportunitiesQuery>,
) -> Response {
    match load_dashboard_data(&state.workspace_root).await {
        Ok(data) => {
            let (page_rows, _source_counts, _selected_source, page, total_pages) =
                filtered_paginated_opportunities(&data.opportunities, &query);
            let mut resp = render_html(OpportunitiesTablePartialTemplate {
                opportunities: page_rows,
                page,
                total_pages,
            });
            resp.headers_mut().insert(
                header::HeaderName::from_static("hx-trigger"),
                header::HeaderValue::from_static("opportunitiesTableLoaded"),
            );
            resp
        }
        Err(err) => server_error(err),
    }
}

async fn opportunities_facets_handler(
    State(state): State<Arc<AppState>>,
    Query(query): Query<OpportunitiesQuery>,
) -> Response {
    match load_dashboard_data(&state.workspace_root).await {
        Ok(data) => {
            let (_rows, source_counts, selected_source, _page, _total_pages) =
                filtered_paginated_opportunities(&data.opportunities, &query);
            let all_selected = selected_source.is_empty();
            render_html(OpportunitiesFacetsPartialTemplate {
                source_counts,
                all_selected,
            })
        }
        Err(err) => server_error(err),
    }
}

async fn opportunity_detail_handler(
    State(state): State<Arc<AppState>>,
    AxumPath(id): AxumPath<String>,
) -> Response {
    match load_dashboard_data(&state.workspace_root).await {
        Ok(data) => {
            if let Some(opportunity) = data.opportunities.into_iter().find(|o| o.id == id) {
                let tags_text = if opportunity.tags.is_empty() {
                    "none".to_string()
                } else {
                    opportunity.tags.join(", ")
                };
                let risk_flags_text = if opportunity.risk_flags.is_empty() {
                    "none".to_string()
                } else {
                    opportunity.risk_flags.join(", ")
                };
                render_html(OpportunityDetailTemplate {
                    opportunity,
                    tags_text,
                    risk_flags_text,
                })
            } else {
                (StatusCode::NOT_FOUND, Html("Opportunity not found".to_string())).into_response()
            }
        }
        Err(err) => server_error(err),
    }
}

async fn sources_handler(State(state): State<Arc<AppState>>) -> Response {
    match load_dashboard_data(&state.workspace_root).await {
        Ok(data) => render_html(SourcesTemplate { sources: data.sources }),
        Err(err) => server_error(err),
    }
}

async fn review_handler(State(state): State<Arc<AppState>>) -> Response {
    match load_dashboard_data(&state.workspace_root).await {
        Ok(data) => {
            let review_items = if let Some(pool) = connect_db_from_env().await {
                match load_open_review_opportunity_ids_from_db(&pool).await {
                    Ok(open_ids) => data
                        .opportunities
                        .into_iter()
                        .filter(|o| open_ids.contains(&o.id))
                        .collect::<Vec<_>>(),
                    Err(_) => data
                        .opportunities
                        .into_iter()
                        .filter(|o| o.review_required)
                        .collect::<Vec<_>>(),
                }
            } else {
                data
                    .opportunities
                    .into_iter()
                    .filter(|o| o.review_required)
                    .collect::<Vec<_>>()
            };
            render_html(ReviewTemplate { review_items })
        }
        Err(err) => server_error(err),
    }
}

async fn review_resolve_handler(
    State(_state): State<Arc<AppState>>,
    AxumPath(id): AxumPath<String>,
) -> Response {
    if let Some(pool) = connect_db_from_env().await {
        if let Err(err) = sqlx::query(
            r#"
            UPDATE review_items
               SET status = 'resolved',
                   resolved_at = NOW()
             WHERE opportunity_id::text = $1
               AND status = 'open'
            "#,
        )
        .bind(&id)
        .execute(&pool)
        .await
        {
            return server_error(anyhow::anyhow!(format!("failed to resolve review item: {err}")));
        }
    }
    render_html(ReviewResolvePartialTemplate { review_id: id })
}

async fn reports_handler(State(state): State<Arc<AppState>>) -> Response {
    match load_dashboard_data(&state.workspace_root).await {
        Ok(data) => render_html(ReportsTemplate { runs: data.runs }),
        Err(err) => server_error(err),
    }
}

async fn reports_chart_handler(State(state): State<Arc<AppState>>) -> Response {
    match load_dashboard_data(&state.workspace_root).await {
        Ok(data) => {
            let x = data.runs.iter().map(|r| r.run_id.clone()).collect::<Vec<_>>();
            let y = data.runs.iter().map(|r| r.opportunities as i64).collect::<Vec<_>>();
            Json(serde_json::json!({
                "data": [{
                    "type": "bar",
                    "x": x,
                    "y": y,
                    "marker": {"color": "#0ea5e9"}
                }],
                "layout": {
                    "title": "Opportunities Per Run",
                    "paper_bgcolor": "#ffffff",
                    "plot_bgcolor": "#f8fafc"
                }
            }))
            .into_response()
        }
        Err(err) => server_error(err),
    }
}

async fn app_css_handler(State(state): State<Arc<AppState>>) -> Response {
    let css_path = state.workspace_root.join("assets/static/app.css");
    match tokio::fs::read_to_string(&css_path).await {
        Ok(css) => (
            [(header::CONTENT_TYPE, "text/css; charset=utf-8")],
            css,
        )
            .into_response(),
        Err(_) => (StatusCode::NOT_FOUND, Html("/* missing app.css */".to_string())).into_response(),
    }
}

fn render_html<T: Template>(tpl: T) -> Response {
    match tpl.render() {
        Ok(html) => Html(html).into_response(),
        Err(err) => server_error(anyhow::anyhow!(err.to_string())),
    }
}

fn server_error(err: anyhow::Error) -> Response {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Html(format!("Server error: {}", err)),
    )
        .into_response()
}

async fn load_dashboard_data(workspace_root: &Path) -> anyhow::Result<DashboardData> {
    let runs = load_runs(workspace_root, 20)?;
    let db_pool = connect_db_from_env().await;
    let sources = if let Some(pool) = &db_pool {
        match load_sources_from_db(pool).await {
            Ok(rows) if !rows.is_empty() => rows,
            _ => load_sources_from_yaml(workspace_root)?,
        }
    } else {
        load_sources_from_yaml(workspace_root)?
    };
    let opportunities = if let Some(pool) = &db_pool {
        match load_latest_opportunities_from_db(pool).await {
            Ok(rows) if !rows.is_empty() => rows,
            _ => load_latest_opportunities_from_reports(workspace_root)?,
        }
    } else {
        load_latest_opportunities_from_reports(workspace_root)?
    };
    Ok(DashboardData {
        sources,
        opportunities,
        runs,
    })
}

async fn connect_db_from_env() -> Option<PgPool> {
    let database_url = std::env::var("DATABASE_URL").ok()?;
    PgPool::connect(&database_url).await.ok()
}

fn load_sources_from_yaml(workspace_root: &Path) -> anyhow::Result<Vec<SourceRow>> {
    let path = workspace_root.join("sources.yaml");
    let yaml = std::fs::read_to_string(&path)?;
    let parsed: SourcesYaml = serde_yaml::from_str(&yaml)?;
    Ok(parsed.sources)
}

async fn load_sources_from_db(pool: &PgPool) -> anyhow::Result<Vec<SourceRow>> {
    let rows = sqlx::query(
        r#"
        SELECT source_id, display_name, enabled, crawlability, config_json
          FROM sources
         ORDER BY source_id
        "#,
    )
    .fetch_all(pool)
    .await?;

    let mut out = Vec::with_capacity(rows.len());
    for row in rows {
        let config_json: serde_json::Value = row.try_get("config_json")?;
        let listing_urls = config_json
            .get("listing_urls")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(ToString::to_string))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let mode = config_json
            .get("mode")
            .and_then(|v| v.as_str())
            .unwrap_or("crawler")
            .to_string();
        out.push(SourceRow {
            source_id: row.try_get("source_id")?,
            display_name: row.try_get("display_name")?,
            enabled: row.try_get("enabled")?,
            crawlability: row.try_get("crawlability")?,
            mode,
            listing_urls,
        });
    }
    Ok(out)
}

fn load_runs(workspace_root: &Path, limit: usize) -> anyhow::Result<Vec<RunReportRow>> {
    let reports_root = workspace_root.join("reports");
    if !reports_root.exists() {
        return Ok(vec![]);
    }
    let mut entries = std::fs::read_dir(&reports_root)?
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().map(|ft| ft.is_dir()).unwrap_or(false))
        .collect::<Vec<_>>();
    entries.sort_by_key(|e| e.metadata().and_then(|m| m.modified()).ok());
    entries.reverse();

    let mut runs = Vec::new();
    for e in entries.into_iter().take(limit) {
        let run_id = e.file_name().to_string_lossy().to_string();
        let delta_path = e.path().join("opportunities_delta.json");
        let count = if delta_path.exists() {
            let v: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&delta_path)?)?;
            v.get("opportunities")
                .and_then(|o| o.as_array())
                .map(|a| a.len())
                .unwrap_or(0)
        } else {
            0
        };
        runs.push(RunReportRow {
            run_id,
            opportunities: count,
            has_chart: true,
            has_parquet_manifest: e.path().join("snapshots/manifest.json").exists(),
        });
    }
    Ok(runs)
}

fn load_latest_opportunities_from_reports(workspace_root: &Path) -> anyhow::Result<Vec<WebOpportunity>> {
    let latest_run = load_runs(workspace_root, 1)?.into_iter().next();
    let Some(run) = latest_run else { return Ok(vec![]); };
    let delta_path = workspace_root
        .join("reports")
        .join(&run.run_id)
        .join("opportunities_delta.json");
    let delta: OpportunitiesDelta = serde_json::from_str(&std::fs::read_to_string(&delta_path)?)?;
    Ok(delta
        .opportunities
        .into_iter()
        .enumerate()
        .map(|(idx, o)| WebOpportunity {
            id: idx.to_string(),
            source_id: o.source_id,
            title: o.draft.title.value.unwrap_or_else(|| o.canonical_key.clone()),
            pay_model: o.draft.pay_model.value,
            pay_rate_min: o.draft.pay_rate_min.value,
            pay_rate_max: o.draft.pay_rate_max.value,
            currency: o.draft.currency.value,
            apply_url: o.draft.apply_url.value,
            review_required: o.review_required,
            dedup_confidence: o.dedup_confidence,
            tags: o.tags,
            risk_flags: o.risk_flags,
        })
        .collect())
}

async fn load_latest_opportunities_from_db(pool: &PgPool) -> anyhow::Result<Vec<WebOpportunity>> {
    let rows = sqlx::query(
        r#"
        SELECT o.id::text AS id,
               COALESCE(s.source_id, '') AS source_id,
               o.canonical_key,
               ov.data_json
          FROM opportunities o
          LEFT JOIN sources s ON s.id = o.source_id
          LEFT JOIN opportunity_versions ov ON ov.id = o.current_version_id
         ORDER BY o.updated_at DESC, o.created_at DESC
         LIMIT 500
        "#,
    )
    .fetch_all(pool)
    .await?;

    let mut out = Vec::with_capacity(rows.len());
    for row in rows {
        let id: String = row.try_get("id")?;
        let source_id: String = row.try_get("source_id")?;
        let canonical_key: String = row.try_get("canonical_key")?;
        let data_json: Option<serde_json::Value> = row.try_get("data_json")?;

        if let Some(value) = data_json {
            if let Ok(staged) = serde_json::from_value::<StagedOpportunity>(value) {
                out.push(WebOpportunity {
                    id,
                    source_id: if source_id.is_empty() { staged.source_id.clone() } else { source_id },
                    title: staged
                        .draft
                        .title
                        .value
                        .clone()
                        .unwrap_or_else(|| staged.canonical_key.clone()),
                    pay_model: staged.draft.pay_model.value.clone(),
                    pay_rate_min: staged.draft.pay_rate_min.value,
                    pay_rate_max: staged.draft.pay_rate_max.value,
                    currency: staged.draft.currency.value.clone(),
                    apply_url: staged.draft.apply_url.value.clone(),
                    review_required: staged.review_required,
                    dedup_confidence: staged.dedup_confidence,
                    tags: staged.tags.clone(),
                    risk_flags: staged.risk_flags.clone(),
                });
                continue;
            }
        }

        out.push(WebOpportunity {
            id,
            source_id,
            title: canonical_key.clone(),
            pay_model: None,
            pay_rate_min: None,
            pay_rate_max: None,
            currency: None,
            apply_url: None,
            review_required: false,
            dedup_confidence: None,
            tags: vec![],
            risk_flags: vec![],
        });
    }
    Ok(out)
}

async fn load_open_review_opportunity_ids_from_db(pool: &PgPool) -> anyhow::Result<HashSet<String>> {
    let rows = sqlx::query(
        r#"
        SELECT DISTINCT opportunity_id::text AS opportunity_id
          FROM review_items
         WHERE status = 'open'
           AND opportunity_id IS NOT NULL
        "#,
    )
    .fetch_all(pool)
    .await?;
    let mut out = HashSet::with_capacity(rows.len());
    for row in rows {
        let id: String = row.try_get("opportunity_id")?;
        out.insert(id);
    }
    Ok(out)
}

fn filtered_paginated_opportunities(
    all: &[WebOpportunity],
    query: &OpportunitiesQuery,
) -> (Vec<WebOpportunity>, Vec<FacetCountRow>, String, usize, usize) {
    let mut counts = BTreeMap::<String, usize>::new();
    for o in all {
        *counts.entry(o.source_id.clone()).or_default() += 1;
    }
    let selected_source = query.source.clone().unwrap_or_default();
    let source_counts = counts
        .into_iter()
        .map(|(source_id, count)| FacetCountRow {
            selected: !selected_source.is_empty() && selected_source == source_id,
            source_id,
            count,
        })
        .collect::<Vec<_>>();

    let filtered = all
        .iter()
        .filter(|o| selected_source.is_empty() || o.source_id == selected_source)
        .cloned()
        .collect::<Vec<_>>();

    let per_page = query.per_page.unwrap_or(20).max(1);
    let total_pages = filtered.len().max(1).div_ceil(per_page);
    let page = query.page.unwrap_or(1).clamp(1, total_pages);
    let start = (page - 1) * per_page;
    let page_rows = filtered.into_iter().skip(start).take(per_page).collect::<Vec<_>>();

    (page_rows, source_counts, selected_source, page, total_pages)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    fn workspace_root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .canonicalize()
            .unwrap()
    }

    #[tokio::test]
    async fn handler_smoke_get_index() {
        let app = app(AppState::new(workspace_root()));
        let resp = app
            .oneshot(axum::http::Request::builder().uri("/").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = resp.into_body().collect().await.unwrap().to_bytes();
        let text = String::from_utf8(body.to_vec()).unwrap();
        assert!(text.contains("RHOF Dashboard"));
    }

    #[tokio::test]
    async fn handler_smoke_htmx_partials() {
        let app = app(AppState::new(workspace_root()));
        let table = app
            .clone()
            .oneshot(axum::http::Request::builder().uri("/opportunities/table").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(table.status(), StatusCode::OK);

        let facets = app
            .oneshot(axum::http::Request::builder().uri("/opportunities/facets").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(facets.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn handler_smoke_reports_chart_json() {
        let app = app(AppState::new(workspace_root()));
        let resp = app
            .oneshot(axum::http::Request::builder().uri("/reports/chart").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        assert_eq!(resp.headers()[header::CONTENT_TYPE].to_str().unwrap(), "application/json");
    }

    #[tokio::test]
    async fn handler_smoke_review_resolve_post() {
        let app = app(AppState::new(workspace_root()));
        let resp = app
            .oneshot(
                axum::http::Request::builder()
                    .method("POST")
                    .uri("/review/abc/resolve")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
    }
}
