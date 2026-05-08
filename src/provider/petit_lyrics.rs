use std::time::Duration;

use async_trait::async_trait;
use base64::Engine;
use regex::Regex;
use reqwest::header::{HeaderMap, HeaderValue, ACCEPT, COOKIE, USER_AGENT};
use scraper::{ElementRef, Html, Selector};
use serde::Deserialize;
use serde_json::json;

use crate::decoder::InputFormat;
use crate::provider::{FetchedLyric, LyricProvider, SearchResult, Source};
use crate::{Error, Result};

const USER_AGENT_VALUE: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36";

pub struct PetitLyricsProvider {
    client: reqwest::Client,
    base_url: String,
}

impl PetitLyricsProvider {
    pub fn new(cookie: Option<String>) -> Result<Self> {
        Self::with_base_url(cookie, "https://petitlyrics.com")
    }

    #[cfg(test)]
    fn with_base_url(cookie: Option<String>, base_url: impl Into<String>) -> Result<Self> {
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
        })
    }

    #[cfg(not(test))]
    fn with_base_url(cookie: Option<String>, base_url: impl Into<String>) -> Result<Self> {
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
        })
    }

    async fn search_page(&self, query: &str) -> Result<Vec<SearchResult>> {
        if let Some(id) = parse_direct_lyric_id(query) {
            return Ok(vec![SearchResult {
                source: Source::PetitLyrics,
                id: id.clone(),
                title: format!("PetitLyrics {id}"),
                artist: String::new(),
                album: None,
                duration_ms: None,
                extra: json!({
                    "lyrics_id": id,
                    "url": self.lyric_url(&id)
                }),
            }]);
        }

        let url = format!("{}/en/search_lyrics", self.base_url);
        let html = self
            .client
            .get(url)
            .query(&[("title", query)])
            .send()
            .await?
            .error_for_status()?
            .text()
            .await?;

        Ok(parse_search_results(&html, &self.base_url))
    }

    async fn download_text(&self, id: &str) -> Result<Vec<u8>> {
        let detail_url = self.lyric_url(id);
        let html = self
            .client
            .get(&detail_url)
            .send()
            .await?
            .error_for_status()?
            .text()
            .await?;

        match self.download_text_via_ajax(id, &detail_url, &html).await {
            Ok(raw) if !raw.is_empty() => Ok(raw),
            Ok(_) => fallback_canvas_text(&html)
                .map(|text| ensure_trailing_newline(text).into_bytes())
                .ok_or_else(|| Error::Provider("PetitLyrics lyric response was empty".into())),
            Err(ajax_err) => fallback_canvas_text(&html)
                .map(|text| ensure_trailing_newline(text).into_bytes())
                .ok_or_else(|| {
                    Error::Provider(format!(
                        "PetitLyrics AJAX lyric fetch failed and no canvas fallback was found: {ajax_err}"
                    ))
                }),
        }
    }

    async fn download_text_via_ajax(
        &self,
        id: &str,
        detail_url: &str,
        html: &str,
    ) -> Result<Vec<u8>> {
        let csrf_script = extract_csrf_script_url(html)
            .ok_or_else(|| Error::Provider("PetitLyrics CSRF script was not found".into()))?;
        let csrf_script = resolve_url(&self.base_url, &csrf_script);
        let script = self
            .client
            .get(csrf_script)
            .send()
            .await?
            .error_for_status()?
            .text()
            .await?;
        let csrf_token = extract_csrf_token(&script)
            .ok_or_else(|| Error::Provider("PetitLyrics CSRF token was not found".into()))?;
        let ajax_url = format!("{}/com/get_lyrics.ajax", self.base_url);
        let response = self
            .client
            .post(ajax_url)
            .header("X-Requested-With", "XMLHttpRequest")
            .header("Referer", detail_url)
            .header("Origin", origin(&self.base_url))
            .header("X-CSRF-Token", csrf_token)
            .form(&[("lyrics_id", id)])
            .send()
            .await?;
        let status = response.status();
        let body = response.text().await?;
        if !status.is_success() {
            return Err(Error::Provider(format!(
                "PetitLyrics AJAX failed with HTTP {status}: {}",
                response_preview(&body)
            )));
        }

        let lines = serde_json::from_str::<Vec<PetitLyricsAjaxLine>>(&body)?;
        let mut decoded = Vec::new();
        for line in lines {
            if line.lyrics.trim().is_empty() {
                continue;
            }
            let bytes = base64::engine::general_purpose::STANDARD
                .decode(line.lyrics.trim())
                .map_err(|err| Error::Provider(format!("invalid PetitLyrics base64: {err}")))?;
            decoded.push(String::from_utf8_lossy(&bytes).to_string());
        }

        if decoded.is_empty() {
            return Err(Error::Provider(
                "PetitLyrics AJAX response did not include lyric lines".into(),
            ));
        }

        Ok(format_plain_lines(decoded).into_bytes())
    }

    fn lyric_url(&self, id: &str) -> String {
        format!("{}/en/lyrics/{id}", self.base_url)
    }
}

