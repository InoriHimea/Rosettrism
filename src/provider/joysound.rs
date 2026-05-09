use std::collections::HashSet;
use std::time::Duration;

use async_trait::async_trait;
use regex::Regex;
use reqwest::header::{HeaderMap, HeaderValue, ACCEPT, COOKIE, USER_AGENT};
use scraper::{Html, Selector};
use serde::Deserialize;
use serde_json::json;

use crate::decoder::text;
use crate::decoder::InputFormat;
use crate::model::LyricMeta;
use crate::provider::{FetchedLyric, LyricProvider, SearchResult, Source};
use crate::{Error, Result};

const USER_AGENT_VALUE: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36";
const DETAIL_MARKER: &str = "\u{3053}\u{306e}\u{66f2}\u{306e}\u{6b4c}\u{8a5e}";
const DETAIL_END_MARKER: &str = "\u{6b4c}\u{8a5e}\u{3092}\u{3059}\u{3079}\u{3066}\u{898b}\u{308b}";
const DETAIL_INFO_MARKER: &str = "\u{697d}\u{66f2}\u{60c5}\u{5831}";
const DETAIL_GIFT_MARKER: &str = "\u{6b4c}\u{8a5e}\u{3092}\u{8d08}\u{308b}";

pub struct JoysoundProvider {
    client: reqwest::Client,
    base_url: String,
    lyric_api_url: String,
}

impl JoysoundProvider {
    pub fn new(cookie: Option<String>) -> Result<Self> {
        Self::with_urls(
            cookie,
            "https://www.joysound.com",
            "https://mspxy.joysound.com/Common/Lyric",
        )
    }

    #[cfg(test)]
    fn with_base_url(cookie: Option<String>, base_url: impl Into<String>) -> Result<Self> {
        let base_url = trim_trailing_slash(base_url.into());
        let lyric_api_url = format!("{base_url}/Common/Lyric");
        Self::with_urls(cookie, base_url, lyric_api_url)
    }

    fn with_urls(
        cookie: Option<String>,
        base_url: impl Into<String>,
        lyric_api_url: impl Into<String>,
    ) -> Result<Self> {
        let mut headers = HeaderMap::new();
        headers.insert(USER_AGENT, HeaderValue::from_static(USER_AGENT_VALUE));
        headers.insert(
            ACCEPT,
            HeaderValue::from_static("text/html,application/json,text/plain,*/*"),
        );
        if let Some(cookie) = cookie {
            if !cookie.trim().is_empty() {
                headers.insert(
                    COOKIE,
                    HeaderValue::from_str(cookie.trim())
                        .map_err(|err| Error::Provider(format!("invalid cookie header: {err}")))?,
                );
            }
        }

        let client = reqwest::Client::builder()
            .default_headers(headers)
            .cookie_store(true)
            .timeout(Duration::from_secs(12))
            .build()?;

        Ok(Self {
            client,
            base_url: trim_trailing_slash(base_url.into()),
            lyric_api_url: lyric_api_url.into(),
        })
    }

    async fn search_page(&self, query: &str) -> Result<Vec<SearchResult>> {
        if let Some(id) = parse_direct_song_id(query) {
            return Ok(vec![self.direct_result(id)]);
        }

        let html = self
            .client
            .get(format!("{}/web/search/song", self.base_url))
            .query(&[("keyword", query)])
            .send()
            .await?
            .error_for_status()?
            .text()
            .await?;

        Ok(parse_search_results(&html, &self.base_url))
    }

    async fn download_text(&self, id: &str) -> Result<Vec<u8>> {
        let html = self
            .client
            .get(self.song_url(id))
            .send()
            .await?
            .error_for_status()?
            .text()
            .await?;

        let text = extract_lyrics_text(&html)
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| Error::Provider("JOYSOUND lyric text was not found".into()))?;

