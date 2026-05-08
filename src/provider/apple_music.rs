use std::collections::HashSet;
use std::sync::Mutex;
use std::time::Duration;

use async_trait::async_trait;
use base64::Engine;
use regex::Regex;
use reqwest::header::{
    HeaderMap, HeaderName, HeaderValue, ACCEPT, COOKIE, ORIGIN, REFERER, USER_AGENT,
};
use serde::de::DeserializeOwned;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::decoder::InputFormat;
use crate::provider::{FetchedLyric, LyricProvider, SearchResult, Source};
use crate::{Error, Result};

const DEFAULT_STOREFRONT: &str = "us";
const USER_AGENT_VALUE: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36";
const MEDIA_USER_TOKEN: HeaderName = HeaderName::from_static("media-user-token");

pub struct AppleMusicProvider {
    client: reqwest::Client,
    storefront: String,
    language: String,
    web_url: String,
    api_base_url: String,
    has_media_user_token: bool,
    developer_token: Mutex<Option<String>>,
}

impl AppleMusicProvider {
    pub fn new(cookie: Option<String>) -> Result<Self> {
        let storefront = env_var_any(&[
            "ROSETTRISM_APPLE_MUSIC_STOREFRONT",
            "LRC_DECODE_APPLE_MUSIC_STOREFRONT",
        ])
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| DEFAULT_STOREFRONT.to_string());
        let developer_token = env_var_any(&[
            "ROSETTRISM_APPLE_MUSIC_DEVELOPER_TOKEN",
            "LRC_DECODE_APPLE_MUSIC_DEVELOPER_TOKEN",
        ])
        .filter(|value| !value.trim().is_empty());
        let language = env_var_any(&[
            "ROSETTRISM_APPLE_MUSIC_LANGUAGE",
            "LRC_DECODE_APPLE_MUSIC_LANGUAGE",
        ])
        .unwrap_or_default();

