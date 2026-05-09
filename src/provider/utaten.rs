use std::collections::HashSet;
use std::time::Duration;

use async_trait::async_trait;
use regex::Regex;
use reqwest::header::{HeaderMap, HeaderValue, ACCEPT, COOKIE, USER_AGENT};
use scraper::{CaseSensitivity, ElementRef, Html, Node, Selector};
use serde_json::json;

use crate::decoder::InputFormat;
use crate::model::{LyricDocument, LyricLine, LyricMeta, LyricRubySpan};
use crate::provider::{FetchedLyric, LyricProvider, SearchResult, Source};
use crate::{Error, Result};

const USER_AGENT_VALUE: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36";

pub struct UtatenProvider {
    client: reqwest::Client,
    base_url: String,
}

impl UtatenProvider {
    pub fn new(cookie: Option<String>) -> Result<Self> {
        Self::with_base_url(cookie, "https://utaten.com")
    }

    fn with_base_url(cookie: Option<String>, base_url: impl Into<String>) -> Result<Self> {
        let mut headers = HeaderMap::new();
        headers.insert(USER_AGENT, HeaderValue::from_static(USER_AGENT_VALUE));
        headers.insert(
            ACCEPT,
            HeaderValue::from_static("text/html,application/xhtml+xml,text/plain,*/*"),
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
            return Ok(vec![self.direct_result(id)]);
        }

        let html = self
            .client
            .get(format!("{}/lyric/search", self.base_url))
            .query(&[("title", query), ("sort", "popular_sort_asc")])
            .send()
            .await?
            .error_for_status()?
            .text()
            .await?;

        Ok(parse_search_results(&html, &self.base_url))
    }

    async fn download_lyrics(&self, id: &str, result: &SearchResult) -> Result<FetchedLyric> {
        let html = self
            .client
            .get(self.lyric_url(id))
            .send()
            .await?
            .error_for_status()?
            .text()
            .await?;

        let mut document = extract_lyrics_document(&html)
            .ok_or_else(|| Error::Provider("UtaTen lyric text was not found".into()))?;
        document.meta = LyricMeta {
            title: if result.title.trim().is_empty() || result.title.starts_with("UtaTen ") {
                None
            } else {
                Some(result.title.clone())
            },
            artist: if result.artist.trim().is_empty() {
                None
            } else {
                Some(result.artist.clone())
            },
            source: Some("utaten".into()),
            ..Default::default()
        };
        let text = document
            .lines
            .iter()
            .map(|line| line.text.as_str())
            .collect::<Vec<_>>()
            .join("\n");

        Ok(FetchedLyric {
            input_format: InputFormat::Text,
            raw: ensure_trailing_newline(text).into_bytes(),
            document: Some(document),
            annotations: Vec::new(),
        })
    }

    fn direct_result(&self, id: String) -> SearchResult {
        let url = self.lyric_url(&id);
        SearchResult {
            source: Source::Utaten,
            id: id.clone(),
            title: format!("UtaTen {id}"),
            artist: String::new(),
            album: None,
            duration_ms: None,
            extra: json!({
                "lyrics_id": id,
                "url": url
            }),
        }
    }

    fn lyric_url(&self, id: &str) -> String {
        format!("{}/lyric/{id}/", self.base_url)
    }
}

#[async_trait]
impl LyricProvider for UtatenProvider {
    async fn search(&self, query: &str) -> Result<Vec<SearchResult>> {
        self.search_page(query).await
    }

    async fn fetch(&self, result: &SearchResult) -> Result<FetchedLyric> {
        let id = result
            .extra
            .get("lyrics_id")
            .and_then(|value| value.as_str())
            .unwrap_or(result.id.as_str());

        self.download_lyrics(id, result).await
    }
}

fn parse_search_results(html: &str, base_url: &str) -> Vec<SearchResult> {
    let document = Html::parse_document(html);
    let link_selector = selector("a[href]");
    let mut seen = HashSet::new();

    document
        .select(&link_selector)
        .filter_map(|link| {
            let href = link.value().attr("href")?;
            let id = parse_direct_lyric_id(href)?;
            if !seen.insert(id.clone()) {
                return None;
            }

            let title = normalize_inline(link.text().collect::<Vec<_>>().join(""));
            Some(SearchResult {
                source: Source::Utaten,
                id: id.clone(),
                title: if title.is_empty() {
                    format!("UtaTen {id}")
                } else {
                    title
                },
                artist: String::new(),
                album: None,
                duration_ms: None,
                extra: json!({
                    "lyrics_id": id,
                    "url": resolve_url(base_url, href)
                }),
            })
        })
        .collect()
}

fn extract_lyrics_document(html: &str) -> Option<LyricDocument> {
    let document = Html::parse_document(html);
    for css in [".hiragana", ".lyricBody", "#lyrics"] {
        let selector = selector(css);
        if let Some(element) = document.select(&selector).next() {
            if css == ".hiragana" {
                if let Some(document) = element_to_ruby_document(element) {
                    return Some(document);
                }
            }

            let text = html_fragment_to_text(&element.inner_html());
            if let Some(document) = text_to_document(&text) {
                return Some(document);
            }
        }
    }

    None
}