        Ok(ensure_trailing_newline(text).into_bytes())
    }

    async fn download_json_lyric(&self, id: &str) -> Result<FetchedLyric> {
        let response = self
            .client
            .post(&self.lyric_api_url)
            .header("X-JSP-APP-NAME", "0000800")
            .form(&[
                ("kind", "naviGroupId"),
                ("selSongNo", id),
                ("interactionFlg", "0"),
                ("apiVer", "1.0"),
            ])
            .send()
            .await?
            .error_for_status()?
            .json::<JoysoundLyricResponse>()
            .await?;
        let lyric = response
            .lyric_list
            .into_iter()
            .filter_map(|entry| entry.lyric)
            .find(|value| !value.trim().is_empty())
            .ok_or_else(|| Error::Provider("JOYSOUND JSON lyric response was empty".into()))?;
        let raw = ensure_trailing_newline(lyric).into_bytes();
        let mut document = text::parse(&String::from_utf8_lossy(&raw))?;
        document.meta = LyricMeta {
            title: response.song_name.filter(|value| !value.trim().is_empty()),
            artist: response
                .artist_name
                .filter(|value| !value.trim().is_empty()),
            source: Some("joysound".into()),
            ..Default::default()
        };

        Ok(FetchedLyric {
            input_format: InputFormat::Text,
            raw,
            document: Some(document),
            annotations: Vec::new(),
        })
    }

    fn direct_result(&self, id: String) -> SearchResult {
        let url = self.song_url(&id);
        SearchResult {
            source: Source::Joysound,
            id: id.clone(),
            title: format!("JOYSOUND {id}"),
            artist: String::new(),
            album: None,
            duration_ms: None,
            extra: json!({
                "song_id": id,
                "url": url
            }),
        }
    }

    fn song_url(&self, id: &str) -> String {
        format!("{}/web/search/song/{id}", self.base_url)
    }
}

#[async_trait]
impl LyricProvider for JoysoundProvider {
    async fn search(&self, query: &str) -> Result<Vec<SearchResult>> {
        self.search_page(query).await
    }

    async fn fetch(&self, result: &SearchResult) -> Result<FetchedLyric> {
        let id = result
            .extra
            .get("song_id")
            .and_then(|value| value.as_str())
            .unwrap_or(result.id.as_str());

        let mut errors = Vec::new();
        match self.download_json_lyric(id).await {
            Ok(fetched) => return Ok(fetched),
            Err(err) => errors.push(format!("json: {err}")),
        }

        match self.download_text(id).await {
            Ok(raw) => Ok(FetchedLyric {
                input_format: InputFormat::Text,
                raw,
                document: None,
                annotations: Vec::new(),
            }),
            Err(err) => {
                errors.push(format!("html: {err}"));
                Err(Error::Provider(format!(
                    "JOYSOUND download failed: {}",
                    errors.join("; ")
                )))
            }
        }
    }
}

#[derive(Debug, Default, Deserialize)]
struct JoysoundLyricResponse {
    #[serde(default, rename = "songName")]
    song_name: Option<String>,
    #[serde(default, rename = "artistName")]
    artist_name: Option<String>,
    #[serde(default, rename = "lyricList")]
    lyric_list: Vec<JoysoundLyricEntry>,
}

#[derive(Debug, Deserialize)]
struct JoysoundLyricEntry {
    #[serde(default)]
    lyric: Option<String>,
}

fn parse_search_results(html: &str, base_url: &str) -> Vec<SearchResult> {
    let document = Html::parse_document(html);
    let link_selector = selector("a[href]");
    let mut seen = HashSet::new();

    document
        .select(&link_selector)
        .filter_map(|link| {
            let href = link.value().attr("href")?;
            let id = parse_direct_song_id(href)?;
            if !seen.insert(id.clone()) {
                return None;
            }

            let title = normalize_inline(link.text().collect::<Vec<_>>().join(""));
            Some(SearchResult {
                source: Source::Joysound,
                id: id.clone(),
                title: if title.is_empty() {
                    format!("JOYSOUND {id}")
                } else {
                    title
                },
                artist: String::new(),
                album: None,
                duration_ms: None,
                extra: json!({
                    "song_id": id,
                    "url": resolve_url(base_url, href)
                }),
            })
        })
        .collect()
}