        Self::with_endpoints(
            cookie,
            storefront,
            language,
            "https://music.apple.com/us/browse",
            "https://amp-api.music.apple.com",
            developer_token,
        )
    }

    fn with_endpoints(
        auth: Option<String>,
        storefront: impl Into<String>,
        language: impl Into<String>,
        web_url: impl Into<String>,
        api_base_url: impl Into<String>,
        developer_token: Option<String>,
    ) -> Result<Self> {
        let mut headers = HeaderMap::new();
        headers.insert(USER_AGENT, HeaderValue::from_static(USER_AGENT_VALUE));
        headers.insert(
            ACCEPT,
            HeaderValue::from_static("application/json, text/plain, */*"),
        );
        headers.insert(ORIGIN, HeaderValue::from_static("https://music.apple.com"));
        headers.insert(
            REFERER,
            HeaderValue::from_static("https://music.apple.com/"),
        );

        let mut has_media_user_token = false;
        if let Some(auth) = auth
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            let media_user_token = extract_media_user_token(auth);
            let cookie = append_media_user_token_cookie(
                extract_cookie_header(auth),
                media_user_token.as_deref(),
            );

            if let Some(media_user_token) = media_user_token {
                has_media_user_token = true;
                headers.insert(
                    MEDIA_USER_TOKEN,
                    HeaderValue::from_str(&media_user_token).map_err(|err| {
                        Error::Provider(format!("invalid media-user-token header: {err}"))
                    })?,
                );
            }

            if let Some(cookie) = cookie {
                headers.insert(
                    COOKIE,
                    HeaderValue::from_str(&cookie)
                        .map_err(|err| Error::Provider(format!("invalid cookie header: {err}")))?,
                );
            }
        }

        let client = reqwest::Client::builder()
            .default_headers(headers)
            .timeout(Duration::from_secs(12))
            .build()?;

        Ok(Self {
            client,
            storefront: normalize_storefront(storefront),
            language: language.into().trim().to_string(),
            web_url: web_url.into(),
            api_base_url: trim_trailing_slash(api_base_url.into()),
            has_media_user_token,
            developer_token: Mutex::new(developer_token.map(|value| value.trim().to_string())),
        })
    }

    async fn developer_token(&self) -> Result<String> {
        if let Some(token) = self.cached_developer_token()? {
            return Ok(token);
        }

        let token = self.fetch_developer_token().await?;
        let mut cached = self
            .developer_token
            .lock()
            .map_err(|_| Error::Provider("Apple Music developer token cache is poisoned".into()))?;
        *cached = Some(token.clone());

        Ok(token)
    }

    fn cached_developer_token(&self) -> Result<Option<String>> {
        Ok(self
            .developer_token
            .lock()
            .map_err(|_| Error::Provider("Apple Music developer token cache is poisoned".into()))?
            .clone()
            .filter(|value| !value.trim().is_empty()))
    }

    async fn fetch_developer_token(&self) -> Result<String> {
        let html = self
            .client
            .get(&self.web_url)
            .send()
            .await?
            .error_for_status()?
            .text()
            .await?;

        if let Some(token) = extract_developer_token(&html) {
            return Ok(token);
        }

        let script_urls = extract_script_urls(&html, &self.web_url);
        let mut errors = Vec::new();

        for script_url in script_urls.into_iter().take(40) {
            match self.client.get(&script_url).send().await {
                Ok(response) => match response.error_for_status() {
                    Ok(response) => match response.text().await {
                        Ok(script) => {
                            if let Some(token) = extract_developer_token(&script) {
                                return Ok(token);
                            }
                        }
                        Err(err) => errors.push(format!("{script_url}: {err}")),
                    },
                    Err(err) => errors.push(format!("{script_url}: {err}")),
                },
                Err(err) => errors.push(format!("{script_url}: {err}")),
            }
        }

        let detail = if errors.is_empty() {
            String::new()
        } else {
            format!(" Checked scripts: {}", errors.join("; "))
        };
        Err(Error::Provider(format!(
            "Apple Music developer token was not found in the web player.{detail}"
        )))
    }

    async fn send_json<T>(&self, request: reqwest::RequestBuilder, context: &str) -> Result<T>
    where
        T: DeserializeOwned,
    {
        let token = self.developer_token().await?;
        let response = request.bearer_auth(token).send().await?;
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
                "{context} failed with HTTP {status}{detail}"
            )));
        }

        Ok(serde_json::from_str(&body)?)
    }

    async fn search_catalog(&self, query: &str) -> Result<Vec<SearchResult>> {
        let storefront = self.storefront.as_str();
        let url = format!("{}/v1/catalog/{storefront}/search", self.api_base_url);
        let response = self
            .send_json::<AppleSearchResponse>(
                self.client.get(url).query(&[
                    ("term", query),
                    ("types", "songs"),
                    ("limit", "20"),
                    ("platform", "web"),
                ]),
                "Apple Music search",
            )
            .await?;

        Ok(response
            .results
            .and_then(|results| results.songs)
            .map(|songs| songs.data)
            .unwrap_or_default()
            .into_iter()
            .map(|song| {
                let attributes = song.attributes.unwrap_or_default();
                let title = attributes.name.unwrap_or_else(|| song.id.clone());
                let artist = attributes.artist_name.unwrap_or_default();
                let album = attributes.album_name;
                let duration_ms = attributes.duration_in_millis;

                SearchResult {
                    source: Source::AppleMusic,
                    id: song.id.clone(),
                    title,
                    artist,
                    album,
                    duration_ms,
                    extra: json!({
                        "songid": song.id,
                        "storefront": storefront
                    }),
                }
            })
            .collect())
    }

    async fn download_ttml(&self, song_id: &str, storefront: &str) -> Result<Vec<u8>> {
        let mut errors = Vec::new();

        for lyric_kind in ["lyrics", "syllable-lyrics"] {
            match self
                .download_ttml_from_endpoint(song_id, storefront, lyric_kind)
                .await
            {
                Ok(ttml) => return Ok(ttml),
                Err(err) => errors.push(format!("{lyric_kind}: {err}")),
            }
        }

        if !self.has_media_user_token {
            return Err(Error::Provider(format!(
                "Apple Music TTML lyrics require a Media-User-Token from a logged-in Apple Music subscriber. Set `ROSETTRISM_APPLE_MUSIC_COOKIE` (legacy: `LRC_DECODE_APPLE_MUSIC_COOKIE`) or `--cookie-file` to `media-user-token=...`. Tried endpoints: {}",
                errors.join("; ")
            )));
        }

        Err(Error::Provider(format!(
            "Apple Music lyric download failed: {}",
            errors.join("; ")
        )))
    }

    async fn download_ttml_from_endpoint(
        &self,
        song_id: &str,
        storefront: &str,
        lyric_kind: &str,
    ) -> Result<Vec<u8>> {
        let url = format!(
            "{}/v1/catalog/{}/songs/{}/{}",
            self.api_base_url, storefront, song_id, lyric_kind
        );
        let response = self
            .send_json::<AppleLyricsResponse>(
                self.client.get(url).query(&[
                    ("l", self.language.as_str()),
                    ("extend", "ttmlLocalizations"),
                ]),
                "Apple Music lyric",
            )
            .await?;

        let ttml = response
            .data
            .into_iter()
            .find_map(|item| item.attributes.and_then(AppleLyricsAttributes::into_ttml))
            .filter(|ttml| !ttml.trim().is_empty())
            .ok_or_else(|| {
                Error::Provider("Apple Music lyric response did not include TTML".into())
            })?;

        Ok(ttml.into_bytes())
    }
}

