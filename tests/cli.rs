use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn decode_missing_file_fails() {
    let mut cmd = Command::cargo_bin("rosettrism").unwrap();
    cmd.args(["decode", "missing.krc"]);
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("failed to read"));
}

#[test]
fn help_mentions_full_name_and_command_aliases() {
    let mut cmd = Command::cargo_bin("rosettrism").unwrap();
    cmd.arg("--help");
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("Rosettrism"))
        .stdout(predicate::str::contains("rosettrism"))
        .stdout(predicate::str::contains("rstm"))
        .stdout(predicate::str::contains("rosm"));
}

#[test]
fn decode_raw_defaults_to_decoded_file_next_to_input() {
    let dir = std::env::temp_dir().join(format!(
        "rosettrism-cli-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&dir).unwrap();

    let input = dir.join("sample.qrc");
    let output = dir.join("sample_decoded.xml");
    fs::write(
        &input,
        r#"<?xml version="1.0"?><QrcInfos><LyricInfo><Lyric_1 LyricContent="[1000,500](1000,500,0)Hi"/></LyricInfo></QrcInfos>"#,
    )
    .unwrap();

    let mut cmd = Command::cargo_bin("rosettrism").unwrap();
    cmd.args(["decode"])
        .arg(&input)
        .args(["--input-format", "qrc", "-f", "raw"]);
    cmd.assert().success().stdout(predicate::str::is_empty());

    let rendered = fs::read_to_string(&output).unwrap();
    assert!(rendered.contains("LyricContent"));

    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn decode_ignores_cookie_file() {
    let dir = std::env::temp_dir().join(format!(
        "rosettrism-cli-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&dir).unwrap();

    let output = dir.join("sample.lrc");
    let missing_cookie = dir.join("missing.cookie");

    let mut cmd = Command::cargo_bin("rosettrism").unwrap();
    cmd.arg("--cookie-file")
        .arg(&missing_cookie)
        .arg("decode")
        .arg(PathBuf::from("tests").join("fixtures").join("sample.qrc"))
        .arg("-o")
        .arg(&output);
    cmd.assert().success();

    let rendered = fs::read_to_string(&output).unwrap();
    assert!(rendered.contains("[00:01.00]"));

    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn decode_apple_music_ttml_with_auto_detection() {
    let dir = std::env::temp_dir().join(format!(
        "rosettrism-cli-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&dir).unwrap();

    let input = dir.join("apple.ttml");
    let output = dir.join("apple.lrc");
    fs::write(
        &input,
        r#"<?xml version="1.0" encoding="UTF-8"?>
<tt xmlns="http://www.w3.org/ns/ttml">
  <body>
    <div>
      <p begin="00:01.000" end="00:02.000"><span begin="00:01.000" end="00:01.400">Hi</span> <span begin="00:01.400" end="00:02.000">there</span></p>
    </div>
  </body>
</tt>"#,
    )
    .unwrap();

    let mut cmd = Command::cargo_bin("rosettrism").unwrap();
    cmd.args(["decode"])
        .arg(&input)
        .args(["-f", "lrc", "-o"])
        .arg(&output);
    cmd.assert().success();

    let rendered = fs::read_to_string(&output).unwrap();
    assert!(rendered.contains("[00:01.00]Hi there"));

    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn fetch_source_requires_explicit_format() {
    let mut cmd = Command::cargo_bin("rosettrism").unwrap();
    cmd.args(["fetch", "Song Artist", "--source", "qq", "--top", "1"]);
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("source-specific fetch requires"));
}

#[test]
fn fetch_help_mentions_aggregate_options() {
    let mut cmd = Command::cargo_bin("rosettrism").unwrap();
    cmd.args(["fetch", "--help"]);
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("--merge-mode"))
        .stdout(predicate::str::contains("--top"))
        .stdout(predicate::str::contains("--ttl"))
        .stdout(predicate::str::contains("--force-refresh"));
}

#[test]
fn server_help_mentions_bind_options() {
    let mut cmd = Command::cargo_bin("rosettrism").unwrap();
    cmd.args(["server", "--help"]);
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("--host"))
        .stdout(predicate::str::contains("--port"))
        .stdout(predicate::str::contains("--open"));
}

#[test]
fn search_help_lists_online_sources() {
    let mut cmd = Command::cargo_bin("rosettrism").unwrap();
    cmd.args(["search", "--help"]);
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("animesongz"))
        .stdout(predicate::str::contains("apple-music"))
        .stdout(predicate::str::contains("awa"))
        .stdout(predicate::str::contains("azlyrics"))
        .stdout(predicate::str::contains("browser-mxm"))
        .stdout(predicate::str::contains("genius"))
        .stdout(predicate::str::contains("j-lyric"))
        .stdout(predicate::str::contains("j-lyric-net"))
        .stdout(predicate::str::contains("j-total"))
        .stdout(predicate::str::contains("joysound"))
        .stdout(predicate::str::contains("joy-sound"))
        .stdout(predicate::str::contains("kashinavi"))
        .stdout(predicate::str::contains("kkbox"))
        .stdout(predicate::str::contains("kkbox-web"))
        .stdout(predicate::str::contains("lrclib"))
        .stdout(predicate::str::contains("lrc-lib"))
        .stdout(predicate::str::contains("line-music"))
        .stdout(predicate::str::contains("line"))
        .stdout(predicate::str::contains("lyrical-nonsense"))
        .stdout(predicate::str::contains("migu"))
        .stdout(predicate::str::contains("migu-music"))
        .stdout(predicate::str::contains("musixmatch"))
        .stdout(predicate::str::contains("offline-db"))
        .stdout(predicate::str::contains("local-db"))
        .stdout(predicate::str::contains("petit-lyrics"))
        .stdout(predicate::str::contains("petitlyrics"))
        .stdout(predicate::str::contains("rocklyric"))
        .stdout(predicate::str::contains("rock-lyric"))
        .stdout(predicate::str::contains("songtexte"))
        .stdout(predicate::str::contains("spotify-lyrics"))
        .stdout(predicate::str::contains("spotify"))
        .stdout(predicate::str::contains("tunecore"))
        .stdout(predicate::str::contains("linkco-re"))
        .stdout(predicate::str::contains("uta-net"))
        .stdout(predicate::str::contains("utanet"))
        .stdout(predicate::str::contains("utaten"))
        .stdout(predicate::str::contains("uta-ten"))
        .stdout(predicate::str::contains("utamap"));
}

#[test]
fn experimental_sources_are_gated_in_cli_by_default() {
    let mut cmd = Command::cargo_bin("rosettrism").unwrap();
    cmd.env_remove("ROSETTRISM_ALLOW_EXPERIMENTAL").args([
        "search",
        "spotify:track:ABCDEFGHIJKLMNOPQRSTUV",
        "--source",
        "spotify-lyrics",
    ]);
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("experimental source"))
        .stderr(predicate::str::contains("--allow-experimental"));
}

#[test]
fn allow_experimental_reaches_stub_provider() {
    let mut cmd = Command::cargo_bin("rosettrism").unwrap();
    cmd.args([
        "--allow-experimental",
        "search",
        "Song",
        "--source",
        "browser-mxm",
    ]);
    cmd.assert()
        .failure()
        .stderr(predicate::str::contains("not implemented yet"));
}

fn temp_cli_cache_dir(prefix: &str) -> (PathBuf, PathBuf) {
    let dir = std::env::temp_dir().join(format!(
        "{prefix}-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&dir).unwrap();
    let db = dir.join("cache.sqlite");
    (dir, db)
}

fn seed_cache_for_maintenance(db: &PathBuf) {
    use rosettrism::cache::{now_unix, CachePut, FetchRunMetadata, UpstreamCache};
    use rosettrism::provider::Source;
    use serde_json::json;
    use std::time::Duration;

    let cache = UpstreamCache::open(db).unwrap();
    cache
        .put(CachePut {
            key: "expired-upstream",
            source: Source::Lrclib,
            operation: "search",
            status_code: 200,
            body: br#"[]"#,
            metadata: &json!({"query":"expired"}),
            ttl: Duration::from_secs(60),
        })
        .unwrap();
    cache
        .put(CachePut {
            key: "fresh-upstream",
            source: Source::Lrclib,
            operation: "search",
            status_code: 200,
            body: br#"[]"#,
            metadata: &json!({"query":"fresh"}),
            ttl: Duration::from_secs(3600),
        })
        .unwrap();
    let expired_unified_id = cache
        .put_unified(
            "expired-unified",
            br#"{"results":[]}"#,
            &[],
            Duration::from_secs(60),
        )
        .unwrap();
    let fresh_unified_id = cache
        .put_unified(
            "fresh-unified",
            br#"{"results":[]}"#,
            &[],
            Duration::from_secs(3600),
        )
        .unwrap();
    cache
        .put_ai_score(expired_unified_id, &json!({"score":"expired"}))
        .unwrap();
    cache
        .put_ai_score(fresh_unified_id, &json!({"score":"old"}))
        .unwrap();
    cache
        .put_ai_score(fresh_unified_id, &json!({"score":"new"}))
        .unwrap();
    for query in ["old run", "new run"] {
        let run_id = cache
            .start_fetch_run(query, Some(Source::Lrclib), "test")
            .unwrap();
        cache
            .finish_fetch_run(
                run_id,
                "success",
                None,
                FetchRunMetadata {
                    provider_count: Some(1),
                    candidate_count: Some(1),
                    cache_event: None,
                },
            )
            .unwrap();
    }
    drop(cache);

    let now = now_unix();
    let connection = rusqlite::Connection::open(db).unwrap();
    connection
        .execute(
            "UPDATE upstream_cache SET expires_at = ?1 WHERE cache_key = 'expired-upstream'",
            rusqlite::params![now - 10],
        )
        .unwrap();
    connection
        .execute(
            "UPDATE unified_cache SET expires_at = ?1 WHERE cache_key = 'expired-unified'",
            rusqlite::params![now - 10],
        )
        .unwrap();
    connection
        .execute(
            "UPDATE fetch_runs SET started_at = ?1, created_at = ?1 WHERE query = 'old run'",
            rusqlite::params![now - 20],
        )
        .unwrap();
    connection
        .execute(
            "UPDATE fetch_runs SET started_at = ?1, created_at = ?1 WHERE query = 'new run'",
            rusqlite::params![now - 10],
        )
        .unwrap();
    connection
        .execute(
            "UPDATE ai_scores SET created_at = ?1 WHERE score_json LIKE '%old%'",
            rusqlite::params![now - 20],
        )
        .unwrap();
    connection
        .execute(
            "UPDATE ai_scores SET created_at = ?1 WHERE score_json LIKE '%new%'",
            rusqlite::params![now - 10],
        )
        .unwrap();
}

#[test]
fn cache_stats_prints_json() {
    let (dir, db) = temp_cli_cache_dir("rosettrism-cli-cache-stats");
    seed_cache_for_maintenance(&db);

    let mut cmd = Command::cargo_bin("rosettrism").unwrap();
    cmd.arg("--db").arg(&db).args(["cache", "stats"]);
    let output = cmd.assert().success().get_output().stdout.clone();
    let value: serde_json::Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(value["upstream_entries"], 2);
    assert_eq!(value["expired_upstream_entries"], 1);
    assert_eq!(value["expired_unified_entries"], 1);

    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn cache_prune_is_dry_run_unless_yes() {
    let (dir, db) = temp_cli_cache_dir("rosettrism-cli-cache-prune");
    seed_cache_for_maintenance(&db);

    let mut dry_run = Command::cargo_bin("rosettrism").unwrap();
    dry_run.arg("--db").arg(&db).args([
        "cache",
        "prune",
        "--keep-fetch-runs",
        "1",
        "--keep-ai-scores",
        "1",
    ]);
    let output = dry_run.assert().success().get_output().stdout.clone();
    let value: serde_json::Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(value["dry_run"], true);
    assert_eq!(value["expired_upstream_entries"], 1);

    let mut stats_after_dry_run = Command::cargo_bin("rosettrism").unwrap();
    stats_after_dry_run
        .arg("--db")
        .arg(&db)
        .args(["cache", "stats"]);
    let output = stats_after_dry_run
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let value: serde_json::Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(value["upstream_entries"], 2);
    assert_eq!(value["fetch_run_entries"], 2);

    let mut prune = Command::cargo_bin("rosettrism").unwrap();
    prune.arg("--db").arg(&db).args([
        "cache",
        "prune",
        "--yes",
        "--keep-fetch-runs",
        "1",
        "--keep-ai-scores",
        "1",
    ]);
    prune.assert().success();

    let mut stats_after_prune = Command::cargo_bin("rosettrism").unwrap();
    stats_after_prune
        .arg("--db")
        .arg(&db)
        .args(["cache", "stats"]);
    let output = stats_after_prune
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    let value: serde_json::Value = serde_json::from_slice(&output).unwrap();
    assert_eq!(value["upstream_entries"], 1);
    assert_eq!(value["unified_entries"], 1);
    assert_eq!(value["fetch_run_entries"], 1);
    assert_eq!(value["ai_score_entries"], 1);

    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn cache_export_supports_jsonl_and_pretty_json() {
    let (dir, db) = temp_cli_cache_dir("rosettrism-cli-cache-export");
    seed_cache_for_maintenance(&db);

    let mut jsonl = Command::cargo_bin("rosettrism").unwrap();
    jsonl.arg("--db").arg(&db).args([
        "cache",
        "export",
        "--format",
        "jsonl",
        "--upstream",
        "--limit",
        "1",
    ]);
    let output = jsonl.assert().success().get_output().stdout.clone();
    let lines = String::from_utf8(output).unwrap();
    let values = lines
        .lines()
        .map(|line| serde_json::from_str::<serde_json::Value>(line).unwrap())
        .collect::<Vec<_>>();
    assert_eq!(values.len(), 1);
    assert_eq!(values[0]["section"], "upstream");

    let mut pretty = Command::cargo_bin("rosettrism").unwrap();
    pretty.arg("--db").arg(&db).args([
        "cache",
        "export",
        "--format",
        "pretty-json",
        "--fetch-runs",
        "--limit",
        "2",
    ]);
    let output = pretty.assert().success().get_output().stdout.clone();
    let value: serde_json::Value = serde_json::from_slice(&output).unwrap();
    assert!(value
        .as_array()
        .unwrap()
        .iter()
        .all(|item| item["section"] == "fetch_runs"));

    fs::remove_dir_all(dir).unwrap();
}
