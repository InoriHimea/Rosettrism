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
