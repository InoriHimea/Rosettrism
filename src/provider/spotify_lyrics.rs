use std::time::Duration;

use async_trait::async_trait;
use regex::Regex;
use reqwest::header::{HeaderMap, HeaderValue, ACCEPT, AUTHORIZATION, COOKIE, USER_AGENT};
use reqwest::StatusCode;
use serde::Deserialize;
use serde_json::json;

use crate::decoder::InputFormat;
use crate::model::{LyricDocument, LyricLine, LyricMeta, LyricWord};
use crate::provider::{FetchedLyric, LyricProvider, SearchResult, Source};
use crate::{Error, Result};

const USER_AGENT_VALUE: &str = concat!(
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) rosettrism/",
    env!("CARGO_PKG_VERSION")
);

pub struct SpotifyLyricsProvider {
    client: reqwest::Client,
    api_base_url: String,
    has_auth: bool,
}

impl SpotifyLyricsProvider {
    pub fn new(auth: Option<String>) -> Result<Self> {
        Self::with_api_base_url(auth, "https://spclient.wg.spotify.com")
    }

    fn with_api_base_url(auth: Option<String>, api_base_url: impl Into<String>) -> Result<Self> {
        let mut headers = HeaderMap::new();
        headers.insert(USER_AGENT, HeaderValue::from_static(USER_AGENT_VALUE));
        headers.insert(ACCEPT, HeaderValue::from_static("application/json, */*"));

        let has_auth = if let Some(auth) = auth
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            apply_auth_header(&mut headers, auth)?;
            true
        } else {
            false
        };

        let client = reqwest::Client::builder()
            .default_headers(headers)
            .timeout(Duration::from_secs(12))
            .build()?;

        Ok(Self {
            client,
            api_base_url: trim_trailing_slash(api_base_url.into()),
            has_auth,
        })
    }

    async fn search_track(&self, query: &str) -> Result<Vec<SearchResult>> {
        let Some(track_id) = parse_track_id(query) else {
            return Err(Error::Provider(
                "Spotify lyrics currently supports direct Spotify track URLs, spotify:track URIs, or 22-character track IDs only".into(),
            ));
        };

        Ok(vec![SearchResult {
            source: Source::SpotifyLyrics,
            id: track_id.clone(),
            title: format!("Spotify track {track_id}"),
            artist: String::new(),
            album: None,
            duration_ms: None,
            extra: json!({ "spotify_track_id": track_id }),
        }])
    }

    async fn download_track(&self, track_id: &str, result: &SearchResult) -> Result<FetchedLyric> {
        if !self.has_auth {
            return Err(Error::Provider(
                "Spotify lyrics requires user-provided auth via ROSETTRISM_SPOTIFY_BEARER_TOKEN, ROSETTRISM_SPOTIFY_COOKIE_FILE, or --cookie-file".into(),
            ));
        }

        let url = format!("{}/color-lyrics/v2/track/{track_id}", self.api_base_url);
        let response = self.client.get(url).send().await?;
        let status = response.status();
        let body = response.text().await?;

        if !status.is_success() {
            return Err(spotify_status_error(status, &body));
        }

        let mut document = parse_spotify_document(&body)?;
        document.meta.title = maybe_known_title(result);
        document.meta.artist = maybe_known_artist(result);
        document.meta.source = Some("spotify-lyrics".into());

        Ok(FetchedLyric {
            input_format: InputFormat::Json,
            raw: body.into_bytes(),
            document: Some(document),
            annotations: Vec::new(),
        })
    }
}

#[async_trait]
impl LyricProvider for SpotifyLyricsProvider {
    async fn search(&self, query: &str) -> Result<Vec<SearchResult>> {
        self.search_track(query).await
    }

    async fn fetch(&self, result: &SearchResult) -> Result<FetchedLyric> {
        let track_id = result
            .extra
            .get("spotify_track_id")
            .and_then(|value| value.as_str())
            .unwrap_or(result.id.as_str());

        self.download_track(track_id, result).await
    }
}