#[async_trait]
impl LyricProvider for AppleMusicProvider {
    async fn search(&self, query: &str) -> Result<Vec<SearchResult>> {
        if let Some(song) = parse_apple_music_song_query(query, &self.storefront) {
            return Ok(vec![SearchResult {
                source: Source::AppleMusic,
                id: song.id.clone(),
                title: format!("Apple Music Song {}", song.id),
                artist: String::new(),
                album: None,
                duration_ms: None,
                extra: json!({
                    "songid": song.id,
                    "storefront": song.storefront
                }),
            }]);
        }

        self.search_catalog(query).await
    }

    async fn fetch(&self, result: &SearchResult) -> Result<FetchedLyric> {
        let song_id = result
            .extra
            .get("songid")
            .and_then(Value::as_str)
            .unwrap_or(result.id.as_str());
        let storefront = result
            .extra
            .get("storefront")
            .and_then(Value::as_str)
            .unwrap_or(self.storefront.as_str());

        Ok(FetchedLyric {
            input_format: InputFormat::AppleMusic,
            raw: self.download_ttml(song_id, storefront).await?,
            document: None,
        })
    }
}

#[derive(Debug)]
struct AppleSongQuery {
    id: String,
    storefront: String,
}

fn parse_apple_music_song_query(value: &str, default_storefront: &str) -> Option<AppleSongQuery> {
    let value = value.trim();
    if !value.is_empty() && value.chars().all(|ch| ch.is_ascii_digit()) {
        return Some(AppleSongQuery {
            id: value.to_string(),
            storefront: default_storefront.to_string(),
        });
    }

    let storefront = storefront_from_url(value).unwrap_or_else(|| default_storefront.to_string());
    if let Some(id) = query_song_id(value) {
        return Some(AppleSongQuery { id, storefront });
    }

    if !value.contains("music.apple.com") || !value.contains("/song/") {
        return None;
    }

    let re = Regex::new(r"(?i)/song/[^?#\s]+/(\d+)").ok()?;
    let id = re.captures(value)?.get(1)?.as_str().to_string();

    Some(AppleSongQuery { id, storefront })
}

fn query_song_id(value: &str) -> Option<String> {
    let query = value
        .split_once('?')?
        .1
        .split('#')
        .next()
        .unwrap_or_default();

    query.split('&').find_map(|part| {
        let (key, value) = part.split_once('=')?;
        if key == "i" && !value.is_empty() && value.chars().all(|ch| ch.is_ascii_digit()) {
            Some(value.to_string())
        } else {
            None
        }
    })
}

fn storefront_from_url(value: &str) -> Option<String> {
    let marker = "music.apple.com/";
    let index = value.find(marker)? + marker.len();
    let storefront = value[index..]
        .split(['/', '?', '#'])
        .next()
        .unwrap_or_default();

    if storefront.is_empty() || storefront.contains('.') {
        None
    } else {
        Some(normalize_storefront(storefront))
    }
}