#[async_trait]
impl LyricProvider for PetitLyricsProvider {
    async fn search(&self, query: &str) -> Result<Vec<SearchResult>> {
        self.search_page(query).await
    }

    async fn fetch(&self, result: &SearchResult) -> Result<FetchedLyric> {
        let id = result
            .extra
            .get("lyrics_id")
            .and_then(|value| value.as_str())
            .unwrap_or(result.id.as_str());

        Ok(FetchedLyric {
            input_format: InputFormat::Text,
            raw: self.download_text(id).await?,
            document: None,
        })
    }
}

#[derive(Debug, Deserialize)]
struct PetitLyricsAjaxLine {
    lyrics: String,
}

fn parse_search_results(html: &str, base_url: &str) -> Vec<SearchResult> {
    let document = Html::parse_document(html);
    let row_selector = selector("#lyrics_list tr");

    document
        .select(&row_selector)
        .filter_map(|row| parse_search_row(row, base_url))
        .collect()
}

fn parse_search_row(row: ElementRef<'_>, base_url: &str) -> Option<SearchResult> {
    let id = lyric_id_from_row(row)?;
    let title = selected_text(row, ".lyrics-list-title").unwrap_or_else(|| format!("Lyrics {id}"));
    let artist = selected_text(row, ".lyrics-list-artist").unwrap_or_default();
    let album = selected_text(row, ".lyrics-list-album").filter(|value| !value.is_empty());
    let sync_type = selected_text(row, ".lyrics-list-sync");
    let posted_date = selected_texts(row, ".lyrics-list-other").into_iter().next();

    Some(SearchResult {
        source: Source::PetitLyrics,
        id: id.clone(),
        title,
        artist,
        album,
        duration_ms: None,
        extra: json!({
            "lyrics_id": id,
            "url": format!("{}/en/lyrics/{}", trim_trailing_slash(base_url.to_string()), id),
            "sync_type": sync_type,
            "posted_date": posted_date
        }),
    })
}

fn lyric_id_from_row(row: ElementRef<'_>) -> Option<String> {
    let link_selector = selector("a[href]");
    row.select(&link_selector).find_map(|link| {
        let href = link.value().attr("href")?;
        parse_direct_lyric_id(href)
    })
}

fn selected_text(row: ElementRef<'_>, css: &str) -> Option<String> {
    selected_texts(row, css).into_iter().next()
}

fn selected_texts(row: ElementRef<'_>, css: &str) -> Vec<String> {
    let selector = selector(css);
    row.select(&selector)
        .map(|element| normalize_text(element.text().collect::<Vec<_>>().join("")))
        .filter(|value| !value.is_empty())
        .collect()
}

fn fallback_canvas_text(html: &str) -> Option<String> {
    let document = Html::parse_document(html);
    let canvas_selector = selector("canvas#lyrics");
    document.select(&canvas_selector).find_map(|canvas| {
        let text = normalize_preserve_lines(canvas.text().collect::<Vec<_>>().join(""));
        if text.trim().is_empty() {
            None
        } else {
            Some(text)
        }
    })
}