#[derive(Debug, Deserialize)]
struct SpotifyResponse {
    lyrics: Option<SpotifyLyrics>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SpotifyLyrics {
    #[serde(default)]
    lines: Vec<SpotifyLine>,
    #[serde(default)]
    provider: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SpotifyLine {
    #[serde(default)]
    start_time_ms: Option<serde_json::Value>,
    #[serde(default)]
    end_time_ms: Option<serde_json::Value>,
    #[serde(default)]
    words: Option<String>,
    #[serde(default)]
    syllables: Vec<SpotifySyllable>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SpotifySyllable {
    #[serde(default)]
    start_time_ms: Option<serde_json::Value>,
    #[serde(default)]
    end_time_ms: Option<serde_json::Value>,
    #[serde(default)]
    duration_ms: Option<serde_json::Value>,
    #[serde(default)]
    text: Option<String>,
    #[serde(default)]
    words: Option<String>,
    #[serde(default)]
    num_chars: Option<serde_json::Value>,
}

fn parse_spotify_document(body: &str) -> Result<LyricDocument> {
    let response: SpotifyResponse = serde_json::from_str(body)?;
    let lyrics = response
        .lyrics
        .ok_or_else(|| Error::Provider("Spotify lyric response did not include lyrics".into()))?;

    let mut lines = Vec::new();
    for line in lyrics.lines {
        let text = line.words.unwrap_or_default();
        if text.trim().is_empty() {
            continue;
        }

        let start_ms = value_to_u32(line.start_time_ms.as_ref()).unwrap_or(0);
        let end_ms = value_to_u32(line.end_time_ms.as_ref());
        let duration_ms = end_ms.and_then(|end| end.checked_sub(start_ms));
        let words = spotify_syllables_to_words(&text, start_ms, line.syllables);

        lines.push(LyricLine {
            start_ms,
            duration_ms,
            text,
            words,
            ruby: Vec::new(),
            reading: None,
            romanized: None,
        });
    }

    if lines.is_empty() {
        return Err(Error::Provider(
            "Spotify lyric response did not contain lyric lines".into(),
        ));
    }

    let mut document = LyricDocument {
        meta: LyricMeta {
            source: lyrics.provider.or_else(|| Some("spotify-lyrics".into())),
            ..Default::default()
        },
        lines,
    };
    document.sort_and_fill_durations();
    Ok(document)
}

fn spotify_syllables_to_words(
    line_text: &str,
    line_start_ms: u32,
    syllables: Vec<SpotifySyllable>,
) -> Vec<LyricWord> {
    let mut char_cursor = 0_usize;
    let mut words = Vec::new();

    for syllable in syllables {
        let start_ms = value_to_u32(syllable.start_time_ms.as_ref()).unwrap_or(line_start_ms);
        let duration_ms = value_to_u32(syllable.duration_ms.as_ref()).or_else(|| {
            value_to_u32(syllable.end_time_ms.as_ref()).and_then(|end| end.checked_sub(start_ms))
        });
        let text = syllable
            .text
            .or(syllable.words)
            .or_else(|| {
                let count = value_to_u32(syllable.num_chars.as_ref())? as usize;
                take_chars(line_text, &mut char_cursor, count)
            })
            .unwrap_or_default();

        if text.is_empty() {
            continue;
        }

        words.push(LyricWord {
            offset_ms: start_ms.saturating_sub(line_start_ms),
            duration_ms: duration_ms.unwrap_or(0),
            text,
        });
    }

    words
}

fn take_chars(value: &str, cursor: &mut usize, count: usize) -> Option<String> {
    if count == 0 {
        return None;
    }

    let chars = value.chars().collect::<Vec<_>>();
    while *cursor < chars.len() && chars[*cursor].is_whitespace() {
        *cursor += 1;
    }

    if *cursor >= chars.len() {
        return None;
    }

    let start = *cursor;
    let end = (*cursor + count).min(chars.len());
    *cursor = end;
    Some(chars[start..end].iter().collect())
}

fn apply_auth_header(headers: &mut HeaderMap, auth: &str) -> Result<()> {
    if looks_like_cookie(auth) {
        headers.insert(
            COOKIE,
            HeaderValue::from_str(auth)
                .map_err(|err| Error::Provider(format!("invalid Spotify cookie header: {err}")))?,
        );
        return Ok(());
    }

    let token = auth.strip_prefix("Bearer ").unwrap_or(auth).trim();
    headers.insert(
        AUTHORIZATION,
        HeaderValue::from_str(&format!("Bearer {token}")).map_err(|err| {
            Error::Provider(format!("invalid Spotify bearer token header: {err}"))
        })?,
    );
    Ok(())
}

fn looks_like_cookie(value: &str) -> bool {
    value.contains('=') || value.contains(';')
}

fn parse_track_id(value: &str) -> Option<String> {
    let trimmed = value.trim();
    let id_re = Regex::new(r"^[A-Za-z0-9]{22}$").ok()?;
    if id_re.is_match(trimmed) {
        return Some(trimmed.to_string());
    }

    let uri_re =
        Regex::new(r"(?:spotify:track:|open\.spotify\.com/track/)([A-Za-z0-9]{22})").ok()?;
    uri_re
        .captures(trimmed)
        .and_then(|capture| capture.get(1))
        .map(|value| value.as_str().to_string())
}

fn value_to_u32(value: Option<&serde_json::Value>) -> Option<u32> {
    match value? {
        serde_json::Value::Number(number) => {
            number.as_u64().and_then(|value| u32::try_from(value).ok())
        }
        serde_json::Value::String(value) => value.trim().parse().ok(),
        _ => None,
    }
}

fn maybe_known_title(result: &SearchResult) -> Option<String> {
    if result.title.starts_with("Spotify track ") {
        None
    } else {
        Some(result.title.clone())
    }
}

fn maybe_known_artist(result: &SearchResult) -> Option<String> {
    if result.artist.trim().is_empty() {
        None
    } else {
        Some(result.artist.clone())
    }
}

fn spotify_status_error(status: StatusCode, body: &str) -> Error {
    let hint = match status {
        StatusCode::UNAUTHORIZED => "authorization was rejected",
        StatusCode::FORBIDDEN => "lyrics are unavailable for this account, market, or track",
        StatusCode::NOT_FOUND => "track or lyrics were not found",
        StatusCode::TOO_MANY_REQUESTS => "rate limited by Spotify",
        _ => "request failed",
    };
    let preview = response_preview(body);
    let detail = if preview.is_empty() {
        String::new()
    } else {
        format!(": {preview}")
    };

    Error::Provider(format!("Spotify lyrics {hint} with HTTP {status}{detail}"))
}

fn response_preview(body: &str) -> String {
    const MAX_CHARS: usize = 180;

    let collapsed = body.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut chars = collapsed.chars();
    let preview = chars.by_ref().take(MAX_CHARS).collect::<String>();
    if chars.next().is_some() {
        format!("{preview}...")
    } else {
        preview
    }
}

fn trim_trailing_slash(mut value: String) -> String {
    while value.ends_with('/') {
        value.pop();
    }
    value
}

#[cfg(test)]
mod tests {
    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::*;

    fn provider(server: &MockServer, auth: Option<&str>) -> SpotifyLyricsProvider {
        SpotifyLyricsProvider::with_api_base_url(auth.map(ToOwned::to_owned), server.uri()).unwrap()
    }

    #[tokio::test]
    async fn supports_direct_track_url_uri_and_id() {
        let server = MockServer::start().await;
        let provider = provider(&server, None);

        let by_id = provider.search("1234567890abcdefghijKL").await.unwrap();
        assert_eq!(by_id[0].id, "1234567890abcdefghijKL");

        let by_uri = provider
            .search("spotify:track:ABCDEFGHIJKLMNOPQRSTUV")
            .await
            .unwrap();
        assert_eq!(by_uri[0].id, "ABCDEFGHIJKLMNOPQRSTUV");

        let by_url = provider
            .search("https://open.spotify.com/track/ABCDEFGHIJKLMNOPQRSTUV?si=abc")
            .await
            .unwrap();
        assert_eq!(by_url[0].id, "ABCDEFGHIJKLMNOPQRSTUV");
    }

    #[tokio::test]
    async fn fetches_line_synced_json_document() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/color-lyrics/v2/track/ABCDEFGHIJKLMNOPQRSTUV"))
            .and(header("authorization", "Bearer token-123"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "lyrics": {
                    "provider": "Musixmatch",
                    "lines": [
                        {
                            "startTimeMs": "1000",
                            "endTimeMs": "2500",
                            "words": "hello world",
                            "syllables": [
                                { "startTimeMs": "1000", "durationMs": "400", "text": "hello" },
                                { "startTimeMs": "1500", "endTimeMs": "2500", "text": "world" }
                            ]
                        },
                        {
                            "startTimeMs": "3000",
                            "words": "next"
                        }
                    ]
                }
            })))
            .mount(&server)
            .await;

        let result = SearchResult {
            source: Source::SpotifyLyrics,
            id: "ABCDEFGHIJKLMNOPQRSTUV".into(),
            title: "Song".into(),
            artist: "Artist".into(),
            album: None,
            duration_ms: None,
            extra: json!({ "spotify_track_id": "ABCDEFGHIJKLMNOPQRSTUV" }),
        };

        let fetched = provider(&server, Some("token-123"))
            .fetch(&result)
            .await
            .unwrap();
        assert_eq!(fetched.input_format, InputFormat::Json);
        let document = fetched.document.unwrap();
        assert_eq!(document.meta.title.as_deref(), Some("Song"));
        assert_eq!(document.meta.artist.as_deref(), Some("Artist"));
        assert_eq!(document.lines[0].start_ms, 1_000);
        assert_eq!(document.lines[0].duration_ms, Some(1_500));
        assert_eq!(document.lines[0].words[1].offset_ms, 500);
        assert_eq!(document.lines[0].words[1].duration_ms, 1_000);
    }

