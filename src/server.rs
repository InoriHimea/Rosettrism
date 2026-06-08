use std::net::SocketAddr;

use axum::body::Body;
use axum::extract::{Path, Query, State};
use axum::http::{header, HeaderMap, HeaderValue, Response, StatusCode};
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};
use rust_embed::RustEmbed;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::cache::UpstreamCache;
use crate::provider::{builtin_provider_registry, SearchResult, Source};
use crate::service::{
    source_from_cli_name, AggregateFetchRequest, AiScoringConfig, LyricNeed, MergeMode,
    ServiceContext, SourceSearchRequest, SourceSearchResult, SpecificFetchFormat,
    SpecificFetchResult,
};
use crate::{Error, Result};

#[derive(Clone)]
pub struct ServerOptions {
    pub host: String,
    pub port: u16,
    pub open_browser: bool,
    pub context: ServiceContext,
}

#[derive(Clone)]
struct AppState {
    context: ServiceContext,
    server_token: Option<String>,
}

#[derive(RustEmbed)]
#[folder = "frontend/dist/"]
struct DashboardAssets;

const DEFAULT_SOURCE_SEARCH_LIMIT: usize = 100;
const DEFAULT_AGGREGATE_SEARCH_LIMIT: usize = 10;

pub async fn run(options: ServerOptions) -> Result<()> {
    let token = std::env::var("ROSETTRISM_SERVER_TOKEN")
        .ok()
        .filter(|value| !value.trim().is_empty());
    if !is_local_host(&options.host) && token.is_none() {
        return Err(Error::Service(
            "binding to a non-local host requires ROSETTRISM_SERVER_TOKEN".into(),
        ));
    }

    let addr: SocketAddr = format!("{}:{}", options.host, options.port)
        .parse()
        .map_err(|err| Error::Service(format!("invalid server address: {err}")))?;
    let state = AppState {
        context: options.context,
        server_token: token,
    };
    let app = app(state);
    let listener = tokio::net::TcpListener::bind(addr).await?;
    let url = format!("http://{addr}");
    eprintln!("Rosettrism server listening on {url}");
    if options.open_browser {
        open_browser(&url);
    }
    axum::serve(listener, app).await?;
    Ok(())
}

fn app(state: AppState) -> Router {
    Router::new()
        .route("/api/health", get(health))
        .route("/api/providers/health", get(providers_health))
        .route("/api/sources", get(sources))
        .route("/api/search", post(search))
        .route("/api/fetch", post(fetch))
        .route("/api/fetch-result", post(fetch_result))
        .route("/api/ai/replay", post(ai_replay))
        .route("/api/cache", get(cache_list))
        .route("/api/cache/:id", get(cache_detail).delete(cache_delete))
        .route("/api/cache/:id/revalidate", post(cache_revalidate))
        .route(
            "/api/unified-cache/:id",
            get(unified_cache_detail).delete(unified_cache_delete),
        )
        .route(
            "/api/unified-cache/:id/revalidate",
            post(unified_cache_revalidate),
        )
        .route("/api/runs", get(fetch_runs))
        .route("/api/stats", get(stats))
        .route("/", get(index))
        .route("/*path", get(asset))
        .with_state(state)
}

