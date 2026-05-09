use std::time::Duration;

use async_trait::async_trait;
use regex::Regex;
use reqwest::header::{HeaderMap, HeaderValue, ACCEPT, USER_AGENT};
use reqwest::StatusCode;
use serde::de::DeserializeOwned;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::decoder::InputFormat;
use crate::provider::{FetchedLyric, LyricProvider, SearchResult, Source};
use crate::{Error, Result};

const USER_AGENT_VALUE: &str = concat!(
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) rosettrism/",
    env!("CARGO_PKG_VERSION")
);

pub struct LrclibProvider {
    client: reqwest::Client,
    api_base_url: String,
}

impl LrclibProvider {
    pub fn new(_cookie: Option<String>) -> Result<Self> {
        Self::with_api_base_url("https://lrclib.net/api")
    }

    fn with_api_base_url(api_base_url: impl Into<String>) -> Result<Self> {
        let mut headers = HeaderMap::new();
        headers.insert(USER_AGENT, HeaderValue::from_static(USER_AGENT_VALUE));
        headers.insert(
            ACCEPT,
            HeaderValue::from_static("application/json, text/plain, */*"),
        );

        let client = reqwest::Client::builder()
            .default_headers(headers)
            .timeout(Duration::from_secs(12))
            .build()?;

        Ok(Self {
            client,
            api_base_url: trim_trailing_slash(api_base_url.into()),
        })
    }

    async fn send_json<T>(&self, endpoint: &str, query: &[(&str, &str)]) -> Result<Option<T>>
    where
        T: DeserializeOwned,
    {
        let url = format!("{}/{}", self.api_base_url, endpoint.trim_start_matches('/'));
        let response = self.client.get(url).query(query).send().await?;
        let status = response.status();
        let body = response.text().await?;

        if status == StatusCode::NOT_FOUND {
            return Ok(None);
        }
        if !status.is_success() {
            let preview = response_preview(&body);
            let detail = if preview.is_empty() {
                String::new()
            } else {
                format!(": {preview}")
            };
            return Err(Error::Provider(format!(
                "LRCLIB {endpoint} failed with HTTP {status}{detail}"
            )));
        }

        Ok(Some(serde_json::from_str(&body)?))
    }

    async fn search_api(&self, query: &str) -> Result<Vec<SearchResult>> {
        if let Some(id) = parse_direct_id(query) {
            return Ok(vec![direct_result(id)]);
        }

        let Some(items) = self
            .send_json::<Vec<LrclibLyrics>>("search", &[("q", query)])
            .await?
        else {
            return Ok(Vec::new());
        };

        Ok(items.into_iter().map(LrclibLyrics::into_result).collect())
    }

    async fn download_by_id(&self, id: &str) -> Result<FetchedLyric> {
        let Some(lyrics) = self
            .send_json::<LrclibLyrics>(&format!("get/{id}"), &[])
            .await?
        else {
            return Err(Error::Provider(format!("LRCLIB lyric {id} was not found")));
        };

        Ok(lyrics.into_fetched())
    }
}

#[async_trait]
impl LyricProvider for LrclibProvider {
    async fn search(&self, query: &str) -> Result<Vec<SearchResult>> {
        self.search_api(query).await
    }

