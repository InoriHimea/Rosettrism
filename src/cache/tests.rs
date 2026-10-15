use super::*;

#[tokio::test]
async fn list_and_detail_include_display_fields_and_preview() {
    let path = std::env::temp_dir().join(format!(
        "rosettrism-cache-display-{}-{}.sqlite",
        now_unix(),
        std::process::id()
    ));
    let cache = UpstreamCache::open(&path).await.unwrap();
    let result = SearchResult {
        source: Source::Lrclib,
        id: "abc-123".into(),
        title: "Song".into(),
        artist: "Artist".into(),
        album: None,
        duration_ms: Some(180_000),
        extra: json!({}),
    };
    let body = serde_json::to_vec(&vec![result.clone()]).unwrap();
    let metadata = json!({
        "payload": "search_results",
        "query": "Song Artist",
        "first_result": result,
    });

    cache
        .put(CachePut {
            key: "display-test",
            source: Source::Lrclib,
            operation: "search",
            status_code: 200,
            body: &body,
            metadata: &metadata,
            ttl: Duration::from_secs(60),
        })
        .await
        .unwrap();

    let entries = cache.list(10).await.unwrap();
    let entry = entries
        .iter()
        .find(|entry| entry.cache_key == "display-test")
        .unwrap();
    assert_eq!(entry.query.as_deref(), Some("Song Artist"));
    assert_eq!(entry.item_id.as_deref(), Some("abc-123"));
    assert_eq!(entry.title.as_deref(), Some("Song"));
    assert_eq!(entry.artist.as_deref(), Some("Artist"));

    let detail = cache.detail(entry.id).await.unwrap().unwrap();
    assert_eq!(detail.query.as_deref(), Some("Song Artist"));
    assert_eq!(detail.item_id.as_deref(), Some("abc-123"));
    assert!(detail.body_text_preview.contains("Song"));

    drop(cache);
    let _ = std::fs::remove_file(path);
}

#[tokio::test]
async fn deleting_upstream_entry_invalidates_dependent_unified_cache() {
    let path = std::env::temp_dir().join(format!(
        "rosettrism-cache-dependency-{}-{}.sqlite",
        now_unix(),
        std::process::id()
    ));
    let cache = UpstreamCache::open(&path).await.unwrap();
    cache
        .put(CachePut {
            key: "upstream-key",
            source: Source::Lrclib,
            operation: "fetch",
            status_code: 200,
            body: b"[00:01.00]Hi\n",
            metadata: &json!({ "item_id": "abc" }),
            ttl: Duration::from_secs(60),
        })
        .await
        .unwrap();
    cache
        .put_unified(
            "unified-key",
            br#"{"results":[],"warnings":[]}"#,
            &["upstream-key".to_string()],
            Duration::from_secs(60),
        )
        .await
        .unwrap();

    let entry = cache
        .list(10)
        .await
        .unwrap()
        .into_iter()
        .find(|entry| entry.cache_key == "upstream-key")
        .unwrap();
    assert!(cache
        .get_unified_fresh("unified-key")
        .await
        .unwrap()
        .is_some());
    assert!(cache.delete(entry.id).await.unwrap());
    assert!(cache
        .get_unified_fresh("unified-key")
        .await
        .unwrap()
        .is_none());
    assert!(!cache.delete(entry.id).await.unwrap());

    drop(cache);
    let _ = std::fs::remove_file(path);
}

#[tokio::test]
async fn records_and_counts_fetch_runs() {
    let path = std::env::temp_dir().join(format!(
        "rosettrism-fetch-runs-{}-{}.sqlite",
        now_unix(),
        std::process::id()
    ));
    let cache = UpstreamCache::open(&path).await.unwrap();

    let first_id = cache
        .start_fetch_run("Song Artist", Some(Source::Lrclib), "fetch_source_result")
        .await
        .unwrap();
    cache
        .finish_fetch_run(
            first_id,
            "cache_hit",
            Some("served_unified_cache: id=42"),
            FetchRunMetadata {
                provider_count: Some(1),
                candidate_count: Some(1),
                cache_event: Some("cache_hit".into()),
            },
        )
        .await
        .unwrap();
    let second_id = cache
        .start_fetch_run("Other Song", None, "aggregate_fetch")
        .await
        .unwrap();
    cache
        .finish_fetch_run(
            second_id,
            "no_lyrics_found",
            Some("no_lyrics_found: all selected sources returned no usable lyric"),
            FetchRunMetadata {
                provider_count: Some(2),
                candidate_count: Some(0),
                cache_event: None,
            },
        )
        .await
        .unwrap();

    let runs = cache.list_fetch_runs(10).await.unwrap();
    assert_eq!(runs.len(), 2);
    assert_eq!(runs[0].id, second_id);
    assert_eq!(runs[0].status, "no_lyrics_found");
    assert_eq!(runs[1].source.as_deref(), Some("lrclib"));
    assert_eq!(
        runs[1].message.as_deref(),
        Some("served_unified_cache: id=42")
    );

    let counts = cache.fetch_run_status_counts().await.unwrap();
    assert!(counts
        .iter()
        .any(|count| count.status == "cache_hit" && count.count == 1));
    assert!(counts
        .iter()
        .any(|count| count.status == "no_lyrics_found" && count.count == 1));
    assert_eq!(cache.stats().await.unwrap().fetch_run_entries, 2);

    drop(cache);
    let _ = std::fs::remove_file(path);
}