fn normalize_storefront(value: impl Into<String>) -> String {
    let value = value.into();
    let value = value.trim();
    if value.is_empty() {
        DEFAULT_STOREFRONT.to_string()
    } else {
        value.to_ascii_lowercase()
    }
}

fn env_var_any(names: &[&str]) -> Option<String> {
    names
        .iter()
        .find_map(|name| std::env::var(name).ok())
        .filter(|value| !value.trim().is_empty())
}

fn extract_media_user_token(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }

    for line in trimmed.lines() {
        let Some((name, value)) = line.split_once(':') else {
            continue;
        };
        let normalized = normalize_token_name(name);
        if is_media_user_token_name(&normalized) {
            return Some(value.trim().to_string());
        }
    }

    for part in trimmed.split(';') {
        let Some((name, value)) = part.split_once('=') else {
            continue;
        };
        let normalized = normalize_token_name(name);
        if is_media_user_token_name(&normalized)
            || (normalized.contains("media")
                && normalized.contains("user")
                && normalized.contains("token"))
        {
            return Some(value.trim().to_string());
        }
    }

    if !trimmed.contains('=') && looks_like_jwt(trimmed) {
        return Some(trimmed.to_string());
    }

    None
}

fn extract_cookie_header(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }

    for line in trimmed.lines() {
        let Some((name, value)) = line.split_once(':') else {
            continue;
        };
        if normalize_token_name(name) == "cookie" {
            let value = value.trim();
            if !value.is_empty() {
                return Some(value.to_string());
            }
        }
    }

    if trimmed.lines().count() == 1 && (trimmed.contains('=') || trimmed.contains(';')) {
        Some(trimmed.to_string())
    } else {
        None
    }
}

fn append_media_user_token_cookie(
    cookie: Option<String>,
    media_user_token: Option<&str>,
) -> Option<String> {
    let Some(media_user_token) = media_user_token else {
        return cookie;
    };

    let media_user_token = media_user_token.trim();
    if media_user_token.is_empty() {
        return cookie;
    }

    match cookie {
        Some(cookie) if cookie_has_name(&cookie, "media-user-token") => Some(cookie),
        Some(cookie) if cookie.trim().is_empty() => {
            Some(format!("media-user-token={media_user_token}"))
        }
        Some(cookie) => Some(format!(
            "{}; media-user-token={}",
            cookie.trim(),
            media_user_token
        )),
        None => Some(format!("media-user-token={media_user_token}")),
    }
}

fn cookie_has_name(cookie: &str, name: &str) -> bool {
    cookie.split(';').any(|part| {
        part.split_once('=')
            .map(|(key, _)| key.trim().eq_ignore_ascii_case(name))
            .unwrap_or(false)
    })
}