fn extract_csrf_script_url(html: &str) -> Option<String> {
    let re = Regex::new(r#"(?i)<script[^>]+src=["']([^"']*/lib/pl-lib\.js\?[^"']*)["']"#).ok()?;
    re.captures(html)
        .and_then(|capture| capture.get(1))
        .map(|value| value.as_str().to_string())
}

fn extract_csrf_token(script: &str) -> Option<String> {
    let re = Regex::new(r#"X-CSRF-Token',\s*'([^']+)'"#).ok()?;
    re.captures(script)
        .and_then(|capture| capture.get(1))
        .map(|value| value.as_str().to_string())
}

fn parse_direct_lyric_id(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if !trimmed.is_empty() && trimmed.chars().all(|ch| ch.is_ascii_digit()) {
        return Some(trimmed.to_string());
    }

    let re = Regex::new(r#"(?:^|/)lyrics/(\d+)(?:[/?#]|$)"#).ok()?;
    re.captures(trimmed)
        .and_then(|capture| capture.get(1))
        .map(|value| value.as_str().to_string())
}

fn format_plain_lines(lines: Vec<String>) -> String {
    ensure_trailing_newline(lines.join("\n"))
}

fn ensure_trailing_newline(mut value: String) -> String {
    if !value.ends_with('\n') {
        value.push('\n');
    }
    value
}

fn normalize_text(value: String) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn normalize_preserve_lines(value: String) -> String {
    let lines = value
        .replace("\r\n", "\n")
        .replace('\r', "\n")
        .lines()
        .map(str::trim_end)
        .collect::<Vec<_>>()
        .join("\n");
    lines.trim_matches('\n').to_string()
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

fn selector(css: &str) -> Selector {
    Selector::parse(css).expect("static selector should parse")
}

#[cfg(test)]
mod tests {
    use base64::Engine;
    use serde_json::json;
    use wiremock::matchers::{body_string_contains, header, method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::*;

    fn provider(server: &MockServer) -> PetitLyricsProvider {
        PetitLyricsProvider::with_base_url(None, server.uri()).unwrap()
    }

    #[test]
    fn parses_search_results() {
        let html = r#"
            <table id="lyrics_list">
              <tr>
                <td>
                  <a href="/lyrics/123"><span class="lyrics-list-title">Song</span></a><br>
                  <span class="lyrics-list-artist">Artist</span>
                  <span class="lyrics-list-album">Album</span>
                  <span class="lyrics-list-other">2024/01/01</span>
                  <span class="lyrics-list-sync text_sync">Word Sync</span>
                </td>
              </tr>
            </table>
        "#;

        let results = parse_search_results(html, "https://petitlyrics.com");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].source, Source::PetitLyrics);
        assert_eq!(results[0].id, "123");
        assert_eq!(results[0].title, "Song");
        assert_eq!(results[0].artist, "Artist");
        assert_eq!(results[0].album.as_deref(), Some("Album"));
        assert_eq!(results[0].extra["sync_type"], "Word Sync");
    }

    #[tokio::test]
    async fn supports_direct_id_and_url_queries() {
        let server = MockServer::start().await;
        let provider = provider(&server);

        let by_id = provider.search("123").await.unwrap();
        assert_eq!(by_id[0].id, "123");

        let by_url = provider
            .search("https://petitlyrics.com/en/lyrics/456")
            .await
            .unwrap();
        assert_eq!(by_url[0].id, "456");
    }

    #[tokio::test]
    async fn searches_and_fetches_ajax_lyrics() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/en/search_lyrics"))
            .and(query_param("title", "Song"))
            .respond_with(ResponseTemplate::new(200).set_body_string(
                r#"<table id="lyrics_list"><tr><td><a href="/lyrics/123"><span class="lyrics-list-title">Song</span></a><span class="lyrics-list-artist">Artist</span></td></tr></table>"#,
            ))
            .mount(&server)
            .await;

        Mock::given(method("GET"))
            .and(path("/en/lyrics/123"))
            .respond_with(ResponseTemplate::new(200).set_body_string(
                r#"<html><head><script src="/lib/pl-lib.js?1"></script></head><body><canvas id="lyrics">fallback</canvas></body></html>"#,
            ))
            .mount(&server)
            .await;

        Mock::given(method("GET"))
            .and(path("/lib/pl-lib.js"))
            .respond_with(ResponseTemplate::new(200).set_body_string(
                "$(document).ajaxSend(function(event, jqxhr) { jqxhr.setRequestHeader('X-CSRF-Token', 'token-1'); });",
            ))
            .mount(&server)
            .await;

        let line = base64::engine::general_purpose::STANDARD.encode("Hi");
        Mock::given(method("POST"))
            .and(path("/com/get_lyrics.ajax"))
            .and(header("x-csrf-token", "token-1"))
            .and(body_string_contains("lyrics_id=123"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!([
                { "lyrics": line }
            ])))
            .mount(&server)
            .await;

        let provider = provider(&server);
        let results = provider.search("Song").await.unwrap();
        let fetched = provider.fetch(&results[0]).await.unwrap();

        assert_eq!(fetched.input_format, InputFormat::Text);
        assert_eq!(String::from_utf8(fetched.raw).unwrap(), "Hi\n");
    }

    #[tokio::test]
    async fn falls_back_to_canvas_text_when_ajax_fails() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/en/lyrics/123"))
            .respond_with(ResponseTemplate::new(200).set_body_string(
                r#"<html><head></head><body><canvas id="lyrics">one
two</canvas></body></html>"#,
            ))
            .mount(&server)
            .await;

        let provider = provider(&server);
        let result = SearchResult {
            source: Source::PetitLyrics,
            id: "123".into(),
            title: "Song".into(),
            artist: "Artist".into(),
            album: None,
            duration_ms: None,
            extra: json!({
                "lyrics_id": "123"
            }),
        };

        let fetched = provider.fetch(&result).await.unwrap();
        assert_eq!(String::from_utf8(fetched.raw).unwrap(), "one\ntwo\n");
    }
}
