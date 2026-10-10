use super::*;
use crate::model::{LyricLine, LyricMeta, LyricRubySpan, LyricTrackQuality};
use serde_json::json;

#[test]
fn golden_ai_score_fixture_tracks_quality_changes() {
    let fixture: serde_json::Value = serde_json::from_str(include_str!(
        "../../tests/fixtures/ai_score_history/candidate_regression.json"
    ))
    .unwrap();
    let summaries: Vec<AiCandidateSummary> =
        serde_json::from_value(fixture["candidates"].clone()).unwrap();
    let expected = &fixture["expected"];
    let heuristic_best_index = summaries
        .iter()
        .max_by(|left, right| {
            left.heuristic_score
                .partial_cmp(&right.heuristic_score)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .map(|summary| summary.index)
        .unwrap();
    assert_eq!(
        heuristic_best_index,
        expected["heuristic_best_index"].as_u64().unwrap() as usize
    );

    let config = ResolvedAiScoringConfig {
        base_url: "https://example.invalid/v1".into(),
        api_key: "fixture-secret".into(),
        model: "fixture-model".into(),
    };
    let summary_hash = hash_json(serde_json::to_string(&summaries).unwrap().as_bytes());
    let v1 = parse_ai_candidate_selection(
        &fixture["ai_response_v1"].to_string(),
        &config,
        &summaries,
        summary_hash.clone(),
    )
    .unwrap()
    .unwrap();
    let v2 = parse_ai_candidate_selection(
        &fixture["ai_response_v2"].to_string(),
        &config,
        &summaries,
        summary_hash,
    )
    .unwrap()
    .unwrap();

    assert_eq!(
        v1.best_index,
        expected["ai_best_index_v1"].as_u64().unwrap() as usize
    );
    assert_eq!(
        v2.best_index,
        expected["ai_best_index_v2"].as_u64().unwrap() as usize
    );
    assert_eq!(v1.prompt_version, AI_PROMPT_VERSION);
    assert_eq!(v1.candidate_summary_version, AI_CANDIDATE_SUMMARY_VERSION);
    assert_ne!(v1.best_index, v2.best_index);
}

#[test]
fn parses_ttl_units() {
    assert_eq!(parse_ttl("7d").unwrap(), Duration::from_secs(604_800));
    assert_eq!(parse_ttl("2h").unwrap(), Duration::from_secs(7_200));
    assert_eq!(parse_ttl("30").unwrap(), Duration::from_secs(30));
}

#[test]
fn inline_merge_keeps_base_text_and_adds_enrichment() {
    let base = SourceCandidate {
        source: Source::Lrclib,
        result: SearchResult {
            source: Source::Lrclib,
            id: "1".into(),
            title: "Song".into(),
            artist: "Artist".into(),
            album: None,
            duration_ms: None,
            extra: json!({}),
        },
        document: LyricDocument {
            meta: LyricMeta::default(),
            lines: vec![LyricLine {
                start_ms: 1_000,
                duration_ms: Some(1_000),
                text: "歌".into(),
                words: Vec::new(),
                ruby: Vec::new(),
                translation: None,
                reading: None,
                romanized: None,
            }],
        },
        quality: LyricTrackQuality::default(),
        annotations: Vec::new(),
    };
    let ruby = SourceCandidate {
        source: Source::Utaten,
        result: SearchResult {
            source: Source::Utaten,
            id: "ruby".into(),
            title: "Song".into(),
            artist: "Artist".into(),
            album: None,
            duration_ms: None,
            extra: json!({}),
        },
        document: LyricDocument {
            lines: vec![LyricLine {
                start_ms: 1_020,
                text: "歌".into(),
                ruby: vec![LyricRubySpan {
                    start_char: 0,
                    end_char: 1,
                    text: "歌".into(),
                    reading: "うた".into(),
                }],
                ..Default::default()
            }],
            ..Default::default()
        },
        ..base.clone()
    };

    let unified = build_unified(&base, &[base.clone(), ruby], MergeMode::Inline).unwrap();
    assert_eq!(unified.inline_lines[0].text, "歌");
    assert_eq!(unified.inline_lines[0].ruby[0].reading, "うた");
}

#[test]
fn unified_cache_refs_use_fetch_cache_keys() {
    let candidate = SourceCandidate {
        source: Source::Lrclib,
        result: SearchResult {
            source: Source::Lrclib,
            id: "1".into(),
            title: "Song".into(),
            artist: "Artist".into(),
            album: None,
            duration_ms: None,
            extra: json!({ "lrclib_id": 1 }),
        },
        document: LyricDocument {
            lines: vec![LyricLine {
                start_ms: 1_000,
                text: "Hi".into(),
                ..Default::default()
            }],
            ..Default::default()
        },
        quality: LyricTrackQuality::default(),
        annotations: Vec::new(),
    };
    let expected = fetch_cache_key(Source::Lrclib, &candidate.result).unwrap();

    let unified = build_unified(&candidate, &[candidate.clone()], MergeMode::Tracks).unwrap();

    assert_eq!(unified.cache_refs, vec![expected]);
    assert_ne!(unified.cache_refs[0], "lrclib:1:lrc");
}

#[test]
fn source_specific_qrc_raw_returns_decoded_lyric_content() {
    use base64::Engine;

    let wrapped = base64::engine::general_purpose::STANDARD.encode("[1000,500](1000,500,0)Hi\n");
    let xml = format!(
        r#"<?xml version="1.0"?><QrcInfos><LyricInfo><Lyric_1 LyricContent="{wrapped}"/></LyricInfo></QrcInfos>"#
    );
    let raw = decode_source_raw_bytes(xml.as_bytes(), crate::decoder::InputFormat::Qrc).unwrap();

    assert_eq!(
        String::from_utf8(raw).unwrap(),
        "[1000,500](1000,500,0)Hi\n"
    );
}

#[tokio::test]
async fn source_specific_search_returns_all_candidates_when_top_is_omitted() {
    let path = service_offline_db_fixture();
    let context = offline_service_context(path.clone());

    let response = context
        .search_source_specific(
            Source::OfflineDb,
            "Artist",
            Some(Duration::from_secs(60)),
            false,
        )
        .await
        .unwrap();

    assert_eq!(response.results.len(), 2);
    assert_eq!(response.results[0].title, "First Song");
    assert_eq!(response.results[1].title, "Second Song");

    let _ = std::fs::remove_file(path);
}

#[tokio::test]
async fn source_specific_top_fetches_multiple_candidates() {
    let path = service_offline_db_fixture();
    let context = offline_service_context(path.clone());

    let result = context
        .fetch_source_specific(
            Source::OfflineDb,
            "Artist",
            SpecificFetchFormat::Raw,
            2,
            Some(Duration::from_secs(60)),
            false,
            false,
        )
        .await
        .unwrap();

    match result {
        SpecificFetchResult::RawMany {
            results, warnings, ..
        } => {
            assert!(warnings.is_empty());
            assert_eq!(results.len(), 2);
            assert!(String::from_utf8(results[0].raw.clone())
                .unwrap()
                .contains("First"));
            assert!(String::from_utf8(results[1].raw.clone())
                .unwrap()
                .contains("Second"));
        }
        _ => panic!("expected RawMany"),
    }

    let _ = std::fs::remove_file(path);
}

#[tokio::test]
async fn multi_source_search_and_fetch_result_use_selected_candidate() {
    let path = service_offline_db_fixture();
    let context = offline_service_context(path.clone());

    let response = context
        .search_sources(SourceSearchRequest {
            query: "Artist".into(),
            sources: Some(vec![Source::OfflineDb]),
            limit: 10,
            force: false,
            ttl_seconds: Some(60),
        })
        .await
        .unwrap();
    assert_eq!(response.results.len(), 2);

    let second = response.results[1].clone();
    let fetched = context
        .fetch_source_result(
            second,
            SpecificFetchFormat::Raw,
            Some(Duration::from_secs(60)),
            false,
            false,
        )
        .await
        .unwrap();
    match fetched {
        SpecificFetchResult::Raw { raw, result, .. } => {
            assert_eq!(result.id, "two");
            assert!(String::from_utf8(raw).unwrap().contains("Second"));
        }
        _ => panic!("expected Raw"),
    }

    let _ = std::fs::remove_file(path);
}

fn offline_service_context(path: std::path::PathBuf) -> ServiceContext {
    ServiceContext {
        offline_db: Some(path),
        provider_options: ProviderOptions {
            allow_experimental: true,
        },
        ..Default::default()
    }
}

fn service_offline_db_fixture() -> std::path::PathBuf {
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "rosettrism-service-offline-{unique}-{}.sqlite",
        std::process::id()
    ));
    let connection = rusqlite::Connection::open(&path).unwrap();
    connection
        .execute_batch(
            r#"
                CREATE TABLE lyrics (
                    id TEXT PRIMARY KEY,
                    title TEXT NOT NULL,
                    artist TEXT NOT NULL,
                    album TEXT,
                    duration_ms INTEGER,
                    source TEXT,
                    format TEXT,
                    text TEXT NOT NULL,
                    reading TEXT,
                    romanized TEXT,
                    metadata_json TEXT
                );
                "#,
        )
        .unwrap();
    connection
        .execute(
            "INSERT INTO lyrics VALUES (?1, ?2, ?3, NULL, ?4, ?5, ?6, ?7, NULL, NULL, NULL)",
            rusqlite::params![
                "one",
                "First Song",
                "Artist",
                120_000_i64,
                "fixture",
                "lrc",
                "[00:01.00]First\n"
            ],
        )
        .unwrap();
    connection
        .execute(
            "INSERT INTO lyrics VALUES (?1, ?2, ?3, NULL, ?4, ?5, ?6, ?7, NULL, NULL, NULL)",
            rusqlite::params![
                "two",
                "Second Song",
                "Artist",
                130_000_i64,
                "fixture",
                "lrc",
                "[00:01.00]Second\n"
            ],
        )
        .unwrap();
    drop(connection);
    path
}