    #[tokio::test]
    async fn fetch_requires_user_auth() {
        let server = MockServer::start().await;
        let result = SearchResult {
            source: Source::SpotifyLyrics,
            id: "ABCDEFGHIJKLMNOPQRSTUV".into(),
            title: "Spotify track ABCDEFGHIJKLMNOPQRSTUV".into(),
            artist: String::new(),
            album: None,
            duration_ms: None,
            extra: json!({}),
        };

        let err = provider(&server, None)
            .fetch(&result)
            .await
            .unwrap_err()
            .to_string();
        assert!(err.contains("requires user-provided auth"));
    }

    #[tokio::test]
    async fn http_errors_are_readable_and_redacted() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/color-lyrics/v2/track/ABCDEFGHIJKLMNOPQRSTUV"))
            .respond_with(ResponseTemplate::new(429).set_body_string("token-123 is too fast"))
            .mount(&server)
            .await;

        let result = SearchResult {
            source: Source::SpotifyLyrics,
            id: "ABCDEFGHIJKLMNOPQRSTUV".into(),
            title: "Spotify track ABCDEFGHIJKLMNOPQRSTUV".into(),
            artist: String::new(),
            album: None,
            duration_ms: None,
            extra: json!({}),
        };

        let err = provider(&server, Some("secret-token"))
            .fetch(&result)
            .await
            .unwrap_err()
            .to_string();
        assert!(err.contains("rate limited"));
        assert!(!err.contains("secret-token"));
    }
}