fn normalize_token_name(value: &str) -> String {
    value
        .trim()
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

fn is_media_user_token_name(value: &str) -> bool {
    matches!(
        value,
        "mediausertoken" | "musicusertoken" | "applemusicusertoken" | "mut"
    )
}

fn extract_developer_token(text: &str) -> Option<String> {
    let re =
        Regex::new(r"eyJ[A-Za-z0-9_-]{10,}\.eyJ[A-Za-z0-9_-]{10,}\.[A-Za-z0-9_-]{20,}").ok()?;
    let mut best: Option<(i32, usize, String)> = None;

    for token_match in re.find_iter(text) {
        let token = token_match.as_str();
        let score = developer_token_score(text, token_match.start(), token_match.end(), token);
        let candidate = (score, token.len(), token.to_string());

        if best
            .as_ref()
            .map(|best| candidate.0 > best.0 || (candidate.0 == best.0 && candidate.1 > best.1))
            .unwrap_or(true)
        {
            best = Some(candidate);
        }
    }

    best.map(|(_, _, token)| token)
}

fn developer_token_score(text: &str, start: usize, end: usize, token: &str) -> i32 {
    let window_start = start.saturating_sub(120);
    let window_end = end.saturating_add(120).min(text.len());
    let window =
        String::from_utf8_lossy(&text.as_bytes()[window_start..window_end]).to_ascii_lowercase();
    let mut score = 0;

    if window.contains("developertoken") || window.contains("developer-token") {
        score += 4;
    }
    if window.contains("musickit") {
        score += 2;
    }
    if decoded_jwt_part(token, 0)
        .as_deref()
        .is_some_and(|header| header.contains("ES256") || header.contains("alg"))
    {
        score += 1;
    }
    if decoded_jwt_part(token, 1)
        .as_deref()
        .is_some_and(|payload| {
            payload.contains("\"iss\"")
                && payload.contains("\"iat\"")
                && payload.contains("\"exp\"")
        })
    {
        score += 2;
    }

    score
}

fn looks_like_jwt(value: &str) -> bool {
    let parts = value.split('.').collect::<Vec<_>>();
    parts.len() == 3
        && parts.iter().all(|part| {
            !part.is_empty()
                && part
                    .chars()
                    .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_')
        })
}

fn decoded_jwt_part(token: &str, index: usize) -> Option<String> {
    let part = token.split('.').nth(index)?;
    let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(part)
        .ok()?;
    String::from_utf8(bytes).ok()
}

fn extract_script_urls(html: &str, base_url: &str) -> Vec<String> {
    let Ok(re) = Regex::new(r#"(?i)<script[^>]+src=["']([^"']+)["']"#) else {
        return Vec::new();
    };
    let mut seen = HashSet::new();
    let mut urls = Vec::new();

    for capture in re.captures_iter(html) {
        let Some(src) = capture.get(1) else {
            continue;
        };
        let url = resolve_url(base_url, src.as_str());
        if seen.insert(url.clone()) {
            urls.push(url);
        }
    }

    urls
}

fn resolve_url(base_url: &str, value: &str) -> String {
    if value.starts_with("http://") || value.starts_with("https://") {
        return value.to_string();
    }
    if let Some(value) = value.strip_prefix("//") {
        return format!("https://{value}");
    }
    if value.starts_with('/') {
        return format!("{}{}", origin(base_url), value);
    }

    let base = base_url.split('?').next().unwrap_or(base_url);
    let parent = base
        .rsplit_once('/')
        .map(|(parent, _)| parent)
        .unwrap_or(base);
    format!("{parent}/{value}")
}

fn origin(value: &str) -> &str {
    let Some(scheme_end) = value.find("://") else {
        return "";
    };
    let host_start = scheme_end + 3;
    let host_end = value[host_start..]
        .find('/')
        .map(|index| host_start + index)
        .unwrap_or(value.len());
    &value[..host_end]
}

fn trim_trailing_slash(mut value: String) -> String {
    while value.ends_with('/') {
        value.pop();
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

#[derive(Debug, Deserialize)]
struct AppleSearchResponse {
    #[serde(default)]
    results: Option<AppleSearchResults>,
}

#[derive(Debug, Deserialize)]
struct AppleSearchResults {
    #[serde(default)]
    songs: Option<AppleSongList>,
}

#[derive(Debug, Deserialize)]
struct AppleSongList {
    #[serde(default)]
    data: Vec<AppleCatalogSong>,
}

#[derive(Debug, Deserialize)]
struct AppleCatalogSong {
    id: String,
    #[serde(default)]
    attributes: Option<AppleSongAttributes>,
}

#[derive(Debug, Default, Deserialize)]
struct AppleSongAttributes {
    #[serde(default)]
    name: Option<String>,
    #[serde(default, alias = "artistName")]
    artist_name: Option<String>,
    #[serde(default, alias = "albumName")]
    album_name: Option<String>,
    #[serde(default, alias = "durationInMillis")]
    duration_in_millis: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct AppleLyricsResponse {
    #[serde(default)]
    data: Vec<AppleLyricsData>,
}

#[derive(Debug, Deserialize)]
struct AppleLyricsData {
    #[serde(default)]
    attributes: Option<AppleLyricsAttributes>,
}

#[derive(Debug, Deserialize)]
struct AppleLyricsAttributes {
    #[serde(default)]
    ttml: Option<String>,
    #[serde(default, alias = "ttmlLocalizations")]
    ttml_localizations: Option<String>,
}

impl AppleLyricsAttributes {
    fn into_ttml(self) -> Option<String> {
        self.ttml
            .filter(|value| !value.trim().is_empty())
            .or_else(|| {
                self.ttml_localizations
                    .filter(|value| !value.trim().is_empty())
            })
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use wiremock::matchers::{header, method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::*;

    const TOKEN: &str = "eyJhbGciOiJFUzI1NiJ9.eyJpc3MiOiJ0ZWFtIiwiaWF0IjoxLCJleHAiOjk5OTk5OTk5OTl9.signatureABCdef1234567890";

    #[tokio::test]
    async fn extracts_web_token_searches_and_fetches_ttml() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/browse"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_string(r#"<html><script src="/assets/app.js"></script></html>"#),
            )
            .mount(&server)
            .await;

        Mock::given(method("GET"))
            .and(path("/assets/app.js"))
            .respond_with(ResponseTemplate::new(200).set_body_string(format!(
                r#"MusicKit.configure({{ developerToken: "{TOKEN}" }});"#
            )))
            .mount(&server)
            .await;

        Mock::given(method("GET"))
            .and(path("/v1/catalog/us/search"))
            .and(query_param("term", "Song Artist"))
            .and(query_param("types", "songs"))
            .and(header("authorization", format!("Bearer {TOKEN}")))
            .and(header("media-user-token", "mut-1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "results": {
                    "songs": {
                        "data": [{
                            "id": "123",
                            "attributes": {
                                "name": "Song",
                                "artistName": "Artist",
                                "albumName": "Album",
                                "durationInMillis": 1000
                            }
                        }]
                    }
                }
            })))
            .mount(&server)
            .await;

        let ttml = r#"<tt xmlns="http://www.w3.org/ns/ttml"><body><div><p begin="00:01.000" end="00:02.000">Hi</p></div></body></tt>"#;
        Mock::given(method("GET"))
            .and(path("/v1/catalog/us/songs/123/lyrics"))
            .and(header("authorization", format!("Bearer {TOKEN}")))
            .and(header("media-user-token", "mut-1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "data": [{
                    "id": "123",
                    "type": "lyrics",
                    "attributes": {
                        "ttml": ttml
                    }
                }]
            })))
            .mount(&server)
            .await;

        let provider = AppleMusicProvider::with_endpoints(
            Some("media-user-token=mut-1".into()),
            "us",
            "",
            format!("{}/browse", server.uri()),
            server.uri(),
            None,
        )
        .unwrap();

        let results = provider.search("Song Artist").await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "Song");
        assert_eq!(results[0].artist, "Artist");

        let fetched = provider.fetch(&results[0]).await.unwrap();
        assert_eq!(fetched.input_format, InputFormat::AppleMusic);
        assert_eq!(String::from_utf8(fetched.raw).unwrap(), ttml);
    }

    #[tokio::test]
    async fn accepts_song_url_without_catalog_search() {
        let server = MockServer::start().await;
        let ttml = r#"<tt xmlns="http://www.w3.org/ns/ttml"><body><div><p begin="00:01.000">Hi</p></div></body></tt>"#;

        Mock::given(method("GET"))
            .and(path("/v1/catalog/jp/songs/456/lyrics"))
            .and(header("authorization", format!("Bearer {TOKEN}")))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "data": [{
                    "attributes": {
                        "ttml": ttml
                    }
                }]
            })))
            .mount(&server)
            .await;

        let provider = AppleMusicProvider::with_endpoints(
            None,
            "us",
            "",
            format!("{}/browse", server.uri()),
            server.uri(),
            Some(TOKEN.into()),
        )
        .unwrap();

        let results = provider
            .search("https://music.apple.com/jp/album/example/111?i=456")
            .await
            .unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, "456");
        assert_eq!(results[0].extra["storefront"], "jp");

        let fetched = provider.fetch(&results[0]).await.unwrap();
        assert_eq!(String::from_utf8(fetched.raw).unwrap(), ttml);
    }

    #[tokio::test]
    async fn falls_back_to_syllable_lyrics_and_ttml_localizations() {
        let server = MockServer::start().await;
        let ttml = r#"<tt xmlns="http://www.w3.org/ns/ttml"><body><div><p begin="00:01.000">Hi</p></div></body></tt>"#;

        Mock::given(method("GET"))
            .and(path("/v1/catalog/us/songs/123/lyrics"))
            .and(query_param("extend", "ttmlLocalizations"))
            .respond_with(ResponseTemplate::new(404).set_body_json(json!({
                "errors": [{
                    "status": "404",
                    "code": "40403"
                }]
            })))
            .mount(&server)
            .await;

        Mock::given(method("GET"))
            .and(path("/v1/catalog/us/songs/123/syllable-lyrics"))
            .and(query_param("extend", "ttmlLocalizations"))
            .and(header("cookie", "media-user-token=mut-1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "data": [{
                    "attributes": {
                        "ttmlLocalizations": ttml
                    }
                }]
            })))
            .mount(&server)
            .await;

        let provider = AppleMusicProvider::with_endpoints(
            Some("media-user-token=mut-1".into()),
            "us",
            "",
            format!("{}/browse", server.uri()),
            server.uri(),
            Some(TOKEN.into()),
        )
        .unwrap();
        let result = SearchResult {
            source: Source::AppleMusic,
            id: "123".into(),
            title: "Song".into(),
            artist: "Artist".into(),
            album: None,
            duration_ms: None,
            extra: json!({
                "songid": "123",
                "storefront": "us"
            }),
        };

        let fetched = provider.fetch(&result).await.unwrap();
        assert_eq!(String::from_utf8(fetched.raw).unwrap(), ttml);
    }

    #[tokio::test]
    async fn missing_media_user_token_gets_actionable_error() {
        let server = MockServer::start().await;

        for lyric_kind in ["lyrics", "syllable-lyrics"] {
            Mock::given(method("GET"))
                .and(path(format!("/v1/catalog/us/songs/123/{lyric_kind}")))
                .respond_with(ResponseTemplate::new(404).set_body_json(json!({
                    "errors": [{
                        "status": "404",
                        "code": "40403"
                    }]
                })))
                .mount(&server)
                .await;
        }

        let provider = AppleMusicProvider::with_endpoints(
            None,
            "us",
            "",
            format!("{}/browse", server.uri()),
            server.uri(),
            Some(TOKEN.into()),
        )
        .unwrap();
        let result = SearchResult {
            source: Source::AppleMusic,
            id: "123".into(),
            title: "Song".into(),
            artist: "Artist".into(),
            album: None,
            duration_ms: None,
            extra: json!({
                "songid": "123",
                "storefront": "us"
            }),
        };

        let err = provider.fetch(&result).await.unwrap_err();
        assert!(err.to_string().contains("Media-User-Token"));
        assert!(err.to_string().contains("ROSETTRISM_APPLE_MUSIC_COOKIE"));
    }

    #[test]
    fn parses_media_user_token_inputs() {
        assert_eq!(
            extract_media_user_token("media-user-token=abc; other=1").as_deref(),
            Some("abc")
        );
        assert_eq!(
            extract_media_user_token("Media-User-Token: def").as_deref(),
            Some("def")
        );
        assert_eq!(extract_media_user_token(TOKEN).as_deref(), Some(TOKEN));
        assert_eq!(extract_media_user_token("foo=bar"), None);
        assert_eq!(
            extract_cookie_header("Cookie: a=b; c=d\r\nMedia-User-Token: def").as_deref(),
            Some("a=b; c=d")
        );
        assert_eq!(
            append_media_user_token_cookie(None, Some("def")).as_deref(),
            Some("media-user-token=def")
        );
        assert_eq!(
            append_media_user_token_cookie(Some("a=b".into()), Some("def")).as_deref(),
            Some("a=b; media-user-token=def")
        );
    }

    #[test]
    fn parses_direct_song_queries() {
        let direct = parse_apple_music_song_query("123", "us").unwrap();
        assert_eq!(direct.id, "123");
        assert_eq!(direct.storefront, "us");

        let song_url =
            parse_apple_music_song_query("https://music.apple.com/tw/song/name/456", "us").unwrap();
        assert_eq!(song_url.id, "456");
        assert_eq!(song_url.storefront, "tw");

        let album_url =
            parse_apple_music_song_query("https://music.apple.com/jp/album/name/1?i=789", "us")
                .unwrap();
        assert_eq!(album_url.id, "789");
        assert_eq!(album_url.storefront, "jp");
    }
}