#[derive(Default)]
struct RubyLine {
    text: String,
    ruby: Vec<LyricRubySpan>,
    reading: String,
}

#[derive(Default)]
struct RubyParseState {
    lines: Vec<RubyLine>,
    current: RubyLine,
}

impl RubyParseState {
    fn push_text(&mut self, value: &str) {
        let text = normalize_inline(value.to_string());
        if !text.is_empty() {
            self.current.text.push_str(&text);
            self.current.reading.push_str(&text);
        }
    }

    fn push_ruby(&mut self, text: String, reading: String) {
        if text.trim().is_empty() {
            return;
        }

        let start_char = self.current.text.chars().count() as u32;
        self.current.text.push_str(&text);
        let end_char = self.current.text.chars().count() as u32;
        if !reading.trim().is_empty() {
            self.current.ruby.push(LyricRubySpan {
                start_char,
                end_char,
                text,
                reading,
            });
            self.current.reading.push_str(
                &self
                    .current
                    .ruby
                    .last()
                    .expect("ruby was just pushed")
                    .reading,
            );
        } else {
            self.current.reading.push_str(&text);
        }
    }

    fn finish_line(&mut self) {
        if !self.current.text.trim().is_empty() {
            self.lines.push(std::mem::take(&mut self.current));
        }
    }

    fn into_document(mut self) -> Option<LyricDocument> {
        self.finish_line();
        if self.lines.is_empty() {
            return None;
        }

        Some(LyricDocument {
            meta: LyricMeta::default(),
            lines: self
                .lines
                .into_iter()
                .map(|line| LyricLine {
                    start_ms: 0,
                    duration_ms: None,
                    reading: if !line.ruby.is_empty()
                        && !line.reading.trim().is_empty()
                        && line.reading != line.text
                    {
                        Some(line.reading)
                    } else {
                        None
                    },
                    text: line.text,
                    words: Vec::new(),
                    ruby: line.ruby,
                    romanized: None,
                })
                .collect(),
        })
    }
}

fn element_to_ruby_document(element: ElementRef<'_>) -> Option<LyricDocument> {
    let mut state = RubyParseState::default();
    for child in element.children() {
        append_node(child, &mut state);
    }
    state.into_document()
}

fn append_node(node: ego_tree::NodeRef<'_, Node>, state: &mut RubyParseState) {
    match node.value() {
        Node::Text(text) => state.push_text(text),
        Node::Element(element) => {
            let Some(element_ref) = ElementRef::wrap(node) else {
                return;
            };
            if element_is_hidden(element_ref) {
                return;
            }
            if element.name() == "br" {
                state.finish_line();
                return;
            }
            if element.has_class("kanji", CaseSensitivity::AsciiCaseInsensitive) {
                append_kanji(element_ref, state);
                return;
            }

            for child in element_ref.children() {
                append_node(child, state);
            }
        }
        _ => {}
    }
}

fn append_kanji(element: ElementRef<'_>, state: &mut RubyParseState) {
    let children = element.child_elements().collect::<Vec<_>>();
    if children.len() >= 2 {
        let reading = selected_node_text(children[0]);
        let text = selected_node_text(children[1]);
        if !text.is_empty() {
            state.push_ruby(text, reading);
            return;
        }
    }

    let rt_selector = selector("rt");
    let reading = element
        .select(&rt_selector)
        .map(selected_node_text)
        .collect::<Vec<_>>()
        .join("");
    let all_text = selected_node_text(element);
    if !reading.is_empty() && all_text.ends_with(&reading) {
        let text = all_text.trim_end_matches(&reading).to_string();
        state.push_ruby(text, reading);
    } else {
        state.push_text(&all_text);
    }
}

fn selected_node_text(element: ElementRef<'_>) -> String {
    normalize_inline(element.text().collect::<Vec<_>>().join(""))
}

fn element_is_hidden(element: ElementRef<'_>) -> bool {
    if let Some(style) = element.attr("style") {
        if style.split(';').any(|part| {
            part.split_whitespace()
                .collect::<String>()
                .eq_ignore_ascii_case("display:none")
        }) {
            return true;
        }
    }

    ["hidden", "d-none", "hide", "none"].iter().any(|class| {
        element
            .value()
            .has_class(class, CaseSensitivity::AsciiCaseInsensitive)
    })
}