fn extract_lyrics_text(html: &str) -> Option<String> {
    let lines = html_to_lines(html);
    if let Some(index) = lines.iter().position(|line| line.contains(DETAIL_MARKER)) {
        let mut collected = Vec::new();
        for line in lines.iter().skip(index + 1) {
            if line.contains(DETAIL_END_MARKER)
                || line == DETAIL_INFO_MARKER
                || line == "DATA"
                || line.starts_with("## ")
            {
                break;
            }
            if line == DETAIL_GIFT_MARKER {
                continue;
            }
            collected.push(line.clone());
        }

        let text = collected.join("\n");
        if !text.trim().is_empty() {
            return Some(text);
        }
    }

    extract_by_selector(html)
}

fn extract_by_selector(html: &str) -> Option<String> {
    let document = Html::parse_document(html);
    for css in [".songDetailLyric", ".lyric", "#lyric"] {
        let selector = selector(css);
        if let Some(element) = document.select(&selector).next() {
            let text = html_fragment_to_text(&element.inner_html());
            if !text.trim().is_empty() {
                return Some(text);
            }
        }
    }

    None
}

fn html_to_lines(html: &str) -> Vec<String> {
    html_fragment_to_text(html)
        .lines()
        .map(ToOwned::to_owned)
        .collect()
}

fn html_fragment_to_text(html: &str) -> String {
    let hidden_re = Regex::new(
        r#"(?is)<[^>]*(?:style\s*=\s*["'][^"']*display\s*:\s*none|class\s*=\s*["'][^"']*(?:hidden|d-none|hide|none)[^"']*)[^>]*>.*?</[^>]+>"#,
    )
    .expect("static hidden element regex should compile");
    let br_re = Regex::new(r"(?i)<br\s*/?>").expect("static br regex should compile");
    let block_re =
        Regex::new(r"(?i)</(?:p|div|li|tr|h[1-6])>").expect("static block regex should compile");

    let mut cleaned = html.to_string();
    loop {
        let next = hidden_re.replace_all(&cleaned, "").to_string();
        if next == cleaned {
            break;
        }
        cleaned = next;
    }

    let cleaned = br_re.replace_all(&cleaned, "\n");
    let cleaned = block_re.replace_all(&cleaned, "\n");
    let fragment = Html::parse_fragment(&cleaned);
    normalize_preserve_lines(fragment.root_element().text().collect::<Vec<_>>().join(""))
}

fn parse_direct_song_id(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if !trimmed.is_empty() && trimmed.chars().all(|ch| ch.is_ascii_digit()) {
        return Some(trimmed.to_string());
    }

    let re = Regex::new(r"(?:^|/)web/search/song/(\d+)(?:[/?#]|$)").ok()?;
    re.captures(trimmed)
        .and_then(|capture| capture.get(1))
        .map(|value| value.as_str().to_string())
}

fn ensure_trailing_newline(mut value: String) -> String {
    if !value.ends_with('\n') {
        value.push('\n');
    }
    value
}

