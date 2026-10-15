use super::*;

pub(super) async fn health(
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
pub(super) struct ProviderHealthQuery {
    pub(super) limit: Option<usize>,
}

pub(super) async fn providers_health(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<ProviderHealthQuery>,
) -> ApiResult<Json<serde_json::Value>> {
    authorize(&state, &headers)?;
    let cache = require_cache(&state)?;
    let limit = query.limit.unwrap_or(20).clamp(1, 500);
    Ok(Json(json!({
        "sample_size": limit,
        "providers": enrich_provider_health(cache.provider_health(limit).await?),
    })))
}

pub(super) async fn sources(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> ApiResult<Json<serde_json::Value>> {
    authorize(&state, &headers)?;
    Ok(Json(json!({
        "manifest_file": crate::provider::ProviderManifest::manifest_file_name(),
        "sources": builtin_provider_registry(),
    })))
}

pub(super) async fn fetch(
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
            .fetch_source_specific(
                source,
                &request.query,
                format,
                top.unwrap_or(1),
                ttl,
                force,
                request.enrich,
            )
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

pub(super) async fn search(
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

pub(super) async fn fetch_result(
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
        .fetch_source_result(request.result, request.format, ttl, force, request.enrich)
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
pub(super) struct AiReplayRequest {
    pub(super) unified_cache_id: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) ai_scoring: Option<AiScoringConfig>,
}

pub(super) async fn ai_replay(
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

pub(super) async fn cache_list(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> ApiResult<Json<serde_json::Value>> {
    authorize(&state, &headers)?;
    let cache = require_cache(&state)?;
    let upstream_entries = cache.list(100).await?;
    let unified_entries = cache.list_unified(100).await?;
    Ok(Json(json!({
        "upstream_entries": upstream_entries,
        "unified_entries": unified_entries,
        "entries": upstream_entries
    })))
}

pub(super) async fn cache_detail(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> ApiResult<Json<serde_json::Value>> {
    authorize(&state, &headers)?;
    let cache = require_cache(&state)?;
    let entry = cache.detail(id).await?;
    let unified_entry = cache.unified_detail(id).await?;
    if entry.is_none() && unified_entry.is_none() {
        return Err(ApiError::not_found("cache entry not found"));
    }
    let ai_scores = match unified_entry.as_ref() {
        Some(entry) => cache.list_ai_scores(entry.id).await?,
        None => Vec::new(),
    };
    Ok(Json(json!({
        "entry": entry,
        "unified_entry": unified_entry,
        "ai_scores": ai_scores
    })))
}

pub(super) async fn unified_cache_detail(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> ApiResult<Json<serde_json::Value>> {
    authorize(&state, &headers)?;
    let cache = require_cache(&state)?;
    let unified_entry = cache.unified_detail(id).await?;
    if unified_entry.is_none() {
        return Err(ApiError::not_found("unified cache entry not found"));
    }
    Ok(Json(json!({
        "entry": null,
        "unified_entry": unified_entry,
        "ai_scores": cache.list_ai_scores(id).await?
    })))
}

pub(super) async fn cache_delete(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> ApiResult<Json<serde_json::Value>> {
    authorize(&state, &headers)?;
    let cache = require_cache(&state)?;
    let deleted = cache.delete(id).await?;
    Ok(Json(json!({ "deleted": deleted })))
}

pub(super) async fn cache_revalidate(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> ApiResult<Json<serde_json::Value>> {
    authorize(&state, &headers)?;
    let cache = require_cache(&state)?;
    let deleted = cache.delete(id).await?;
    Ok(Json(json!({
        "revalidate": "deleted_cached_entry",
        "deleted": deleted
    })))
}

pub(super) async fn unified_cache_delete(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> ApiResult<Json<serde_json::Value>> {
    authorize(&state, &headers)?;
    let cache = require_cache(&state)?;
    let deleted = cache.delete_unified(id).await?;
    Ok(Json(json!({ "deleted": deleted })))
}

pub(super) async fn unified_cache_revalidate(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> ApiResult<Json<serde_json::Value>> {
    authorize(&state, &headers)?;
    let cache = require_cache(&state)?;
    let deleted = cache.delete_unified(id).await?;
    Ok(Json(json!({
        "revalidate": "deleted_unified_cached_entry",
        "deleted": deleted
    })))
}

pub(super) async fn fetch_runs(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> ApiResult<Json<serde_json::Value>> {
    authorize(&state, &headers)?;
    let cache = require_cache(&state)?;
    Ok(Json(json!({
        "runs": cache.list_fetch_runs(100).await?,
        "status_counts": cache.fetch_run_status_counts().await?
    })))
}

#[derive(Debug, Deserialize)]
pub(super) struct StatsQuery {
    pub(super) query_hash: Option<String>,
    pub(super) model: Option<String>,
    pub(super) prompt_version: Option<String>,
    pub(super) created_at_start: Option<i64>,
    pub(super) created_at_end: Option<i64>,
}

pub(super) async fn stats(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<StatsQuery>,
) -> ApiResult<Json<serde_json::Value>> {
    authorize(&state, &headers)?;
    let cache = require_cache(&state)?;
    Ok(Json(json!({
        "cache": cache.stats().await?,
        "ai_scores": cache.query_ai_scores(AiScoreQuery {
            query_hash: query.query_hash,
            model: query.model,
            prompt_version: query.prompt_version,
            created_at_start: query.created_at_start,
            created_at_end: query.created_at_end,
            limit: 20,
        })
        .await?,
        "fetch_runs": cache.list_fetch_runs(20).await?,
        "fetch_run_status_counts": cache.fetch_run_status_counts().await?,
        "provider_health": enrich_provider_health(cache.provider_health(20).await?)
    })))
}

pub(super) fn enrich_provider_health(
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

#[derive(Debug, Deserialize)]
pub(super) struct ApiFetchResultRequest {
    pub(super) result: SearchResult,
    pub(super) format: SpecificFetchFormat,
    #[serde(default)]
    pub(super) force: Option<bool>,
    #[serde(default)]
    pub(super) enrich: bool,
    #[serde(default)]
    pub(super) ttl_seconds: Option<u64>,
    #[serde(default)]
    pub(super) ai_scoring: Option<AiScoringConfig>,
}