async fn health(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> ApiResult<Json<serde_json::Value>> {
    authorize(&state, &headers)?;
    Ok(Json(json!({
        "ok": true,
        "version": env!("CARGO_PKG_VERSION"),
        "cache": state.context.cache.is_some()
    })))
}

#[derive(Debug, Deserialize)]
struct ProviderHealthQuery {
    limit: Option<usize>,
}

async fn providers_health(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<ProviderHealthQuery>,
) -> ApiResult<Json<serde_json::Value>> {
    authorize(&state, &headers)?;
    let cache = require_cache(&state)?;
    let limit = query.limit.unwrap_or(20).clamp(1, 500);
    Ok(Json(json!({
        "sample_size": limit,
        "providers": enrich_provider_health(cache.provider_health(limit)?),
    })))
}

async fn sources(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> ApiResult<Json<serde_json::Value>> {
    authorize(&state, &headers)?;
    Ok(Json(json!({
        "manifest_file": crate::provider::ProviderManifest::manifest_file_name(),
        "sources": builtin_provider_registry(),
    })))
}

async fn fetch(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<ApiFetchRequest>,
) -> ApiResult<Response<Body>> {
    authorize(&state, &headers)?;
    if request.query.trim().is_empty() {
        return Err(ApiError::bad_request("query must not be empty"));
    }

    let ttl = request.ttl_seconds.map(std::time::Duration::from_secs);
    let force = request.force.unwrap_or(false);
    if let Some(source) = request.source.as_deref() {
        let source = source_from_cli_name(source)?;
        let top = request.top.filter(|top| *top > 0);
        if top.is_none() {
            let result = state
                .context
                .search_source_specific(source, &request.query, ttl, force)
                .await?;
            let body = serde_json::to_vec_pretty(&result)?;
            return Ok(response_with_body(
                StatusCode::OK,
                "application/json; charset=utf-8",
                body,
            ));
        }
        let format = request.format.ok_or_else(|| {
            ApiError::bad_request("source-specific fetch requires format=raw or format=json")
        })?;
        let result = state
            .context
            .fetch_source_specific(source, &request.query, format, top.unwrap_or(1), ttl, force)
            .await?;
        return match result {
            SpecificFetchResult::Raw { raw, .. } => Ok(response_with_body(
                StatusCode::OK,
                "text/plain; charset=utf-8",
                raw,
            )),
            SpecificFetchResult::Json {
                document, unified, ..
            } => {
                let body = if let Some(unified) = unified {
                    serde_json::to_vec_pretty(&unified)?
                } else {
                    serde_json::to_vec_pretty(&document)?
                };
                Ok(response_with_body(
                    StatusCode::OK,
                    "application/json; charset=utf-8",
                    body,
                ))
            }
            SpecificFetchResult::RawMany {
                source,
                results,
                warnings,
            } => {
                let body = serde_json::to_vec_pretty(&json!({
                    "source": source,
                    "format": "raw",
                    "results": results,
                    "warnings": warnings,
                }))?;
                Ok(response_with_body(
                    StatusCode::OK,
                    "application/json; charset=utf-8",
                    body,
                ))
            }
            SpecificFetchResult::JsonMany {
                source,
                results,
                warnings,
            } => {
                let body = serde_json::to_vec_pretty(&json!({
                    "source": source,
                    "format": "json",
                    "results": results,
                    "warnings": warnings,
                }))?;
                Ok(response_with_body(
                    StatusCode::OK,
                    "application/json; charset=utf-8",
                    body,
                ))
            }
        };
    }

    if request.format == Some(SpecificFetchFormat::Raw) {
        return Err(ApiError::bad_request(
            "aggregate fetch returns unified JSON; specify source for raw output",
        ));
    }

    let aggregate = state
        .context
        .aggregate_fetch(AggregateFetchRequest {
            query: request.query,
            merge_mode: request.merge_mode.unwrap_or_default(),
            top: request.top.unwrap_or(1),
            needs: request.needs.unwrap_or_else(|| LyricNeed::parse_list(None)),
            translation_lang: request.translation_lang.unwrap_or_else(|| "zh-Hans".into()),
            sources: request
                .sources
                .as_ref()
                .map(|sources| {
                    sources
                        .iter()
                        .map(|source| source_from_cli_name(source))
                        .collect::<Result<Vec<_>>>()
                })
                .transpose()?,
            force,
            ttl_seconds: request.ttl_seconds,
            ai_scoring: request.ai_scoring,
        })
        .await?;
    let body = serde_json::to_vec_pretty(&aggregate)?;
    Ok(response_with_body(
        StatusCode::OK,
        "application/json; charset=utf-8",
        body,
    ))
}

async fn search(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<ApiSearchRequest>,
) -> ApiResult<Response<Body>> {
    authorize(&state, &headers)?;
    let intent = request.search_intent()?;
    let query = intent.provider_query();
    let ttl = request.ttl_seconds.map(std::time::Duration::from_secs);
    let force = request.force.unwrap_or(false);

    if let Some(source) = request.source.as_deref() {
        let source = source_from_cli_name(source)?;
        let mut result = state
            .context
            .search_source_specific(source, &query, ttl, force)
            .await?;
        result.results = filter_results(result.results, &intent);
        add_direct_id_fallback(&mut result, &intent);
        result
            .results
            .truncate(request.search_limit(DEFAULT_SOURCE_SEARCH_LIMIT));
        let body = search_response_body(&result)?;
        return Ok(response_with_body(
            StatusCode::OK,
            "application/json; charset=utf-8",
            body,
        ));
    }

    let mut result = state
        .context
        .search_sources(SourceSearchRequest {
            query,
            sources: request
                .sources
                .as_ref()
                .map(|sources| {
                    sources
                        .iter()
                        .map(|source| source_from_cli_name(source))
                        .collect::<Result<Vec<_>>>()
                })
                .transpose()?,
            limit: DEFAULT_SOURCE_SEARCH_LIMIT,
            force,
            ttl_seconds: request.ttl_seconds,
        })
        .await?;
    for group in &mut result.sources {
        group.results = filter_results(std::mem::take(&mut group.results), &intent);
        add_direct_id_fallback(group, &intent);
    }
    result.results = aggregate_search_results(
        &result.sources,
        request.merge_mode.unwrap_or_default(),
        request.search_limit(DEFAULT_AGGREGATE_SEARCH_LIMIT),
    );
    let body = search_response_body(&result)?;
    Ok(response_with_body(
        StatusCode::OK,
        "application/json; charset=utf-8",
        body,
    ))
}

async fn fetch_result(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<ApiFetchResultRequest>,
) -> ApiResult<Response<Body>> {
    authorize(&state, &headers)?;
    let ttl = request.ttl_seconds.map(std::time::Duration::from_secs);
    let force = request.force.unwrap_or(false);
    if let Some(members) = aggregate_members(&request.result)? {
        if request.format == SpecificFetchFormat::Raw {
            return Err(ApiError::bad_request(
                "aggregate search results only support format=json",
            ));
        }
        let merge_mode = aggregate_merge_mode(&request.result).unwrap_or_default();
        let unified = state
            .context
            .fetch_aggregate_members(members, merge_mode, ttl, force, request.ai_scoring.as_ref())
            .await?;
        let body = serde_json::to_vec_pretty(&json!({
            "source": "aggregate",
            "format": "json",
            "result": request.result,
            "unified": unified,
            "document": unified,
        }))?;
        return Ok(response_with_body(
            StatusCode::OK,
            "application/json; charset=utf-8",
            body,
        ));
    }
    let result = state
        .context
        .fetch_source_result(request.result, request.format, ttl, force)
        .await?;
    match result {
        SpecificFetchResult::Raw {
            source,
            result,
            raw,
        } => {
            let body = serde_json::to_vec_pretty(&json!({
                "source": source,
                "format": "raw",
                "result": result,
                "raw": String::from_utf8_lossy(&raw),
            }))?;
            Ok(response_with_body(
                StatusCode::OK,
                "application/json; charset=utf-8",
                body,
            ))
        }
        SpecificFetchResult::Json {
            source,
            result,
            input_format,
            document,
            annotations,
            unified,
        } => {
            let mut body = json!({
                "source": source,
                "format": "json",
                "input_format": input_format,
                "result": result,
                "document": document,
                "annotations": annotations,
            });
            if let Some(unified) = unified {
                body["unified"] = json!(unified);
            }
            let body = serde_json::to_vec_pretty(&body)?;
            Ok(response_with_body(
                StatusCode::OK,
                "application/json; charset=utf-8",
                body,
            ))
        }
        SpecificFetchResult::RawMany { .. } | SpecificFetchResult::JsonMany { .. } => Err(
            ApiError::bad_request("fetch-result expects one selected search result"),
        ),
    }
}

#[derive(Debug, Deserialize)]
struct AiReplayRequest {
    unified_cache_id: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    ai_scoring: Option<AiScoringConfig>,
}

async fn ai_replay(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<AiReplayRequest>,
) -> ApiResult<Json<serde_json::Value>> {
    authorize(&state, &headers)?;
    let score = state
        .context
        .replay_ai_score_for_unified_cache(request.unified_cache_id, request.ai_scoring.as_ref())
        .await?;
    Ok(Json(json!({
        "unified_cache_id": request.unified_cache_id,
        "ai_score": score
    })))
}

async fn cache_list(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> ApiResult<Json<serde_json::Value>> {
    authorize(&state, &headers)?;
    let cache = require_cache(&state)?;
    let upstream_entries = cache.list(100)?;
    let unified_entries = cache.list_unified(100)?;
    Ok(Json(json!({
        "upstream_entries": upstream_entries,
        "unified_entries": unified_entries,
        "entries": upstream_entries
    })))
}

async fn cache_detail(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> ApiResult<Json<serde_json::Value>> {
    authorize(&state, &headers)?;
    let cache = require_cache(&state)?;
    let entry = cache.detail(id)?;
    let unified_entry = cache.unified_detail(id)?;
    if entry.is_none() && unified_entry.is_none() {
        return Err(ApiError::not_found("cache entry not found"));
    }
    Ok(Json(json!({
        "entry": entry,
        "unified_entry": unified_entry,
        "ai_scores": unified_entry
            .as_ref()
            .map(|entry| cache.list_ai_scores(entry.id))
            .transpose()?
            .unwrap_or_default()
    })))
}

async fn unified_cache_detail(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> ApiResult<Json<serde_json::Value>> {
    authorize(&state, &headers)?;
    let cache = require_cache(&state)?;
    let unified_entry = cache.unified_detail(id)?;
    if unified_entry.is_none() {
        return Err(ApiError::not_found("unified cache entry not found"));
    }
    Ok(Json(json!({
        "entry": null,
        "unified_entry": unified_entry,
        "ai_scores": cache.list_ai_scores(id)?
    })))
}

async fn cache_delete(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> ApiResult<Json<serde_json::Value>> {
    authorize(&state, &headers)?;
    let cache = require_cache(&state)?;
    let deleted = cache.delete(id)?;
    Ok(Json(json!({ "deleted": deleted })))
}

async fn cache_revalidate(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> ApiResult<Json<serde_json::Value>> {
    authorize(&state, &headers)?;
    let cache = require_cache(&state)?;
    let deleted = cache.delete(id)?;
    Ok(Json(json!({
        "revalidate": "deleted_cached_entry",
        "deleted": deleted
    })))
}

async fn unified_cache_delete(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> ApiResult<Json<serde_json::Value>> {
    authorize(&state, &headers)?;
    let cache = require_cache(&state)?;
    let deleted = cache.delete_unified(id)?;
    Ok(Json(json!({ "deleted": deleted })))
}

async fn unified_cache_revalidate(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> ApiResult<Json<serde_json::Value>> {
    authorize(&state, &headers)?;
    let cache = require_cache(&state)?;
    let deleted = cache.delete_unified(id)?;
    Ok(Json(json!({
        "revalidate": "deleted_unified_cached_entry",
        "deleted": deleted
    })))
}

async fn fetch_runs(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> ApiResult<Json<serde_json::Value>> {
    authorize(&state, &headers)?;
    let cache = require_cache(&state)?;
    Ok(Json(json!({
        "runs": cache.list_fetch_runs(100)?,
        "status_counts": cache.fetch_run_status_counts()?
    })))
}

#[derive(Debug, Deserialize)]
struct StatsQuery {
    query_hash: Option<String>,
    model: Option<String>,
    prompt_version: Option<String>,
    created_at_start: Option<i64>,
    created_at_end: Option<i64>,
}

async fn stats(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<StatsQuery>,
) -> ApiResult<Json<serde_json::Value>> {
    authorize(&state, &headers)?;
    let cache = require_cache(&state)?;
    Ok(Json(json!({
        "cache": cache.stats()?,
        "ai_scores": cache.query_ai_scores(AiScoreQuery {
            query_hash: query.query_hash,
            model: query.model,
            prompt_version: query.prompt_version,
            created_at_start: query.created_at_start,
            created_at_end: query.created_at_end,
            limit: 20,
        })?,
        "fetch_runs": cache.list_fetch_runs(20)?,
        "fetch_run_status_counts": cache.fetch_run_status_counts()?,
        "provider_health": enrich_provider_health(cache.provider_health(20)?)
    })))
}

fn enrich_provider_health(
    records: Vec<crate::cache::ProviderHealthRecord>,
) -> Vec<serde_json::Value> {
    records
        .into_iter()
        .map(|record| {
            let mut value = serde_json::to_value(&record).unwrap_or_else(|_| {
                json!({
                    "source": record.source,
                    "status": "unknown"
                })
            });
            if let Some(object) = value.as_object_mut() {
                if let Ok(source) = source_from_cli_name(&record.source) {
                    let manifest = source.manifest();
                    object.insert("capabilities".into(), json!(manifest.capabilities));
                    object.insert("config".into(), json!(manifest.config));
                    object.insert("auth".into(), json!(manifest.auth));
                    object.insert(
                        "experimental".into(),
                        json!(manifest.capabilities.experimental),
                    );
                    object.insert("display_name".into(), json!(manifest.display_name));
                }
            }
            value
        })
        .collect()
}

async fn index() -> impl IntoResponse {
    asset_response("index.html")
}

async fn asset(Path(path): Path<String>) -> impl IntoResponse {
    let path = path.trim_start_matches('/');
    if path.starts_with("api/") {
        return StatusCode::NOT_FOUND.into_response();
    }
    asset_response(if path.is_empty() { "index.html" } else { path })
}

fn asset_response(path: &str) -> axum::response::Response {
    let asset = DashboardAssets::get(path).or_else(|| DashboardAssets::get("index.html"));
    let Some(asset) = asset else {
        return (
            StatusCode::NOT_FOUND,
            "dashboard assets were not embedded; run npm build in frontend",
        )
            .into_response();
    };
    let mime = mime_guess::from_path(path).first_or_text_plain();
    response_with_body(StatusCode::OK, mime.as_ref(), asset.data.into_owned()).into_response()
}

fn response_with_body(
    status: StatusCode,
    content_type: &str,
    body: impl Into<Vec<u8>>,
) -> Response<Body> {
    let mut response = Response::new(Body::from(body.into()));
    *response.status_mut() = status;
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_str(content_type)
            .unwrap_or_else(|_| HeaderValue::from_static("application/octet-stream")),
    );
    response
}

fn authorize(state: &AppState, headers: &HeaderMap) -> ApiResult<()> {
    let Some(expected) = state.server_token.as_deref() else {
        return Ok(());
    };
    let bearer = headers
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "));
    let explicit = headers
        .get("x-rosettrism-token")
        .and_then(|value| value.to_str().ok());
    if bearer == Some(expected) || explicit == Some(expected) {
        Ok(())
    } else {
        Err(ApiError::unauthorized("missing or invalid server token"))
    }
}

fn require_cache(state: &AppState) -> ApiResult<&UpstreamCache> {
    state
        .context
        .cache
        .as_ref()
        .ok_or_else(|| ApiError::bad_request("cache is disabled"))
}

fn is_local_host(host: &str) -> bool {
    matches!(host, "127.0.0.1" | "localhost" | "::1")
}

fn open_browser(url: &str) {
    #[cfg(windows)]
    {
        let _ = std::process::Command::new("cmd")
            .args(["/C", "start", "", url])
            .spawn();
    }
    #[cfg(target_os = "macos")]
    {
        let _ = std::process::Command::new("open").arg(url).spawn();
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let _ = std::process::Command::new("xdg-open").arg(url).spawn();
    }
}

#[derive(Debug, Deserialize)]
struct ApiFetchRequest {
    query: String,
    #[serde(default)]
    source: Option<String>,
    #[serde(default)]
    format: Option<SpecificFetchFormat>,
    #[serde(default)]
    merge_mode: Option<MergeMode>,
    #[serde(default)]
    top: Option<usize>,
    #[serde(default)]
    needs: Option<Vec<LyricNeed>>,
    #[serde(default)]
    translation_lang: Option<String>,
    #[serde(default)]
    sources: Option<Vec<String>>,
    #[serde(default)]
    force: Option<bool>,
    #[serde(default)]
    ttl_seconds: Option<u64>,
    #[serde(default)]
    ai_scoring: Option<AiScoringConfig>,
}

#[derive(Debug, Deserialize)]
struct ApiSearchRequest {
    #[serde(default)]
    query: Option<String>,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    artist: Option<String>,
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    source: Option<String>,
    #[serde(default)]
    sources: Option<Vec<String>>,
    #[serde(default)]
    top: Option<usize>,
    #[serde(default)]
    limit: Option<usize>,
    #[serde(default)]
    merge_mode: Option<MergeMode>,
    #[serde(default)]
    force: Option<bool>,
    #[serde(default)]
    ttl_seconds: Option<u64>,
}

impl ApiSearchRequest {
    fn search_intent(&self) -> ApiResult<SearchIntent> {
        if let Some(id) = self
            .id
            .as_deref()
            .map(str::trim)
            .filter(|id| !id.is_empty())
        {
            return Ok(SearchIntent::Id(id.to_string()));
        }

        let title = self.title.as_deref().map(str::trim).unwrap_or_default();
        let artist = self.artist.as_deref().map(str::trim).unwrap_or_default();
        if !title.is_empty() || !artist.is_empty() {
            return Ok(SearchIntent::Fields {
                title: (!title.is_empty()).then(|| title.to_string()),
                artist: (!artist.is_empty()).then(|| artist.to_string()),
            });
        }

        let query = self.query.as_deref().map(str::trim).unwrap_or_default();
        if !query.is_empty() {
            return Ok(SearchIntent::Keyword(query.to_string()));
        }

        Err(ApiError::bad_request(
            "query, title, artist, or id must not be empty",
        ))
    }

    fn search_limit(&self, default_limit: usize) -> usize {
        self.top
            .or(self.limit)
            .filter(|limit| *limit > 0)
            .unwrap_or(default_limit)
            .clamp(1, default_limit)
    }
}

#[derive(Debug, Clone)]
enum SearchIntent {
    Id(String),
    Fields {
        title: Option<String>,
        artist: Option<String>,
    },
    Keyword(String),
}

impl SearchIntent {
    fn provider_query(&self) -> String {
        match self {
            SearchIntent::Id(id) => id.clone(),
            SearchIntent::Fields { title, artist } => [title.as_deref(), artist.as_deref()]
                .into_iter()
                .flatten()
                .filter(|part| !part.trim().is_empty())
                .collect::<Vec<_>>()
                .join(" "),
            SearchIntent::Keyword(query) => query.clone(),
        }
    }
}

fn filter_results(results: Vec<SearchResult>, intent: &SearchIntent) -> Vec<SearchResult> {
    results
        .into_iter()
        .filter(|result| result_matches_intent(result, intent))
        .collect()
}

fn result_matches_intent(result: &SearchResult, intent: &SearchIntent) -> bool {
    match intent {
        SearchIntent::Id(id) => result_matches_id(result, id),
        SearchIntent::Fields { title, artist } => {
            title
                .as_deref()
                .is_none_or(|title| title_field_matches(&result.title, title))
                && artist
                    .as_deref()
                    .is_none_or(|artist| field_matches(&result.artist, artist))
        }
        SearchIntent::Keyword(query) => keyword_matches(result, query),
    }
}

fn add_direct_id_fallback(result: &mut SourceSearchResult, intent: &SearchIntent) {
    let SearchIntent::Id(id) = intent else {
        return;
    };
    if result.results.is_empty() {
        result.results.push(direct_id_result(result.source, id));
    }
}

fn direct_id_result(source: Source, id: &str) -> SearchResult {
    let id = id.trim().to_string();
    SearchResult {
        source,
        id: id.clone(),
        title: id.clone(),
        artist: String::new(),
        album: None,
        duration_ms: None,
        extra: direct_id_extra(source, &id),
    }
}

fn direct_id_extra(source: Source, id: &str) -> Value {
    match source {
        Source::Qq => match id.parse::<u64>() {
            Ok(song_id) => json!({
                "direct_id": true,
                "songid": song_id
            }),
            Err(_) => json!({
                "direct_id": true,
                "songmid": id
            }),
        },
        Source::Kugou => json!({
            "direct_id": true,
            "provider_result": "song",
            "fmt": "krc",
            "album_audio_id": id.parse::<u64>().ok(),
            "hash": looks_like_kugou_hash(id).then_some(id)
        }),
        _ => json!({
            "direct_id": true
        }),
    }
}

fn looks_like_kugou_hash(id: &str) -> bool {
    id.len() == 32 && id.chars().all(|ch| ch.is_ascii_hexdigit())
}

fn result_matches_id(result: &SearchResult, id: &str) -> bool {
    let needle = normalized_text(id);
    if needle.is_empty() {
        return false;
    }

    normalized_text(&result.id) == needle || value_contains_exact_id(&result.extra, &needle)
}

fn value_contains_exact_id(value: &Value, needle: &str) -> bool {
    match value {
        Value::String(value) => normalized_text(value) == needle,
        Value::Number(value) => normalized_text(&value.to_string()) == needle,
        Value::Array(values) => values
            .iter()
            .any(|value| value_contains_exact_id(value, needle)),
        Value::Object(values) => values
            .values()
            .any(|value| value_contains_exact_id(value, needle)),
        _ => false,
    }
}

fn field_matches(value: &str, filter: &str) -> bool {
    let value = normalized_text(value);
    let filter = normalized_text(filter);
    if value.is_empty() || filter.is_empty() {
        return false;
    }
    if value == filter || value.contains(&filter) {
        return true;
    }

    let reversed = filter.chars().rev().collect::<String>();
    if filter.chars().count() <= 3 && value == reversed {
        return true;
    }

    common_char_ratio(&value, &filter) >= 0.9
}

fn title_field_matches(value: &str, filter: &str) -> bool {
    let value = normalized_text(value);
    let filter = normalized_text(filter);
    if value.is_empty() || filter.is_empty() {
        return false;
    }
    if value == filter || value.starts_with(&filter) {
        return true;
    }

    let reversed = filter.chars().rev().collect::<String>();
    filter.chars().count() <= 3 && value == reversed
}

fn keyword_matches(result: &SearchResult, query: &str) -> bool {
    let haystack = normalized_text(
        &[
            result.title.as_str(),
            result.artist.as_str(),
            result.album.as_deref().unwrap_or_default(),
            result.id.as_str(),
        ]
        .join(" "),
    );
    let terms = query
        .split_whitespace()
        .map(normalized_text)
        .filter(|term| !term.is_empty())
        .collect::<Vec<_>>();

    !terms.is_empty()
        && terms.iter().all(|term| {
            haystack.contains(term)
                || (term.chars().count() <= 3
                    && haystack.contains(&term.chars().rev().collect::<String>()))
        })
}

fn common_char_ratio(value: &str, filter: &str) -> f32 {
    let mut chars = value.chars().collect::<Vec<_>>();
    let mut matched = 0usize;
    for ch in filter.chars() {
        if let Some(index) = chars.iter().position(|candidate| *candidate == ch) {
            chars.remove(index);
            matched += 1;
        }
    }

    let total = filter.chars().count();
    if total == 0 {
        0.0
    } else {
        matched as f32 / total as f32
    }
}

fn normalized_text(value: &str) -> String {
    value
        .chars()
        .flat_map(char::to_lowercase)
        .filter(|ch| !ch.is_whitespace() && !ch.is_ascii_punctuation() && !is_cjk_punctuation(*ch))
        .collect()
}

fn is_cjk_punctuation(ch: char) -> bool {
    matches!(
        ch,
        '，' | '。'
            | '、'
            | '：'
            | '；'
            | '！'
            | '？'
            | '（'
            | '）'
            | '《'
            | '》'
            | '「'
            | '」'
            | '『'
            | '』'
            | '【'
            | '】'
            | '－'
            | '—'
            | '～'
    )
}

fn aggregate_search_results(
    groups: &[SourceSearchResult],
    merge_mode: MergeMode,
    limit: usize,
) -> Vec<SearchResult> {
    let mut aggregate_groups: Vec<AggregateSearchGroup> = Vec::new();
    for result in groups
        .iter()
        .flat_map(|group| group.results.iter().cloned())
    {
        if let Some(group) = aggregate_groups
            .iter_mut()
            .find(|group| group.can_include(&result))
        {
            group.push(result);
        } else {
            aggregate_groups.push(AggregateSearchGroup::new(result));
        }
    }

    aggregate_groups.sort_by(|left, right| {
        right
            .members
            .len()
            .cmp(&left.members.len())
            .then_with(|| left.order.cmp(&right.order))
    });

    aggregate_groups
        .into_iter()
        .take(limit)
        .enumerate()
        .map(|(index, group)| group.into_result(index, merge_mode))
        .collect()
}

struct AggregateSearchGroup {
    title_key: String,
    artist_key: String,
    duration_ms: Option<u32>,
    order: usize,
    members: Vec<SearchResult>,
}

impl AggregateSearchGroup {
    fn new(result: SearchResult) -> Self {
        static NEXT_ORDER: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        Self {
            title_key: normalized_text(&result.title),
            artist_key: normalized_text(&result.artist),
            duration_ms: result.duration_ms,
            order: NEXT_ORDER.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            members: vec![result],
        }
    }

    fn can_include(&self, result: &SearchResult) -> bool {
        self.title_key == normalized_text(&result.title)
            && self.artist_key == normalized_text(&result.artist)
            && duration_close(self.duration_ms, result.duration_ms)
    }

    fn push(&mut self, result: SearchResult) {
        if self
            .members
            .iter()
            .any(|member| member.source == result.source)
        {
            return;
        }
        self.members.push(result);
    }

    fn into_result(self, index: usize, merge_mode: MergeMode) -> SearchResult {
        let base = self
            .members
            .iter()
            .max_by_key(|member| source_rank(member.source))
            .cloned()
            .unwrap_or_else(|| self.members[0].clone());
        let sources = unique_result_sources(&self.members);
        let display_source = format!("聚合({})", sources.join("+"));
        SearchResult {
            source: base.source,
            id: format!("aggregate:{}", index + 1),
            title: base.title,
            artist: base.artist,
            album: base.album,
            duration_ms: base.duration_ms,
            extra: json!({
                "result_kind": "aggregate",
                "display_source": display_source,
                "aggregate_sources": sources,
                "aggregate_members": self.members,
                "merge_mode": merge_mode,
            }),
        }
    }
}

fn duration_close(left: Option<u32>, right: Option<u32>) -> bool {
    match (left, right) {
        (Some(left), Some(right)) => left.abs_diff(right) <= 15_000,
        _ => true,
    }
}

fn source_rank(source: Source) -> u8 {
    match source {
        Source::Kugou => 7,
        Source::Qq => 6,
        Source::Netease => 5,
        Source::Lrclib => 4,
        Source::Migu => 3,
        _ => 1,
    }
}

fn unique_result_sources(results: &[SearchResult]) -> Vec<String> {
    let mut sources = Vec::new();
    for result in results {
        if !sources.contains(&result.source) {
            sources.push(result.source);
        }
    }
    sources.sort_by(|left, right| {
        source_rank(*right)
            .cmp(&source_rank(*left))
            .then_with(|| left.cli_name().cmp(right.cli_name()))
    });
    sources
        .into_iter()
        .map(|source| source.cli_name().to_string())
        .collect()
}

fn aggregate_members(result: &SearchResult) -> ApiResult<Option<Vec<SearchResult>>> {
    if result.extra.get("result_kind").and_then(Value::as_str) != Some("aggregate") {
        return Ok(None);
    }

    let members = result
        .extra
        .get("aggregate_members")
        .cloned()
        .ok_or_else(|| ApiError::bad_request("aggregate result is missing members"))?;
    serde_json::from_value::<Vec<SearchResult>>(members)
        .map(Some)
        .map_err(|err| ApiError::bad_request(format!("invalid aggregate members: {err}")))
}

fn aggregate_merge_mode(result: &SearchResult) -> Option<MergeMode> {
    serde_json::from_value(result.extra.get("merge_mode")?.clone()).ok()
}

fn search_response_body<T: Serialize>(value: &T) -> ApiResult<Vec<u8>> {
    let mut value = serde_json::to_value(value)?;
    inject_display_source(&mut value);
    Ok(serde_json::to_vec_pretty(&value)?)
}

fn inject_display_source(value: &mut Value) {
    match value {
        Value::Object(object) => {
            if object.contains_key("title") && object.contains_key("source") {
                let display_source = object
                    .get("extra")
                    .and_then(|extra| extra.get("display_source"))
                    .and_then(Value::as_str)
                    .map(ToOwned::to_owned)
                    .or_else(|| {
                        object
                            .get("source")
                            .and_then(Value::as_str)
                            .map(ToOwned::to_owned)
                    });
                if let Some(display_source) = display_source {
                    object.insert("display_source".into(), Value::String(display_source));
                }
            }
            for value in object.values_mut() {
                inject_display_source(value);
            }
        }
        Value::Array(values) => {
            for value in values {
                inject_display_source(value);
            }
        }
        _ => {}
    }
}

#[derive(Debug, Deserialize)]
struct ApiFetchResultRequest {
    result: SearchResult,
    format: SpecificFetchFormat,
    #[serde(default)]
    force: Option<bool>,
    #[serde(default)]
    ttl_seconds: Option<u64>,
    #[serde(default)]
    ai_scoring: Option<AiScoringConfig>,
}

type ApiResult<T> = std::result::Result<T, ApiError>;

#[derive(Debug, Serialize)]
struct ApiErrorBody {
    code: &'static str,
    message: String,
    details: Option<Value>,
    retryable: bool,
}

#[derive(Debug)]
struct ApiError {
    status: StatusCode,
    body: ApiErrorBody,
}

impl ApiError {
    fn new(
        status: StatusCode,
        code: &'static str,
        message: impl Into<String>,
        details: Option<Value>,
        retryable: bool,
    ) -> Self {
        Self {
            status,
            body: ApiErrorBody {
                code,
                message: message.into(),
                details,
                retryable,
            },
        }
    }

    fn bad_request(message: impl Into<String>) -> Self {
        let message = message.into();
        let code = if message == "cache is disabled" {
            "cache_disabled"
        } else {
            "validation_error"
        };
        Self::new(StatusCode::BAD_REQUEST, code, message, None, false)
    }

    fn unauthorized(message: impl Into<String>) -> Self {
        Self::new(
            StatusCode::UNAUTHORIZED,
            "auth_missing_or_invalid",
            message,
            None,
            false,
        )
    }

    fn not_found(message: impl Into<String>) -> Self {
        Self::new(
            StatusCode::NOT_FOUND,
            "no_lyrics_found",
            message,
            None,
            false,
        )
    }
}

impl From<Error> for ApiError {
    fn from(value: Error) -> Self {
        let message = value.to_string();
        let (status, code, retryable) = match &value {
            Error::Provider(_) if message.contains("ai_skipped") => {
                (StatusCode::BAD_GATEWAY, "ai_skipped", true)
            }
            Error::Provider(_) if message.contains("provider_warning") => {
                (StatusCode::BAD_GATEWAY, "provider_warning", true)
            }
            Error::Provider(_) if looks_like_no_lyrics_error(&message) => {
                (StatusCode::NOT_FOUND, "no_lyrics_found", false)
            }
            Error::Provider(_) | Error::Network(_) => {
                (StatusCode::BAD_GATEWAY, "provider_warning", true)
            }
            Error::Service(_) if message.contains("invalid source") => {
                (StatusCode::BAD_REQUEST, "validation_error", false)
            }
            Error::Service(_) => (StatusCode::INTERNAL_SERVER_ERROR, "internal_error", false),
            Error::Json(_) | Error::Parse(_) | Error::Decode(_) | Error::UnknownFormat => {
                (StatusCode::BAD_REQUEST, "validation_error", false)
            }
            Error::Storage(_) | Error::Sqlite(_) | Error::Io(_) => {
                (StatusCode::INTERNAL_SERVER_ERROR, "internal_error", false)
            }
        };
        Self::new(
            status,
            code,
            message,
            Some(error_details(&value)),
            retryable,
        )
    }
}

impl From<serde_json::Error> for ApiError {
    fn from(value: serde_json::Error) -> Self {
        Self::new(
            StatusCode::BAD_REQUEST,
            "validation_error",
            value.to_string(),
            Some(json!({ "source": "serde_json" })),
            false,
        )
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        (self.status, Json(self.body)).into_response()
    }
}

fn looks_like_no_lyrics_error(message: &str) -> bool {
    let lower = message.to_ascii_lowercase();
    lower.contains("no_lyrics_found")
        || lower.contains("no lyric")
        || lower.contains("no aggregate members could be fetched")
}

fn error_details(value: &Error) -> Value {
    json!({
        "source": match value {
            Error::UnknownFormat => "unknown_format",
            Error::Decode(_) => "decode",
            Error::Parse(_) => "parse",
            Error::Provider(_) => "provider",
            Error::Service(_) => "service",
            Error::Storage(_) => "storage",
            Error::Network(_) => "network",
            Error::Sqlite(_) => "sqlite",
            Error::Io(_) => "io",
            Error::Json(_) => "json",
        }
    })
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    use axum::body::to_bytes;
    use axum::http::{Method, Request};
    use tower::ServiceExt;

    use super::*;
    use crate::cache::CachePut;

    #[test]
    fn api_error_maps_common_codes() {
        let cache = ApiError::bad_request("cache is disabled");
        assert_eq!(cache.status, StatusCode::BAD_REQUEST);
        assert_eq!(cache.body.code, "cache_disabled");
        assert!(!cache.body.retryable);

        let validation = ApiError::bad_request("query must not be empty");
        assert_eq!(validation.body.code, "validation_error");

        let provider_warning = ApiError::from(Error::Provider(
            "provider_warning: provider task failed".into(),
        ));
        assert_eq!(provider_warning.status, StatusCode::BAD_GATEWAY);
        assert_eq!(provider_warning.body.code, "provider_warning");
        assert!(provider_warning.body.retryable);

        let ai_skipped = ApiError::from(Error::Provider("ai_skipped: missing key".into()));
        assert_eq!(ai_skipped.body.code, "ai_skipped");
        assert!(ai_skipped.body.retryable);

        let no_lyrics = ApiError::from(Error::Provider(
            "no_lyrics_found: all selected sources returned no usable lyric".into(),
        ));
        assert_eq!(no_lyrics.status, StatusCode::NOT_FOUND);
        assert_eq!(no_lyrics.body.code, "no_lyrics_found");
    }

    #[tokio::test]
    async fn api_routes_return_json_with_cache_enabled() {
        let cache = test_cache("api_routes_return_json_with_cache_enabled");
        let cache_id = cache
            .put(CachePut {
                key: "upstream-route",
                source: Source::Lrclib,
                operation: "fetch",
                status_code: 200,
                body: br#"{"lyrics":"hello"}"#,
                metadata: &json!({ "query": "hello", "item_id": "lrclib-1" }),
                ttl: Duration::from_secs(60),
            })
            .unwrap();
        let run_id = cache
            .start_fetch_run("hello", Some(Source::Lrclib), "fetch")
            .unwrap();
        cache
            .finish_fetch_run(
                run_id,
                "success",
                None,
                crate::cache::FetchRunMetadata {
                    provider_count: Some(1),
                    candidate_count: Some(1),
                    cache_event: None,
                },
            )
            .unwrap();
        let app = test_app(cache, None);

        let (health_status, health) = get_json(app.clone(), "/api/health", None).await;
        assert_eq!(health_status, StatusCode::OK);
        assert_eq!(health["ok"], true);
        assert_eq!(health["cache"], true);

        let (stats_status, stats) = get_json(app.clone(), "/api/stats", None).await;
        assert_eq!(stats_status, StatusCode::OK);
        assert_eq!(stats["cache"]["upstream_entries"], 1);
        assert_eq!(stats["fetch_run_status_counts"][0]["status"], "success");
        assert_eq!(stats["provider_health"][0]["source"], "lrclib");

        let (provider_status, provider_health) =
            get_json(app.clone(), "/api/providers/health?limit=5", None).await;
        assert_eq!(provider_status, StatusCode::OK);
        assert_eq!(provider_health["providers"][0]["status"], "healthy");

        let (runs_status, runs) = get_json(app.clone(), "/api/runs", None).await;
        assert_eq!(runs_status, StatusCode::OK);
        assert_eq!(runs["runs"][0]["query"], "hello");

        let (cache_status, detail) = get_json(app, &format!("/api/cache/{cache_id}"), None).await;
        assert_eq!(cache_status, StatusCode::OK);
        assert_eq!(detail["entry"]["id"], cache_id);
        assert!(detail["unified_entry"].is_null());
    }

    #[tokio::test]
    async fn server_token_auth_accepts_explicit_and_bearer_headers() {
        let app = test_app(test_cache("server_token_auth"), Some("secret-token"));

        let (missing_status, missing) = get_json(app.clone(), "/api/health", None).await;
        assert_eq!(missing_status, StatusCode::UNAUTHORIZED);
        assert_eq!(missing["code"], "auth_missing_or_invalid");
        assert_eq!(missing["message"], "missing or invalid server token");
        assert_eq!(missing["retryable"], false);

        let (explicit_status, explicit) = get_json(
            app.clone(),
            "/api/health",
            Some(("x-rosettrism-token", "secret-token")),
        )
        .await;
        assert_eq!(explicit_status, StatusCode::OK);
        assert_eq!(explicit["ok"], true);

        let (bearer_status, bearer) = get_json(
            app,
            "/api/health",
            Some(("authorization", "Bearer secret-token")),
        )
        .await;
        assert_eq!(bearer_status, StatusCode::OK);
        assert_eq!(bearer["ok"], true);
    }

    #[tokio::test]
    async fn cache_detail_returns_upstream_entry_by_id() {
        let cache = test_cache("cache_detail_upstream");
        let id = cache
            .put(CachePut {
                key: "upstream-detail",
                source: Source::Qq,
                operation: "search",
                status_code: 200,
                body: br#"{"results":[]}"#,
                metadata: &json!({ "query": "track" }),
                ttl: Duration::from_secs(60),
            })
            .unwrap();

        let (status, detail) =
            get_json(test_app(cache, None), &format!("/api/cache/{id}"), None).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(detail["entry"]["id"], id);
        assert_eq!(detail["entry"]["source"], "qq");
        assert!(detail["unified_entry"].is_null());
    }

    #[tokio::test]
    async fn cache_detail_returns_unified_entry_by_id() {
        let cache = test_cache("cache_detail_unified");
        let id = cache
            .put_unified(
                "unified-detail",
                br#"{"metadata":{"title":"Track"},"lines":[]}"#,
                &["upstream-a".to_string()],
                Duration::from_secs(60),
            )
            .unwrap();

        let (status, detail) =
            get_json(test_app(cache, None), &format!("/api/cache/{id}"), None).await;
        assert_eq!(status, StatusCode::OK);
        assert!(detail["entry"].is_null());
        assert_eq!(detail["unified_entry"]["id"], id);
        assert_eq!(detail["unified_entry"]["dependencies"][0], "upstream-a");
    }

    #[tokio::test]
    async fn cache_detail_returns_not_found_for_missing_id() {
        let (status, detail) = get_json(
            test_app(test_cache("cache_detail_missing"), None),
            "/api/cache/9999",
            None,
        )
        .await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(detail["code"], "no_lyrics_found");
        assert_eq!(detail["message"], "cache entry not found");
    }
    #[tokio::test]
    async fn cache_list_splits_upstream_and_unified_entries() {
        let cache = test_cache("cache_list_split");
        let upstream_id = cache
            .put(CachePut {
                key: "upstream-list",
                source: Source::Qq,
                operation: "search",
                status_code: 200,
                body: br#"{"results":[]}"#,
                metadata: &json!({ "query": "track" }),
                ttl: Duration::from_secs(60),
            })
            .unwrap();
        let unified_id = cache
            .put_unified(
                "unified-list",
                br#"{"metadata":{"title":"Track"},"lines":[]}"#,
                &["upstream-list".to_string(), "upstream-other".to_string()],
                Duration::from_secs(60),
            )
            .unwrap();

        let (status, list) = get_json(test_app(cache, None), "/api/cache", None).await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(list["upstream_entries"][0]["id"], upstream_id);
        assert_eq!(list["unified_entries"][0]["id"], unified_id);
        assert_eq!(list["unified_entries"][0]["dependency_count"], 2);
        assert_eq!(list["entries"][0]["id"], upstream_id);
    }

    #[tokio::test]
    async fn deleting_unified_cache_removes_ai_scores() {
        let cache = test_cache("delete_unified_ai_scores");
        let unified_id = cache
            .put_unified(
                "unified-delete",
                br#"{"metadata":{"title":"Track"},"lines":[]}"#,
                &[],
                Duration::from_secs(60),
            )
            .unwrap();
        cache
            .put_ai_score(unified_id, &json!({ "candidate_summary_hash": "abc123" }))
            .unwrap();
        assert_eq!(cache.stats().unwrap().ai_score_entries, 1);

        let app = test_app(cache.clone(), None);
        let (status, deleted) = delete_json(
            app.clone(),
            &format!("/api/unified-cache/{unified_id}"),
            None,
        )
        .await;

        assert_eq!(status, StatusCode::OK);
        assert_eq!(deleted["deleted"], true);
        assert!(cache.unified_detail(unified_id).unwrap().is_none());
        assert_eq!(cache.list_ai_scores(unified_id).unwrap().len(), 0);
        assert_eq!(cache.stats().unwrap().ai_score_entries, 0);

        let (detail_status, detail) =
            get_json(app, &format!("/api/unified-cache/{unified_id}"), None).await;
        assert_eq!(detail_status, StatusCode::NOT_FOUND);
        assert_eq!(detail["code"], "no_lyrics_found");
        assert_eq!(detail["message"], "unified cache entry not found");
    }

    #[test]
    fn rejects_non_local_host_without_token() {
        std::env::remove_var("ROSETTRISM_SERVER_TOKEN");
        assert!(!is_local_host("0.0.0.0"));
    }

    #[test]
    fn field_search_filters_only_requested_fields() {
        let original = result(Source::Qq, "海阔天空", "BEYOND", Some("乐与怒"), "mid-1");
        let japanese = result(
            Source::Qq,
            "遥かなる夢に",
            "BEYOND",
            Some("遥かなる梦 1992~1995"),
            "mid-2",
        );
        let album_only = result(Source::Qq, "情人", "BEYOND", Some("海阔天空"), "mid-3");
        let cover = result(Source::Qq, "海阔天空", "Other", None, "mid-4");

        let both = SearchIntent::Fields {
            title: Some("海阔天空".into()),
            artist: Some("Beyond".into()),
        };
        assert!(result_matches_intent(&original, &both));
        assert!(!result_matches_intent(&japanese, &both));
        assert!(!result_matches_intent(&album_only, &both));
        assert!(!result_matches_intent(&cover, &both));

        let title_only = SearchIntent::Fields {
            title: Some("海阔天空".into()),
            artist: None,
        };
        assert!(result_matches_intent(&original, &title_only));
        assert!(result_matches_intent(&cover, &title_only));
        assert!(!result_matches_intent(&album_only, &title_only));

        let artist_only = SearchIntent::Fields {
            title: None,
            artist: Some("Beyond".into()),
        };
        assert!(result_matches_intent(&original, &artist_only));
        assert!(result_matches_intent(&japanese, &artist_only));
        assert!(result_matches_intent(&album_only, &artist_only));
        assert!(!result_matches_intent(&cover, &artist_only));
    }

    #[test]
    fn keyword_and_id_search_use_expected_metadata() {
        let album_match = result(Source::Qq, "情人", "BEYOND", Some("海阔天空"), "mid-3");
        assert!(result_matches_intent(
            &album_match,
            &SearchIntent::Keyword("海阔".into())
        ));
        assert!(result_matches_intent(
            &album_match,
            &SearchIntent::Id("mid-3".into())
        ));
        assert!(!result_matches_intent(
            &album_match,
            &SearchIntent::Id("海阔".into())
        ));
    }

    #[test]
    fn id_search_adds_direct_fallback_when_no_exact_result_exists() {
        let mut result = SourceSearchResult {
            source: Source::Qq,
            query: "mid-1".into(),
            results: Vec::new(),
        };
        add_direct_id_fallback(&mut result, &SearchIntent::Id("mid-1".into()));
        assert_eq!(result.results.len(), 1);
        assert_eq!(result.results[0].id, "mid-1");
        assert_eq!(result.results[0].extra["songmid"], "mid-1");

        let numeric_qq = direct_id_result(Source::Qq, "12345");
        assert_eq!(numeric_qq.extra["songid"], 12345);

        let kugou = direct_id_result(Source::Kugou, "197881579");
        assert_eq!(kugou.extra["album_audio_id"], 197881579);
    }

    #[test]
    fn search_limit_defaults_and_clamps() {
        let request = ApiSearchRequest {
            query: Some("song".into()),
            title: None,
            artist: None,
            id: None,
            source: None,
            sources: None,
            top: None,
            limit: None,
            merge_mode: None,
            force: None,
            ttl_seconds: None,
        };
        assert_eq!(request.search_limit(DEFAULT_SOURCE_SEARCH_LIMIT), 100);
        assert_eq!(request.search_limit(DEFAULT_AGGREGATE_SEARCH_LIMIT), 10);

        let request = ApiSearchRequest {
            top: Some(500),
            ..request
        };
        assert_eq!(request.search_limit(DEFAULT_SOURCE_SEARCH_LIMIT), 100);
        assert_eq!(request.search_limit(DEFAULT_AGGREGATE_SEARCH_LIMIT), 10);
    }

    #[test]
    fn aggregate_search_results_group_and_display_sources() {
        let mut qq_results = Vec::new();
        let mut kugou_results = Vec::new();
        for index in 0..12 {
            let title = format!("Song {index}");
            qq_results.push(result(
                Source::Qq,
                &title,
                "Artist",
                None,
                &format!("qq-{index}"),
            ));
            kugou_results.push(result(
                Source::Kugou,
                &title,
                "Artist",
                None,
                &format!("kg-{index}"),
            ));
        }
        let groups = vec![
            SourceSearchResult {
                source: Source::Qq,
                query: "Song".into(),
                results: qq_results,
            },
            SourceSearchResult {
                source: Source::Kugou,
                query: "Song".into(),
                results: kugou_results,
            },
        ];

        let results = aggregate_search_results(&groups, MergeMode::Tracks, 10);

        assert_eq!(results.len(), 10);
        assert_eq!(
            results[0].extra["display_source"].as_str(),
            Some("聚合(kugou+qq)")
        );
        assert_eq!(
            results[0].extra["aggregate_members"]
                .as_array()
                .map(Vec::len),
            Some(2)
        );
        assert!(aggregate_members(&results[0]).unwrap().is_some());
    }

    async fn get_json(
        app: Router,
        uri: &str,
        header: Option<(&str, &str)>,
    ) -> (StatusCode, serde_json::Value) {
        let mut request = Request::builder().method(Method::GET).uri(uri);
        if let Some((name, value)) = header {
            request = request.header(name, value);
        }
        let response = app
            .oneshot(request.body(Body::empty()).unwrap())
            .await
            .unwrap();
        let status = response.status();
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let value = serde_json::from_slice(&body).unwrap();
        (status, value)
    }

    async fn delete_json(
        app: Router,
        uri: &str,
        header: Option<(&str, &str)>,
    ) -> (StatusCode, serde_json::Value) {
        let mut request = Request::builder().method(Method::DELETE).uri(uri);
        if let Some((name, value)) = header {
            request = request.header(name, value);
        }
        let response = app
            .oneshot(request.body(Body::empty()).unwrap())
            .await
            .unwrap();
        let status = response.status();
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let value = serde_json::from_slice(&body).unwrap();
        (status, value)
    }

    fn test_app(cache: UpstreamCache, server_token: Option<&str>) -> Router {
        app(AppState {
            context: ServiceContext {
                cache: Some(cache),
                ..ServiceContext::default()
            },
            server_token: server_token.map(str::to_string),
        })
    }

    fn test_cache(name: &str) -> UpstreamCache {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "rosettrism-{name}-{}-{now}.sqlite",
            std::process::id()
        ));
        UpstreamCache::open(path).unwrap()
    }

    fn result(
        source: Source,
        title: &str,
        artist: &str,
        album: Option<&str>,
        id: &str,
    ) -> SearchResult {
        SearchResult {
            source,
            id: id.into(),
            title: title.into(),
            artist: artist.into(),
            album: album.map(ToOwned::to_owned),
            duration_ms: Some(240_000),
            extra: json!({ "id": id }),
        }
    }
}