    async fn fetch(&self, result: &SearchResult) -> Result<FetchedLyric> {
        if let Some(fetched) = fetched_from_result_payload(result) {
            return Ok(fetched);
        }

        let id = result
            .extra
            .get("lrclib_id")
            .and_then(Value::as_u64)
            .map(|value| value.to_string())
            .unwrap_or_else(|| result.id.clone());

        if id.trim().is_empty() {
            return Err(Error::Provider(
                "LRCLIB result did not include an id".into(),
            ));
        }

        self.download_by_id(&id).await
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LrclibLyrics {
    id: u64,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    track_name: Option<String>,
    #[serde(default)]
    artist_name: Option<String>,
    #[serde(default)]
    album_name: Option<String>,
    #[serde(default)]
    duration: Option<Value>,
    #[serde(default)]
    instrumental: bool,
    #[serde(default)]
    plain_lyrics: Option<String>,
    #[serde(default)]
    synced_lyrics: Option<String>,
    #[serde(default)]
    lyricsfile: Option<String>,
}

impl LrclibLyrics {
    fn into_result(self) -> SearchResult {
        let title = self
            .track_name
            .clone()
            .or_else(|| self.name.clone())
            .unwrap_or_else(|| format!("LRCLIB {}", self.id));
        let duration_seconds = duration_seconds(self.duration.as_ref());
        let duration_ms = duration_seconds.map(duration_to_ms);

        SearchResult {
            source: Source::Lrclib,
            id: self.id.to_string(),
            title,
            artist: self.artist_name.unwrap_or_default(),
            album: self.album_name,
            duration_ms,
            extra: json!({
                "lrclib_id": self.id,
                "instrumental": self.instrumental,
                "duration": duration_seconds,
                "plainLyrics": self.plain_lyrics,
                "syncedLyrics": self.synced_lyrics,
                "lyricsfile": self.lyricsfile
            }),
        }
    }

    fn into_fetched(self) -> FetchedLyric {
        lyrics_payload_to_fetched(
            self.instrumental,
            self.synced_lyrics,
            self.plain_lyrics,
            self.lyricsfile,
        )
    }
}

fn direct_result(id: String) -> SearchResult {
    SearchResult {
        source: Source::Lrclib,
        id: id.clone(),
        title: format!("LRCLIB {id}"),
        artist: String::new(),
        album: None,
        duration_ms: None,
        extra: json!({
            "lrclib_id": id
        }),
    }
}

fn fetched_from_result_payload(result: &SearchResult) -> Option<FetchedLyric> {
    let instrumental = extra_bool(&result.extra, "instrumental").unwrap_or(false);
    let synced = extra_string(&result.extra, "syncedLyrics");
    let plain = extra_string(&result.extra, "plainLyrics");
    let lyricsfile = extra_string(&result.extra, "lyricsfile");

    if instrumental
        || synced
            .as_deref()
            .is_some_and(|value| !value.trim().is_empty())
        || plain
            .as_deref()
            .is_some_and(|value| !value.trim().is_empty())
        || lyricsfile
            .as_deref()
            .is_some_and(|value| !value.trim().is_empty())
    {
        Some(lyrics_payload_to_fetched(
            instrumental,
            synced,
            plain,
            lyricsfile,
        ))
    } else {
        None
    }
}

fn lyrics_payload_to_fetched(
    instrumental: bool,
    synced: Option<String>,
    plain: Option<String>,
    lyricsfile: Option<String>,
) -> FetchedLyric {
    if instrumental {
        return FetchedLyric {
            input_format: InputFormat::Text,
            raw: Vec::new(),
            document: None,
            annotations: Vec::new(),
        };
    }

    if let Some(synced) = synced.filter(|value| !value.trim().is_empty()) {
        return FetchedLyric {
            input_format: InputFormat::Lrc,
            raw: ensure_trailing_newline(synced).into_bytes(),
            document: None,
            annotations: Vec::new(),
        };
    }

    if let Some(plain) = plain.filter(|value| !value.trim().is_empty()) {
        return FetchedLyric {
            input_format: InputFormat::Text,
            raw: ensure_trailing_newline(plain).into_bytes(),
            document: None,
            annotations: Vec::new(),
        };
    }

    let lyricsfile = lyricsfile.unwrap_or_default();
    FetchedLyric {
        input_format: if looks_like_lrc(&lyricsfile) {
            InputFormat::Lrc
        } else {
            InputFormat::Text
        },
        raw: ensure_trailing_newline(lyricsfile).into_bytes(),
        document: None,
        annotations: Vec::new(),
    }
}

fn duration_seconds(value: Option<&Value>) -> Option<f64> {
    match value? {
        Value::Number(number) => number.as_f64(),
        Value::String(value) => value.trim().parse().ok(),
        _ => None,
    }
}

fn duration_to_ms(seconds: f64) -> u32 {
    if !seconds.is_finite() || seconds <= 0.0 {
        return 0;
    }
    (seconds * 1_000.0).round().min(u32::MAX as f64) as u32
}

fn extra_string(value: &Value, key: &str) -> Option<String> {
    let value = value.get(key)?;
    if value.is_null() {
        return None;
    }

    value
        .as_str()
        .map(ToOwned::to_owned)
        .or_else(|| value.as_u64().map(|number| number.to_string()))
        .or_else(|| value.as_i64().map(|number| number.to_string()))
        .or_else(|| value.as_f64().map(|number| number.to_string()))
}

fn extra_bool(value: &Value, key: &str) -> Option<bool> {
    let value = value.get(key)?;
    value
        .as_bool()
        .or_else(|| value.as_u64().map(|number| number != 0))
        .or_else(|| value.as_i64().map(|number| number != 0))
}

fn looks_like_lrc(value: &str) -> bool {
    Regex::new(r"(?m)\[\d{1,2}:\d{2}(?:[.:]\d{1,3})?\]")
        .ok()
        .is_some_and(|re| re.is_match(value))
}

fn parse_direct_id(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if !trimmed.is_empty() && trimmed.chars().all(|ch| ch.is_ascii_digit()) {
        Some(trimmed.to_string())
    } else {
        None
    }
}

fn ensure_trailing_newline(mut value: String) -> String {
    if !value.ends_with('\n') {
        value.push('\n');
    }
    value
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
    use serde_json::json;
    use wiremock::matchers::{method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::*;

    fn provider(server: &MockServer) -> LrclibProvider {
        LrclibProvider::with_api_base_url(format!("{}/api", server.uri())).unwrap()
    }

    #[tokio::test]
    async fn searches_and_fetches_synced_lrc() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/api/search"))
            .and(query_param("q", "Song Artist"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!([{
                "id": 10,
                "trackName": "Song",
                "artistName": "Artist",
                "albumName": "Album",
                "duration": 123,
                "instrumental": false
            }])))
            .mount(&server)
            .await;

        Mock::given(method("GET"))
            .and(path("/api/get/10"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "id": 10,
                "trackName": "Song",
                "artistName": "Artist",
                "instrumental": false,
                "plainLyrics": "Hi",
                "syncedLyrics": "[00:01.00]Hi"
            })))
            .mount(&server)
            .await;

        let provider = provider(&server);
        let results = provider.search("Song Artist").await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].source, Source::Lrclib);
        assert_eq!(results[0].duration_ms, Some(123_000));
        assert_eq!(results[0].extra["lrclib_id"], 10);

        let fetched = provider.fetch(&results[0]).await.unwrap();
        assert_eq!(fetched.input_format, InputFormat::Lrc);
        assert_eq!(String::from_utf8(fetched.raw).unwrap(), "[00:01.00]Hi\n");
    }

    #[tokio::test]
    async fn search_payload_can_be_fetched_without_second_request() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/api/search"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!([{
                "id": 10,
                "trackName": "Song",
                "artistName": "Artist",
                "duration": 123.45,
                "instrumental": false,
                "plainLyrics": "Hi",
                "syncedLyrics": "[00:01.00]Hi"
            }])))
            .mount(&server)
            .await;

        let provider = provider(&server);
        let results = provider.search("Song Artist").await.unwrap();
        assert_eq!(results[0].duration_ms, Some(123_450));

        let fetched = provider.fetch(&results[0]).await.unwrap();
        assert_eq!(fetched.input_format, InputFormat::Lrc);
        assert_eq!(String::from_utf8(fetched.raw).unwrap(), "[00:01.00]Hi\n");
    }

    #[tokio::test]
    async fn supports_direct_numeric_id_queries() {
        let server = MockServer::start().await;
        let results = provider(&server).search("10").await.unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, "10");
        assert_eq!(results[0].extra["lrclib_id"], "10");
    }

    #[tokio::test]
    async fn fetch_uses_lyricsfile_when_no_plain_or_synced_lyrics_exist() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/api/get/10"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "id": 10,
                "instrumental": false,
                "lyricsfile": "[00:01.00]Hi"
            })))
            .mount(&server)
            .await;

        let result = SearchResult {
            source: Source::Lrclib,
            id: "10".into(),
            title: "Song".into(),
            artist: "Artist".into(),
            album: None,
            duration_ms: None,
            extra: json!({}),
        };

        let fetched = provider(&server).fetch(&result).await.unwrap();
        assert_eq!(fetched.input_format, InputFormat::Lrc);
        assert_eq!(String::from_utf8(fetched.raw).unwrap(), "[00:01.00]Hi\n");
    }

    #[tokio::test]
    async fn fetch_falls_back_to_plain_lyrics() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/api/get/10"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "id": 10,
                "trackName": "Song",
                "artistName": "Artist",
                "instrumental": false,
                "plainLyrics": "Hi\nthere",
                "syncedLyrics": null
            })))
            .mount(&server)
            .await;

        let result = SearchResult {
            source: Source::Lrclib,
            id: "10".into(),
            title: "Song".into(),
            artist: "Artist".into(),
            album: None,
            duration_ms: None,
            extra: json!({ "lrclib_id": 10 }),
        };

        let fetched = provider(&server).fetch(&result).await.unwrap();
        assert_eq!(fetched.input_format, InputFormat::Text);
        assert_eq!(String::from_utf8(fetched.raw).unwrap(), "Hi\nthere\n");
    }

    #[tokio::test]
    async fn instrumental_fetch_returns_empty_text() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/api/get/10"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "id": 10,
                "instrumental": true,
                "plainLyrics": null,
                "syncedLyrics": null
            })))
            .mount(&server)
            .await;

        let result = SearchResult {
            source: Source::Lrclib,
            id: "10".into(),
            title: "Song".into(),
            artist: "Artist".into(),
            album: None,
            duration_ms: None,
            extra: json!({}),
        };

        let fetched = provider(&server).fetch(&result).await.unwrap();
        assert_eq!(fetched.input_format, InputFormat::Text);
        assert!(fetched.raw.is_empty());
    }

    #[tokio::test]
    async fn http_errors_are_readable() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/api/search"))
            .respond_with(ResponseTemplate::new(500).set_body_string("upstream sad"))
            .mount(&server)
            .await;

        let err = provider(&server)
            .search("Song")
            .await
            .unwrap_err()
            .to_string();
        assert!(err.contains("LRCLIB"));
        assert!(err.contains("HTTP 500"));
        assert!(err.contains("upstream sad"));
    }
}
