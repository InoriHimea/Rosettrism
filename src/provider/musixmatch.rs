use async_trait::async_trait;
use reqwest::header::{HeaderMap, HeaderValue, ACCEPT, USER_AGENT};
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

pub struct MusixmatchProvider {
    client: reqwest::Client,
    api_base_url: String,
    api_key: String,
}

impl MusixmatchProvider {
    pub fn new(api_key: Option<String>, timeout_ms: u64) -> Result<Self> {
        Self::with_api_base_url(api_key, "https://api.musixmatch.com/ws/1.1", timeout_ms)
    }

    fn with_api_base_url(
        api_key: Option<String>,
        api_base_url: impl Into<String>,
        timeout_ms: u64,
    ) -> Result<Self> {
        let api_key = api_key
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .ok_or_else(|| {
                Error::Provider(
                    "Musixmatch requires an API key. Set ROSETTRISM_MUSIXMATCH_API_KEY (legacy: LRC_DECODE_MUSIXMATCH_API_KEY) or pass a raw key with --cookie-file."
                        .into(),
                )
            })?;

        let mut headers = HeaderMap::new();
        headers.insert(USER_AGENT, HeaderValue::from_static(USER_AGENT_VALUE));
        headers.insert(
            ACCEPT,
            HeaderValue::from_static("application/json, text/plain, */*"),
        );

        let client = crate::provider::apply_client_timeout(
            reqwest::Client::builder().default_headers(headers),
            timeout_ms,
        )
        .build()?;

        Ok(Self {
            client,
            api_base_url: trim_trailing_slash(api_base_url.into()),
            api_key,
        })
    }

    async fn send_api<T>(&self, endpoint: &str, query: Vec<(&str, String)>) -> Result<T>
    where
        T: DeserializeOwned,
    {
        let url = format!("{}/{}", self.api_base_url, endpoint);
        let response = self
            .client
            .get(url)
            .query(&query)
            .query(&[("apikey", self.api_key.as_str())])
            .send()
            .await?;
        let status = response.status();
        let body = response.text().await?;

        if !status.is_success() {
            let preview = response_preview(&body);
            let detail = if preview.is_empty() {
                String::new()
            } else {
                format!(": {preview}")
            };
            return Err(Error::Provider(format!(
                "Musixmatch {endpoint} failed with HTTP {status}{detail}"
            )));
        }

        let envelope: MusixmatchEnvelope<T> = serde_json::from_str(&body)?;
        if envelope.message.header.status_code != 200 {
            return Err(Error::Provider(format!(
                "Musixmatch {endpoint} failed with status code {}",
                envelope.message.header.status_code
            )));
        }

        envelope.message.body.ok_or_else(|| {
            Error::Provider(format!(
                "Musixmatch {endpoint} response did not include a body"
            ))
        })
    }

    async fn search_tracks(&self, query: &str) -> Result<Vec<SearchResult>> {
        let body = self
            .send_api::<MusixmatchSearchBody>(
                "track.search",
                vec![
                    ("q", query.to_string()),
                    ("f_has_lyrics", "1".to_string()),
                    ("page", "1".to_string()),
                    ("page_size", "10".to_string()),
                ],
            )
            .await?;

        let results = body
            .track_list
            .into_iter()
            .filter_map(|item| {
                let track = item.track;
                let track_id = track.track_id?;
                let title = track.track_name.unwrap_or_else(|| track_id.to_string());
                let artist = track.artist_name.unwrap_or_default();
                let duration_ms = track
                    .track_length
                    .map(|seconds| seconds.saturating_mul(1_000));

                Some(SearchResult {
                    source: Source::Musixmatch,
                    id: track_id.to_string(),
                    title,
                    artist,
                    album: track.album_name,
                    duration_ms,
                    extra: json!({
                        "track_id": track_id,
                        "commontrack_id": track.commontrack_id,
                        "track_isrc": track.track_isrc,
                        "track_spotify_id": track.track_spotify_id,
                        "has_lyrics": track.has_lyrics.unwrap_or_default() != 0,
                        "has_subtitles": track.has_subtitles.unwrap_or_default() != 0,
                        "restricted": track.restricted.unwrap_or_default() != 0
                    }),
                })
            })
            .collect();

        Ok(results)
    }