#[tokio::test]
async fn stores_and_lists_ai_scores_for_unified_cache() {
    let path = std::env::temp_dir().join(format!(
        "rosettrism-ai-score-{}-{}.sqlite",
        now_unix(),
        std::process::id()
    ));
    let cache = UpstreamCache::open(&path).await.unwrap();
    let unified_id = cache
        .put_unified(
            "unified-ai-score-test",
            br#"{"mode":"tracks","results":[]}"#,
            &[],
            Duration::from_secs(60),
        )
        .await
        .unwrap();
    let score = json!({
        "model": "gpt-4o-mini",
        "base_url": "https://api.openai.com/v1",
        "prompt_version": "ai-score-prompt-v1",
        "candidate_summary_hash": "abc123",
        "request_payload": { "api_key": "secret-key", "messages": ["safe"] },
        "best_index": 0,
        "scores": [{"index": 0, "source": "qq", "heuristic_score": 90.0, "ai_score": 95.0, "reason": "best timing"}],
        "reason": "best timing",
        "created_at": now_unix()
    });

    cache.put_ai_score(unified_id, &score).await.unwrap();

    let scores = cache.list_ai_scores(unified_id).await.unwrap();
    assert_eq!(scores.len(), 1);
    assert_eq!(scores[0].unified_cache_id, unified_id);
    assert_eq!(scores[0].score_json["candidate_summary_hash"], "abc123");
    assert_eq!(
        scores[0].score_json["request_payload"]["api_key"],
        "[REDACTED]"
    );
    assert!(!serde_json::to_string(&scores[0].score_json)
        .unwrap()
        .contains("secret-key"));

    let queried = cache
        .query_ai_scores(AiScoreQuery {
            query_hash: Some("abc123".into()),
            model: Some("gpt-4o-mini".into()),
            prompt_version: Some("ai-score-prompt-v1".into()),
            created_at_start: Some(0),
            created_at_end: Some(now_unix() + 60),
            limit: 10,
        })
        .await
        .unwrap();
    assert_eq!(queried.len(), 1);
    assert_eq!(cache.stats().await.unwrap().ai_score_entries, 1);

    drop(cache);
    let _ = std::fs::remove_file(path);
}

#[tokio::test]
async fn migrates_legacy_ai_scores_before_creating_query_index() {
    let path = std::env::temp_dir().join(format!(
        "rosettrism-ai-score-legacy-{}-{}.sqlite",
        now_unix(),
        std::process::id()
    ));
    {
        let connection = Connection::open(&path).unwrap();
        connection
            .execute_batch(
                r#"
                CREATE TABLE unified_cache (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    cache_key TEXT NOT NULL UNIQUE,
                    body BLOB NOT NULL,
                    body_hash TEXT NOT NULL,
                    dependencies_json TEXT NOT NULL,
                    created_at INTEGER NOT NULL,
                    expires_at INTEGER NOT NULL,
                    request_version TEXT NOT NULL
                );
                CREATE TABLE ai_scores (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    unified_cache_id INTEGER NOT NULL,
                    score_json TEXT NOT NULL,
                    created_at INTEGER NOT NULL
                );
                INSERT INTO unified_cache (
                    id, cache_key, body, body_hash, dependencies_json,
                    created_at, expires_at, request_version
                ) VALUES (
                    1, 'legacy-unified', x'7B7D', 'legacy-hash', '[]',
                    1, 9999999999, 'unified-v1'
                );
                INSERT INTO ai_scores (
                    unified_cache_id, score_json, created_at
                ) VALUES (
                    1,
                    '{"candidate_summary_hash":"legacy-hash","model":"legacy-model","prompt_version":"legacy-prompt"}',
                    2
                );
                "#,
            )
            .unwrap();
    }

    let cache = UpstreamCache::open(&path).await.unwrap();
    let scores = cache
        .query_ai_scores(AiScoreQuery {
            query_hash: Some("legacy-hash".into()),
            model: Some("legacy-model".into()),
            prompt_version: Some("legacy-prompt".into()),
            limit: 10,
            ..AiScoreQuery::default()
        })
        .await
        .unwrap();
    assert_eq!(scores.len(), 1);

    drop(cache);
    let _ = std::fs::remove_file(path);
}