fn text_to_document(text: &str) -> Option<LyricDocument> {
    let lines = text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(|line| LyricLine {
            start_ms: 0,
            duration_ms: None,
            text: line.to_string(),
            words: Vec::new(),
            ruby: Vec::new(),
            reading: None,
            romanized: None,
        })
        .collect::<Vec<_>>();

    if lines.is_empty() {
        None
    } else {
        Some(LyricDocument {
            meta: LyricMeta::default(),
            lines,
        })
    }
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

fn parse_direct_lyric_id(value: &str) -> Option<String> {
    let trimmed = value.trim();
    let id_re = Regex::new(r"^[a-z]{2}\d{8}$").ok()?;
    if id_re.is_match(trimmed) {
        return Some(trimmed.to_string());
    }

    let url_re = Regex::new(r"(?:^|/)lyric/([a-z]{2}\d{8})(?:[/?#]|$)").ok()?;
    url_re
        .captures(trimmed)
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
    use wiremock::matchers::{method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::*;

    fn provider(server: &MockServer) -> UtatenProvider {
        UtatenProvider::with_base_url(None, server.uri()).unwrap()
    }

    #[test]
    fn parses_search_results_and_deduplicates_links() {
        let html = r#"
            <section>
              <a href="/lyric/mi24041201/">Song</a>
              <a href="/lyric/mi24041201/">Song duplicate</a>
              <a href="https://utaten.com/lyric/ya17060751/">Other Song</a>
            </section>
        "#;

        let results = parse_search_results(html, "https://utaten.com");
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].source, Source::Utaten);
        assert_eq!(results[0].id, "mi24041201");
        assert_eq!(results[0].title, "Song");
        assert_eq!(
            results[0].extra["url"],
            "https://utaten.com/lyric/mi24041201/"
        );
    }

    #[tokio::test]
    async fn supports_direct_id_and_url_queries() {
        let server = MockServer::start().await;
        let provider = provider(&server);

        let by_id = provider.search("mi24041201").await.unwrap();
        assert_eq!(by_id[0].id, "mi24041201");

        let by_url = provider
            .search("https://utaten.com/lyric/ya17060751/")
            .await
            .unwrap();
        assert_eq!(by_url[0].id, "ya17060751");
    }

    #[tokio::test]
    async fn searches_and_fetches_hiragana_text() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/lyric/search"))
            .and(query_param("title", "Song"))
            .and(query_param("sort", "popular_sort_asc"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_string(r#"<a href="/lyric/mi24041201/">Song</a>"#),
            )
            .mount(&server)
            .await;

        Mock::given(method("GET"))
            .and(path("/lyric/mi24041201/"))
            .respond_with(ResponseTemplate::new(200).set_body_string(
                r#"<div class="hiragana">one<br><span class="d-none">junk</span>two<p style="display:none">bad</p></div>"#,
            ))
            .mount(&server)
            .await;

        let provider = provider(&server);
        let results = provider.search("Song").await.unwrap();
        let fetched = provider.fetch(&results[0]).await.unwrap();

        assert_eq!(fetched.input_format, InputFormat::Text);
        assert_eq!(String::from_utf8(fetched.raw).unwrap(), "one\ntwo\n");
        let document = fetched.document.unwrap();
        let json = serde_json::to_value(&document.lines[0]).unwrap();
        assert!(json.get("ruby").is_none());
    }

    #[tokio::test]
    async fn fetches_hiragana_with_ruby_document() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/lyric/mi24041201/"))
            .respond_with(ResponseTemplate::new(200).set_body_string(
                r#"<div class="hiragana"><span class="kanji"><span>hi</span><span>日</span></span>々<br><span style="display:none">bad</span>two</div>"#,
            ))
            .mount(&server)
            .await;

        let provider = provider(&server);
        let result = SearchResult {
            source: Source::Utaten,
            id: "mi24041201".into(),
            title: "Song".into(),
            artist: "Artist".into(),
            album: None,
            duration_ms: None,
            extra: json!({ "lyrics_id": "mi24041201" }),
        };

        let fetched = provider.fetch(&result).await.unwrap();
        assert_eq!(String::from_utf8(fetched.raw).unwrap(), "日々\ntwo\n");
        let document = fetched.document.unwrap();
        assert_eq!(document.meta.source.as_deref(), Some("utaten"));
        assert_eq!(document.lines[0].text, "日々");
        assert_eq!(document.lines[0].ruby.len(), 1);
        assert_eq!(document.lines[0].ruby[0].start_char, 0);
        assert_eq!(document.lines[0].ruby[0].end_char, 1);
        assert_eq!(document.lines[0].ruby[0].text, "日");
        assert_eq!(document.lines[0].ruby[0].reading, "hi");

        let lrc = crate::exporter::lrc::to_string(&document);
        assert!(lrc.contains("[00:00.00]日々"));
        assert!(!lrc.contains("hi"));
    }

    #[tokio::test]
    async fn missing_lyrics_get_readable_error() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/lyric/mi24041201/"))
            .respond_with(ResponseTemplate::new(200).set_body_string("<html></html>"))
            .mount(&server)
            .await;

        let result = SearchResult {
            source: Source::Utaten,
            id: "mi24041201".into(),
            title: "Song".into(),
            artist: String::new(),
            album: None,
            duration_ms: None,
            extra: json!({ "lyrics_id": "mi24041201" }),
        };

        let err = provider(&server)
            .fetch(&result)
            .await
            .unwrap_err()
            .to_string();
        assert!(err.contains("UtaTen lyric text was not found"));
    }
}