    async fn download_subtitle(&self, identifier: Identifier) -> Result<Vec<u8>> {
        let body = self
            .send_api::<MusixmatchSubtitleBody>(
                "track.subtitle.get",
                vec![
                    (identifier.name, identifier.value),
                    ("subtitle_format", "lrc".to_string()),
                ],
            )
            .await?;

        let subtitle = body
            .subtitle
            .and_then(|subtitle| subtitle.subtitle_body)
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| Error::Provider("Musixmatch subtitle response was empty".into()))?;

        Ok(ensure_trailing_newline(subtitle).into_bytes())
    }

    async fn download_plain_lyrics(&self, identifier: Identifier) -> Result<Vec<u8>> {
        let body = self
            .send_api::<MusixmatchLyricsBody>(
                "track.lyrics.get",
                vec![(identifier.name, identifier.value)],
            )
            .await?;

        let lyrics = body
            .lyrics
            .ok_or_else(|| Error::Provider("Musixmatch lyric response was empty".into()))?;
        let body = lyrics.lyrics_body.unwrap_or_default();
        if !body.trim().is_empty() {
            return Ok(ensure_trailing_newline(body).into_bytes());
        }

        let reason = lyrics
            .lyrics_copyright
            .filter(|value| !value.trim().is_empty())
            .map(|value| format!(": {}", response_preview(&value)))
            .unwrap_or_default();
        Err(Error::Provider(format!(
            "Musixmatch lyric body was empty or restricted{reason}"
        )))
    }
}

#[async_trait]
impl LyricProvider for MusixmatchProvider {
    async fn search(&self, query: &str) -> Result<Vec<SearchResult>> {
        self.search_tracks(query).await
    }

    async fn fetch(&self, result: &SearchResult) -> Result<FetchedLyric> {
        let mut errors = Vec::new();
        let should_try_subtitle = extra_bool(&result.extra, "has_subtitles").unwrap_or(true);

        if should_try_subtitle {
            for identifier in subtitle_identifiers(result) {
                match self.download_subtitle(identifier.clone()).await {
                    Ok(raw) => {
                        return Ok(FetchedLyric {
                            input_format: InputFormat::Lrc,
                            raw,
                            document: None,
                            annotations: Vec::new(),
                        });
                    }
                    Err(err) => errors.push(format!("subtitle {}: {err}", identifier.name)),
                }
            }
        }

        for identifier in lyric_identifiers(result) {
            match self.download_plain_lyrics(identifier.clone()).await {
                Ok(raw) => {
                    return Ok(FetchedLyric {
                        input_format: InputFormat::Text,
                        raw,
                        document: None,
                        annotations: Vec::new(),
                    });
                }
                Err(err) => errors.push(format!("lyrics {}: {err}", identifier.name)),
            }
        }

        if extra_bool(&result.extra, "restricted").unwrap_or(false) {
            errors.push("candidate is marked restricted by Musixmatch".to_string());
        }

        Err(Error::Provider(format!(
            "Musixmatch download failed: {}",
            errors.join("; ")
        )))
    }
}

#[derive(Debug, Deserialize)]
#[serde(bound(deserialize = "T: Deserialize<'de>"))]
struct MusixmatchEnvelope<T> {
    message: MusixmatchMessage<T>,
}

#[derive(Debug, Deserialize)]
#[serde(bound(deserialize = "T: Deserialize<'de>"))]
struct MusixmatchMessage<T> {
    #[serde(default)]
    header: MusixmatchHeader,
    #[serde(default)]
    body: Option<T>,
}

#[derive(Debug, Default, Deserialize)]
struct MusixmatchHeader {
    #[serde(default)]
    status_code: i32,
}