fn normalize_inline(value: String) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn normalize_preserve_lines(value: String) -> String {
    value
        .replace("\r\n", "\n")
        .replace('\r', "\n")
        .lines()
        .map(|line| normalize_inline(line.to_string()))
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
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

    format!("{}/{}", trim_trailing_slash(base_url.to_string()), value)
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

fn selector(css: &str) -> Selector {
    Selector::parse(css).expect("static selector should parse")
}

#[cfg(test)]
mod tests {
    use wiremock::matchers::{body_string_contains, header, method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::*;

    fn provider(server: &MockServer) -> JoysoundProvider {
        JoysoundProvider::with_base_url(None, server.uri()).unwrap()
    }

    #[test]
    fn parses_search_results_and_deduplicates_links() {
        let html = r#"
            <ol>
              <li><a href="/web/search/song/1027580">Song Artist</a></li>
              <li><a href="/web/search/song/1027580">lyrics</a></li>
              <li><a href="https://www.joysound.com/web/search/song/1114148">Other Song</a></li>
            </ol>
        "#;

        let results = parse_search_results(html, "https://www.joysound.com");
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].source, Source::Joysound);
        assert_eq!(results[0].id, "1027580");
        assert_eq!(results[0].title, "Song Artist");
        assert_eq!(
            results[0].extra["url"],
            "https://www.joysound.com/web/search/song/1027580"
        );
    }

    #[tokio::test]
    async fn supports_direct_id_and_url_queries() {
        let server = MockServer::start().await;
        let provider = provider(&server);

        let by_id = provider.search("1027580").await.unwrap();
        assert_eq!(by_id[0].id, "1027580");

        let by_url = provider
            .search("https://www.joysound.com/web/search/song/1114148")
            .await
            .unwrap();
        assert_eq!(by_url[0].id, "1114148");
    }

    #[tokio::test]
    async fn fetches_public_json_lyric_with_metadata() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/Common/Lyric"))
            .and(header("x-jsp-app-name", "0000800"))
            .and(body_string_contains("selSongNo=1027580"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "songName": "Song",
                "artistName": "Artist",
                "lyricList": [{
                    "lyric": "line one\nline two"
                }]
            })))
            .mount(&server)
            .await;

        let result = SearchResult {
            source: Source::Joysound,
            id: "1027580".into(),
            title: "Song".into(),
            artist: String::new(),
            album: None,
            duration_ms: None,
            extra: json!({ "song_id": "1027580" }),
        };

        let fetched = provider(&server).fetch(&result).await.unwrap();
        assert_eq!(fetched.input_format, InputFormat::Text);
        assert_eq!(
            String::from_utf8(fetched.raw).unwrap(),
            "line one\nline two\n"
        );
        let document = fetched.document.unwrap();
        assert_eq!(document.meta.title.as_deref(), Some("Song"));
        assert_eq!(document.meta.artist.as_deref(), Some("Artist"));
    }

    #[tokio::test]
    async fn falls_back_to_public_detail_text_when_json_is_empty() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/Common/Lyric"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "lyricList": []
            })))
            .mount(&server)
            .await;

        Mock::given(method("GET"))
            .and(path("/web/search/song/1027580"))
            .respond_with(ResponseTemplate::new(200).set_body_string(format!(
                "<div>{DETAIL_MARKER}</div><p>{DETAIL_GIFT_MARKER}</p><div>line one<br>line two</div><div>{DETAIL_END_MARKER}</div>"
            )))
            .mount(&server)
            .await;

        let result = SearchResult {
            source: Source::Joysound,
            id: "1027580".into(),
            title: "Song".into(),
            artist: String::new(),
            album: None,
            duration_ms: None,
            extra: json!({ "song_id": "1027580" }),
        };

        let fetched = provider(&server).fetch(&result).await.unwrap();
        assert_eq!(fetched.document, None);
        assert_eq!(
            String::from_utf8(fetched.raw).unwrap(),
            "line one\nline two\n"
        );
    }

    #[tokio::test]
    async fn missing_lyrics_get_readable_error() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/Common/Lyric"))
            .respond_with(ResponseTemplate::new(500).set_body_string("sad"))
            .mount(&server)
            .await;

        Mock::given(method("GET"))
            .and(path("/web/search/song/1027580"))
            .respond_with(ResponseTemplate::new(200).set_body_string("<html></html>"))
            .mount(&server)
            .await;

        let result = SearchResult {
            source: Source::Joysound,
            id: "1027580".into(),
            title: "Song".into(),
            artist: String::new(),
            album: None,
            duration_ms: None,
            extra: json!({ "song_id": "1027580" }),
        };

        let err = provider(&server)
            .fetch(&result)
            .await
            .unwrap_err()
            .to_string();
        assert!(err.contains("JOYSOUND download failed"));
        assert!(err.contains("json:"));
        assert!(err.contains("html:"));
    }

    #[tokio::test]
    async fn searches_public_web_page() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/web/search/song"))
            .and(query_param("keyword", "Song"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_string(r#"<a href="/web/search/song/1027580">Song Artist</a>"#),
            )
            .mount(&server)
            .await;

        let results = provider(&server).search("Song").await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, "1027580");
    }
}
