use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use axum::body::to_bytes;
use axum::http::{Method, Request};
use tower::ServiceExt;

use super::*;
use crate::cache::CachePut;
use crate::decoder::InputFormat;
use crate::model::{LyricDocument, LyricLine, LyricMeta};
use crate::provider::{FetchedLyric, LyricProvider};

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
    let cache = test_cache("api_routes_return_json_with_cache_enabled").await;
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
        .await
        .unwrap();
    let run_id = cache
        .start_fetch_run("hello", Some(Source::Lrclib), "fetch")
        .await
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
        .await
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
async fn fetch_result_enrich_false_uses_only_current_provider() {
    let (app, calls) = test_app_with_tracking_provider();
    let (status, body) = post_json(
        app,
        "/api/fetch-result",
        json!({
            "result": result(Source::Qq, "Base Song", "Artist", None, "base-1"),
            "format": "json",
            "enrich": false,
        }),
        None,
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert!(body.get("unified").is_none());

    let calls = calls.lock().unwrap();
    assert_eq!(calls.as_slice(), &["qq:fetch:base-1"]);
}

#[tokio::test]
async fn fetch_result_enrich_true_keeps_cross_source_enrichment() {
    let (app, calls) = test_app_with_tracking_provider();
    let (status, body) = post_json(
        app,
        "/api/fetch-result",
        json!({
            "result": result(Source::Qq, "Base Song", "Artist", None, "base-1"),
            "format": "json",
            "enrich": true,
        }),
        None,
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert!(body["unified"].is_object());
    assert!(body["unified"]["meta"]["source"]
        .as_str()
        .is_some_and(|value| value.starts_with("enriched(")));

    let calls = calls.lock().unwrap();
    assert!(calls.iter().any(|entry| entry != "qq:fetch:base-1"));
    assert!(calls.iter().any(|entry| entry.starts_with("qq:fetch:")));
    assert!(calls.iter().any(|entry| entry.contains(":search:")));
}

#[tokio::test]
async fn server_token_auth_accepts_explicit_and_bearer_headers() {
    let app = test_app(test_cache("server_token_auth").await, Some("secret-token"));

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
    let cache = test_cache("cache_detail_upstream").await;
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
        .await
        .unwrap();

    let (status, detail) = get_json(test_app(cache, None), &format!("/api/cache/{id}"), None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(detail["entry"]["id"], id);
    assert_eq!(detail["entry"]["source"], "qq");
    assert!(detail["unified_entry"].is_null());
}

#[tokio::test]
async fn cache_detail_returns_unified_entry_by_id() {
    let cache = test_cache("cache_detail_unified").await;
    let id = cache
        .put_unified(
            "unified-detail",
            br#"{"metadata":{"title":"Track"},"lines":[]}"#,
            &["upstream-a".to_string()],
            Duration::from_secs(60),
        )
        .await
        .unwrap();

    let (status, detail) = get_json(test_app(cache, None), &format!("/api/cache/{id}"), None).await;
    assert_eq!(status, StatusCode::OK);
    assert!(detail["entry"].is_null());
    assert_eq!(detail["unified_entry"]["id"], id);
    assert_eq!(detail["unified_entry"]["dependencies"][0], "upstream-a");
}

#[tokio::test]
async fn cache_detail_returns_not_found_for_missing_id() {
    let (status, detail) = get_json(
        test_app(test_cache("cache_detail_missing").await, None),
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
    let cache = test_cache("cache_list_split").await;
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
        .await
        .unwrap();
    let unified_id = cache
        .put_unified(
            "unified-list",
            br#"{"metadata":{"title":"Track"},"lines":[]}"#,
            &["upstream-list".to_string(), "upstream-other".to_string()],
            Duration::from_secs(60),
        )
        .await
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
    let cache = test_cache("delete_unified_ai_scores").await;
    let unified_id = cache
        .put_unified(
            "unified-delete",
            br#"{"metadata":{"title":"Track"},"lines":[]}"#,
            &[],
            Duration::from_secs(60),
        )
        .await
        .unwrap();
    cache
        .put_ai_score(unified_id, &json!({ "candidate_summary_hash": "abc123" }))
        .await
        .unwrap();
    assert_eq!(cache.stats().await.unwrap().ai_score_entries, 1);

    let app = test_app(cache.clone(), None);
    let (status, deleted) = delete_json(
        app.clone(),
        &format!("/api/unified-cache/{unified_id}"),
        None,
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(deleted["deleted"], true);
    assert!(cache.unified_detail(unified_id).await.unwrap().is_none());
    assert_eq!(cache.list_ai_scores(unified_id).await.unwrap().len(), 0);
    assert_eq!(cache.stats().await.unwrap().ai_score_entries, 0);

    let (detail_status, detail) =
        get_json(app, &format!("/api/unified-cache/{unified_id}"), None).await;
    assert_eq!(detail_status, StatusCode::NOT_FOUND);
    assert_eq!(detail["code"], "no_lyrics_found");
    assert_eq!(detail["message"], "unified cache entry not found");
}

#[tokio::test]
async fn cache_revalidate_deletes_upstream_entry() {
    let cache = test_cache("cache_revalidate_upstream").await;
    let id = cache
        .put(CachePut {
            key: "revalidate-upstream",
            source: Source::Lrclib,
            operation: "fetch",
            status_code: 200,
            body: br#"{"lyrics":"hello"}"#,
            metadata: &json!({ "query": "hello" }),
            ttl: Duration::from_secs(60),
        })
        .await
        .unwrap();
    let app = test_app(cache.clone(), None);

    let (status, body) = post_json(
        app.clone(),
        &format!("/api/cache/{id}/revalidate"),
        json!({}),
        None,
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["revalidate"], "deleted_cached_entry");
    assert_eq!(body["deleted"], true);
    assert!(cache.detail(id).await.unwrap().is_none());
}

#[tokio::test]
async fn unified_cache_revalidate_deletes_unified_entry() {
    let cache = test_cache("unified_cache_revalidate").await;
    let id = cache
        .put_unified(
            "revalidate-unified",
            br#"{"metadata":{"title":"Track"},"lines":[]}"#,
            &[],
            Duration::from_secs(60),
        )
        .await
        .unwrap();
    let app = test_app(cache.clone(), None);

    let (status, body) = post_json(
        app.clone(),
        &format!("/api/unified-cache/{id}/revalidate"),
        json!({}),
        None,
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["revalidate"], "deleted_unified_cached_entry");
    assert_eq!(body["deleted"], true);
    assert!(cache.unified_detail(id).await.unwrap().is_none());
}

#[tokio::test]
async fn cache_delete_removes_upstream_entry_and_reports_404_after() {
    let cache = test_cache("cache_delete_upstream").await;
    let id = cache
        .put(CachePut {
            key: "delete-upstream",
            source: Source::Qq,
            operation: "search",
            status_code: 200,
            body: br#"{"results":[]}"#,
            metadata: &json!({ "query": "track" }),
            ttl: Duration::from_secs(60),
        })
        .await
        .unwrap();
    let app = test_app(cache.clone(), None);

    let (status, deleted) = delete_json(app.clone(), &format!("/api/cache/{id}"), None).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(deleted["deleted"], true);
    assert!(cache.detail(id).await.unwrap().is_none());

    // 二次删除同一 id 时 deleted=false，但路由仍返回 OK（幂等）
    let (second_status, second_deleted) = delete_json(app, &format!("/api/cache/{id}"), None).await;
    assert_eq!(second_status, StatusCode::OK);
    assert_eq!(second_deleted["deleted"], false);
}

#[tokio::test]
async fn ai_replay_returns_service_error_when_ai_scoring_not_configured() {
    let cache = test_cache("ai_replay_unconfigured").await;
    let unified_id = cache
        .put_unified(
            "ai-replay-unconfigured",
            br#"{"metadata":{"title":"Track"},"lines":[]}"#,
            &[],
            Duration::from_secs(60),
        )
        .await
        .unwrap();
    let app = test_app(cache, None);

    let (status, body) = post_json(
        app,
        "/api/ai/replay",
        json!({ "unified_cache_id": unified_id }),
        None,
    )
    .await;

    // 未配置 AI scoring 时返回错误，而非崩溃或空结果
    assert_ne!(status, StatusCode::OK);
    assert!(!body["code"].as_str().unwrap().is_empty());
    assert!(!body["message"].as_str().unwrap().is_empty());
}

#[test]
fn rejects_non_local_host_without_token() {
    std::env::remove_var("ROSETTRISM_SERVER_TOKEN");
    assert!(!is_local_host("0.0.0.0"));
}

#[test]
fn dashboard_fallback_is_limited_to_spa_routes() {
    assert!(should_fallback_to_index(""));
    assert!(should_fallback_to_index("fetch"));
    assert!(should_fallback_to_index("cache/123"));
    assert!(!should_fallback_to_index("assets/missing.js"));
    assert!(!should_fallback_to_index("favicon.ico"));
}

#[test]
fn missing_dashboard_asset_returns_not_found() {
    let response = asset_response("assets/definitely-missing.js");
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
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
fn keyword_search_keeps_direct_url_and_id_results() {
    let utaten = SearchResult {
        source: Source::Utaten,
        id: "ya17060751".into(),
        title: "UtaTen ya17060751".into(),
        artist: String::new(),
        album: None,
        duration_ms: None,
        extra: json!({
            "lyrics_id": "ya17060751",
            "url": "https://utaten.com/lyric/ya17060751/"
        }),
    };
    assert!(result_matches_intent(
        &utaten,
        &SearchIntent::Keyword("https://utaten.com/lyric/ya17060751/".into())
    ));

    let spotify = SearchResult {
        source: Source::SpotifyLyrics,
        id: "ABCDEFGHIJKLMNOPQRSTUV".into(),
        title: "Spotify track ABCDEFGHIJKLMNOPQRSTUV".into(),
        artist: String::new(),
        album: None,
        duration_ms: None,
        extra: json!({
            "spotify_track_id": "ABCDEFGHIJKLMNOPQRSTUV"
        }),
    };
    assert!(result_matches_intent(
        &spotify,
        &SearchIntent::Keyword(
            "https://open.spotify.com/track/ABCDEFGHIJKLMNOPQRSTUV?si=abc".into()
        )
    ));

    let web_source = SearchResult {
        source: Source::UtaNet,
        id: "abc123".into(),
        title: "Uta-Net abc123".into(),
        artist: String::new(),
        album: None,
        duration_ms: None,
        extra: json!({
            "url": "https://example.test/showkasi.php?surl=abc123"
        }),
    };
    assert!(result_matches_intent(
        &web_source,
        &SearchIntent::Keyword("id:abc123".into())
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

async fn post_json(
    app: Router,
    uri: &str,
    body: serde_json::Value,
    header: Option<(&str, &str)>,
) -> (StatusCode, serde_json::Value) {
    let mut request = Request::builder()
        .method(Method::POST)
        .uri(uri)
        .header(axum::http::header::CONTENT_TYPE, "application/json");
    if let Some((name, value)) = header {
        request = request.header(name, value);
    }
    let response = app
        .oneshot(request.body(Body::from(body.to_string())).unwrap())
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

fn test_app_with_tracking_provider() -> (Router, Arc<Mutex<Vec<String>>>) {
    let calls = Arc::new(Mutex::new(Vec::new()));
    let provider_calls = calls.clone();
    let context = ServiceContext {
        provider_factory: Some(Arc::new(move |source, _ttl, _force| {
            Ok(Box::new(TrackingProvider {
                source,
                calls: provider_calls.clone(),
            }) as Box<dyn LyricProvider>)
        })),
        ..ServiceContext::default()
    };
    (
        app(AppState {
            context,
            server_token: None,
        }),
        calls,
    )
}

async fn test_cache(name: &str) -> UpstreamCache {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "rosettrism-{name}-{}-{now}.sqlite",
        std::process::id()
    ));
    UpstreamCache::open(path).await.unwrap()
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

#[derive(Clone)]
struct TrackingProvider {
    source: Source,
    calls: Arc<Mutex<Vec<String>>>,
}

#[async_trait]
impl LyricProvider for TrackingProvider {
    async fn search(&self, query: &str) -> Result<Vec<SearchResult>> {
        self.calls
            .lock()
            .unwrap()
            .push(format!("{}:search:{query}", self.source.cli_name()));
        Ok(vec![SearchResult {
            source: self.source,
            id: format!("{}-candidate", self.source.cli_name()),
            title: format!("{} title", self.source.display_name()),
            artist: "Artist".into(),
            album: None,
            duration_ms: Some(180_000),
            extra: json!({ "source": self.source.cli_name() }),
        }])
    }

    async fn fetch(&self, result: &SearchResult) -> Result<FetchedLyric> {
        self.calls
            .lock()
            .unwrap()
            .push(format!("{}:fetch:{}", self.source.cli_name(), result.id));
        Ok(FetchedLyric {
            input_format: InputFormat::Lrc,
            raw: b"[00:00.00]Tracked\n".to_vec(),
            document: Some(LyricDocument {
                meta: LyricMeta {
                    title: Some(format!("{} title", self.source.display_name())),
                    artist: Some("Artist".into()),
                    ..Default::default()
                },
                lines: vec![LyricLine {
                    start_ms: 0,
                    duration_ms: None,
                    text: format!("{} lyric", self.source.cli_name()),
                    words: Vec::new(),
                    ruby: Vec::new(),
                    translation: None,
                    reading: None,
                    romanized: None,
                }],
            }),
            annotations: Vec::new(),
        })
    }
}