#[derive(Debug, Default, Deserialize)]
struct MusixmatchSearchBody {
    #[serde(default)]
    track_list: Vec<MusixmatchTrackItem>,
}

#[derive(Debug, Deserialize)]
struct MusixmatchTrackItem {
    track: MusixmatchTrack,
}

#[derive(Debug, Deserialize)]
struct MusixmatchTrack {
    #[serde(default)]
    track_id: Option<u64>,
    #[serde(default)]
    commontrack_id: Option<u64>,
    #[serde(default)]
    track_name: Option<String>,
    #[serde(default)]
    artist_name: Option<String>,
    #[serde(default)]
    album_name: Option<String>,
    #[serde(default)]
    track_length: Option<u32>,
    #[serde(default)]
    track_isrc: Option<String>,
    #[serde(default)]
    track_spotify_id: Option<String>,
    #[serde(default)]
    has_lyrics: Option<u8>,
    #[serde(default)]
    has_subtitles: Option<u8>,
    #[serde(default)]
    restricted: Option<u8>,
}

#[derive(Debug, Default, Deserialize)]
struct MusixmatchSubtitleBody {
    #[serde(default)]
    subtitle: Option<MusixmatchSubtitle>,
}

#[derive(Debug, Deserialize)]
struct MusixmatchSubtitle {
    #[serde(default)]
    subtitle_body: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct MusixmatchLyricsBody {
    #[serde(default)]
    lyrics: Option<MusixmatchLyrics>,
}

#[derive(Debug, Deserialize)]
struct MusixmatchLyrics {
    #[serde(default)]
    lyrics_body: Option<String>,
    #[serde(default)]
    lyrics_copyright: Option<String>,
}

#[derive(Debug, Clone)]
struct Identifier {
    name: &'static str,
    value: String,
}

fn subtitle_identifiers(result: &SearchResult) -> Vec<Identifier> {
    let mut identifiers = Vec::new();
    push_identifier(
        &mut identifiers,
        "track_id",
        extra_string(&result.extra, "track_id"),
    );
    push_identifier(
        &mut identifiers,
        "commontrack_id",
        extra_string(&result.extra, "commontrack_id"),
    );
    push_identifier(
        &mut identifiers,
        "track_isrc",
        extra_string(&result.extra, "track_isrc"),
    );
    push_identifier(
        &mut identifiers,
        "track_spotify_id",
        extra_string(&result.extra, "track_spotify_id"),
    );
    push_identifier(
        &mut identifiers,
        "track_itunes_id",
        extra_string(&result.extra, "track_itunes_id"),
    );
    identifiers
}

fn lyric_identifiers(result: &SearchResult) -> Vec<Identifier> {
    let mut identifiers = Vec::new();
    push_identifier(
        &mut identifiers,
        "commontrack_id",
        extra_string(&result.extra, "commontrack_id"),
    );
    push_identifier(
        &mut identifiers,
        "track_isrc",
        extra_string(&result.extra, "track_isrc"),
    );
    push_identifier(
        &mut identifiers,
        "track_spotify_id",
        extra_string(&result.extra, "track_spotify_id"),
    );
    push_identifier(
        &mut identifiers,
        "track_itunes_id",
        extra_string(&result.extra, "track_itunes_id"),
    );
    identifiers
}

fn push_identifier(identifiers: &mut Vec<Identifier>, name: &'static str, value: Option<String>) {
    if let Some(value) = value.filter(|value| !value.trim().is_empty()) {
        identifiers.push(Identifier { name, value });
    }
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
}

fn extra_bool(value: &Value, key: &str) -> Option<bool> {
    let value = value.get(key)?;
    value
        .as_bool()
        .or_else(|| value.as_u64().map(|number| number != 0))
        .or_else(|| value.as_i64().map(|number| number != 0))
}

fn ensure_trailing_newline(mut value: String) -> String {
    while value.ends_with('\0') {
        value.pop();
    }
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

    fn provider(server: &MockServer) -> MusixmatchProvider {
        MusixmatchProvider::with_api_base_url(
            Some("key-1".into()),
            format!("{}/ws/1.1", server.uri()),
            12_000,
        )
        .unwrap()
    }

    #[test]
    fn requires_api_key() {
        let err = match MusixmatchProvider::with_api_base_url(None, "http://localhost", 12_000) {
            Ok(_) => panic!("provider should require an API key"),
            Err(err) => err.to_string(),
        };

        assert!(err.contains("API key"));
        assert!(err.contains("ROSETTRISM_MUSIXMATCH_API_KEY"));
    }

    #[tokio::test]
    async fn searches_and_fetches_subtitle_lrc() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/ws/1.1/track.search"))
            .and(query_param("apikey", "key-1"))
            .and(query_param("q", "Song Artist"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "message": {
                    "header": { "status_code": 200 },
                    "body": {
                        "track_list": [{
                            "track": {
                                "track_id": 10,
                                "commontrack_id": 20,
                                "track_name": "Song",
                                "artist_name": "Artist",
                                "album_name": "Album",
                                "track_length": 1,
                                "track_isrc": "ISRC",
                                "has_lyrics": 1,
                                "has_subtitles": 1,
                                "restricted": 0
                            }
                        }]
                    }
                }
            })))
            .mount(&server)
            .await;

        Mock::given(method("GET"))
            .and(path("/ws/1.1/track.subtitle.get"))
            .and(query_param("track_id", "10"))
            .and(query_param("subtitle_format", "lrc"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "message": {
                    "header": { "status_code": 200 },
                    "body": {
                        "subtitle": {
                            "subtitle_body": "[00:01.00]Hi"
                        }
                    }
                }
            })))
            .mount(&server)
            .await;

        let provider = provider(&server);
        let results = provider.search("Song Artist").await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].source, Source::Musixmatch);
        assert_eq!(results[0].duration_ms, Some(1_000));

        let fetched = provider.fetch(&results[0]).await.unwrap();
        assert_eq!(fetched.input_format, InputFormat::Lrc);
        assert_eq!(String::from_utf8(fetched.raw).unwrap(), "[00:01.00]Hi\n");
    }

    #[tokio::test]
    async fn fetch_falls_back_to_plain_lyrics() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/ws/1.1/track.lyrics.get"))
            .and(query_param("commontrack_id", "20"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "message": {
                    "header": { "status_code": 200 },
                    "body": {
                        "lyrics": {
                            "lyrics_body": "Hi\nthere"
                        }
                    }
                }
            })))
            .mount(&server)
            .await;

        let provider = provider(&server);
        let result = SearchResult {
            source: Source::Musixmatch,
            id: "10".into(),
            title: "Song".into(),
            artist: "Artist".into(),
            album: None,
            duration_ms: None,
            extra: json!({
                "track_id": 10,
                "commontrack_id": 20,
                "has_subtitles": false
            }),
        };

        let fetched = provider.fetch(&result).await.unwrap();
        assert_eq!(fetched.input_format, InputFormat::Text);
        assert_eq!(String::from_utf8(fetched.raw).unwrap(), "Hi\nthere\n");
    }

    #[tokio::test]
    async fn restricted_or_empty_lyrics_get_readable_error() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/ws/1.1/track.lyrics.get"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "message": {
                    "header": { "status_code": 200 },
                    "body": {
                        "lyrics": {
                            "lyrics_body": "",
                            "lyrics_copyright": "Unfortunately we're not authorized to show these lyrics."
                        }
                    }
                }
            })))
            .mount(&server)
            .await;

        let provider = provider(&server);
        let result = SearchResult {
            source: Source::Musixmatch,
            id: "10".into(),
            title: "Song".into(),
            artist: "Artist".into(),
            album: None,
            duration_ms: None,
            extra: json!({
                "commontrack_id": 20,
                "has_subtitles": false,
                "restricted": true
            }),
        };

        let err = provider.fetch(&result).await.unwrap_err().to_string();
        assert!(err.contains("restricted") || err.contains("not authorized"));
    }
}
