use async_trait::async_trait;
use encoding_rs::SHIFT_JIS;
use regex::Regex;
use reqwest::header::{HeaderMap, HeaderValue, ACCEPT, COOKIE, USER_AGENT};
use scraper::{Html, Selector};
use serde_json::{json, Value};

use crate::decoder::{text, InputFormat};
use crate::model::{LyricDocument, LyricMeta};
use crate::provider::{FetchedLyric, LyricProvider, SearchResult, Source};
use crate::{Error, Result};

const USER_AGENT_VALUE: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36";
const COMMON_TEXT_SELECTORS: &[&str] = &[
    "[data-lyrics-container='true']",
    "[data-lyrics-container=\"true\"]",
    ".lyrics",
    ".lyric",
    ".song-lyrics",
    ".songLyrics",
    ".lyricBody",
    ".lyrics-body",
    "#lyrics",
    "#lyric",
    "#kashi",
    "#kasi",
    ".hiragana",
];
const COMMON_TITLE_SELECTORS: &[&str] = &[
    "h1",
    ".title",
    ".song-title",
    ".track-title",
    "[itemprop='name']",
];
const COMMON_ARTIST_SELECTORS: &[&str] = &[
    ".artist",
    ".artist-name",
    ".singer",
    ".song-artist",
    "[itemprop='byArtist']",
];

pub fn provider_for(
    source: Source,
    cookie: Option<String>,
    timeout_ms: u64,
) -> Result<Box<dyn LyricProvider>> {
    if source == Source::LineMusic {
        return Ok(Box::new(LineMusicProvider::new(cookie, timeout_ms)?));
    }

    Ok(Box::new(PublicWebProvider::new(
        cookie,
        config_for(source)?,
        timeout_ms,
    )?))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DecodeMode {
    Utf8,
    ShiftJis,
}

#[derive(Debug, Clone)]
struct WebSourceConfig {
    source: Source,
    key: &'static str,
    display_name: &'static str,
    base_url: String,
    path_template: &'static str,
    id_regex: Option<&'static str>,
    text_selectors: &'static [&'static str],
    title_selectors: &'static [&'static str],
    artist_selectors: &'static [&'static str],
    reading_selectors: &'static [&'static str],
    romanized_selectors: &'static [&'static str],
    decode: DecodeMode,
    use_json_ld: bool,
    use_next_data: bool,
}

impl WebSourceConfig {
    #[cfg(test)]
    fn with_base_url(mut self, base_url: impl Into<String>) -> Self {
        self.base_url = trim_trailing_slash(base_url.into());
        self
    }

    fn build_url(&self, id: &str) -> String {
        format!(
            "{}{}",
            self.base_url,
            self.path_template
                .replace("{id}", id.trim_start_matches('/'))
        )
    }
}

struct PublicWebProvider {
    client: reqwest::Client,
    config: WebSourceConfig,
}

impl PublicWebProvider {
    fn new(cookie: Option<String>, config: WebSourceConfig, timeout_ms: u64) -> Result<Self> {
        let client = web_client(cookie, timeout_ms)?;
        Ok(Self { client, config })
    }

    fn direct_result(&self, id: String, url: String) -> SearchResult {
        SearchResult {
            source: self.config.source,
            id: id.clone(),
            title: format!("{} {id}", self.config.display_name),
            artist: String::new(),
            album: None,
            duration_ms: None,
            extra: json!({
                "url": url,
                "direct": true,
            }),
        }
    }

    async fn search_direct(&self, query: &str) -> Result<Vec<SearchResult>> {
        let trimmed = query.trim();
        if is_http_url(trimmed) {
            let id = url_id(trimmed);
            return Ok(vec![self.direct_result(id, trimmed.to_string())]);
        }

        if let Some(id) = parse_direct_id(trimmed, self.config.id_regex)? {
            let url = self.config.build_url(&id);
            return Ok(vec![self.direct_result(id, url)]);
        }

        Err(Error::Provider(format!(
            "{} supports direct URL or id only; pass a lyric page URL or `id:<page-id>`",
            self.config.display_name
        )))
    }

    async fn fetch_page(&self, result: &SearchResult) -> Result<FetchedLyric> {
        let url = result
            .extra
            .get("url")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned)
            .unwrap_or_else(|| self.config.build_url(&result.id));
        let response = self.client.get(url).send().await?;
        let status = response.status();
        let bytes = response.bytes().await?;
        if !status.is_success() {
            let detail = if status.as_u16() == 403 {
                "blocked or forbidden"
            } else {
                "request failed"
            };
            return Err(Error::Provider(format!(
                "{} fetch {detail} with HTTP {status}",
                self.config.display_name
            )));
        }

        let html = decode_bytes(&bytes, self.config.decode);
        let mut document = extract_document(&self.config, &html)?;
        if document.meta.title.is_none()
            && !result.title.trim().is_empty()
            && !result.title.starts_with(self.config.display_name)
        {
            document.meta.title = Some(result.title.clone());
        }
        let raw = document_text(&document).into_bytes();

        Ok(FetchedLyric {
            input_format: InputFormat::Text,
            raw,
            document: Some(document),
            annotations: Vec::new(),
        })
    }
}

#[async_trait]
impl LyricProvider for PublicWebProvider {
    async fn search(&self, query: &str) -> Result<Vec<SearchResult>> {
        self.search_direct(query).await
    }

    async fn fetch(&self, result: &SearchResult) -> Result<FetchedLyric> {
        self.fetch_page(result).await
    }
}

struct LineMusicProvider {
    client: reqwest::Client,
    base_url: String,
}

impl LineMusicProvider {
    pub fn new(cookie: Option<String>, timeout_ms: u64) -> Result<Self> {
        Self::with_base_url(cookie, "https://music.line.me", timeout_ms)
    }

    fn with_base_url(
        cookie: Option<String>,
        base_url: impl Into<String>,
        timeout_ms: u64,
    ) -> Result<Self> {
        Ok(Self {
            client: web_client(cookie, timeout_ms)?,
            base_url: trim_trailing_slash(base_url.into()),
        })
    }

    fn direct_result(&self, id: String) -> SearchResult {
        SearchResult {
            source: Source::LineMusic,
            id: id.clone(),
            title: format!("LINE MUSIC {id}"),
            artist: String::new(),
            album: None,
            duration_ms: None,
            extra: json!({
                "track_id": id,
                "url": format!("{}/webapp/track/{}", self.base_url, id)
            }),
        }
    }

    async fn search_direct(&self, query: &str) -> Result<Vec<SearchResult>> {
        let Some(id) = parse_line_music_id(query)? else {
            return Err(Error::Provider(
                "LINE MUSIC supports direct track URL or id only".into(),
            ));
        };

        Ok(vec![self.direct_result(id)])
    }

    async fn send_json(&self, path: &str) -> Result<Value> {
        let response = self
            .client
            .get(format!("{}{}", self.base_url, path))
            .send()
            .await?;
        let status = response.status();
        let body = response.text().await?;
        if !status.is_success() {
            return Err(Error::Provider(format!(
                "LINE MUSIC request failed with HTTP {status}: {}",
                response_preview(&body)
            )));
        }

        Ok(serde_json::from_str(&body)?)
    }

    async fn fetch_track(&self, result: &SearchResult) -> Result<FetchedLyric> {
        let id = result
            .extra
            .get("track_id")
            .and_then(Value::as_str)
            .unwrap_or(result.id.as_str());
        let metadata = self
            .send_json(&format!("/api2/tracks/{id}.v1"))
            .await
            .unwrap_or(Value::Null);
        let lyric_json = self
            .send_json(&format!("/api2/track/{id}/lyrics.v1"))
            .await?;
        let lyric = json_find_string(
            &lyric_json,
            &["lyrics", "lyric", "lyricsText", "lyricText", "text"],
        )
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| Error::Provider("LINE MUSIC lyric response was empty".into()))?;

        let mut document = text_to_document("line-music", &lyric)?;
        document.meta.title = json_find_string(&metadata, &["songName", "trackName", "title"]);
        document.meta.artist = json_find_string(&metadata, &["artistName", "artist", "singer"]);
        let raw = ensure_trailing_newline(lyric).into_bytes();

        Ok(FetchedLyric {
            input_format: InputFormat::Text,
            raw,
            document: Some(document),
            annotations: Vec::new(),
        })
    }
}

#[async_trait]
impl LyricProvider for LineMusicProvider {
    async fn search(&self, query: &str) -> Result<Vec<SearchResult>> {
        self.search_direct(query).await
    }

    async fn fetch(&self, result: &SearchResult) -> Result<FetchedLyric> {
        self.fetch_track(result).await
    }
}

fn config_for(source: Source) -> Result<WebSourceConfig> {
    let mut config = match source {
        Source::Animesongz => config(
            source,
            "animesongz",
            "Animesongz",
            "https://animesongz.com",
            "/lyrics/{id}",
        ),
        Source::Awa => config(source, "awa", "AWA", "https://s.awa.fm", "/track/{id}"),
        Source::Azlyrics => config(
            source,
            "azlyrics",
            "AZLyrics",
            "https://www.azlyrics.com",
            "/lyrics/{id}.html",
        ),
        Source::Genius => config(
            source,
            "genius",
            "Genius",
            "https://genius.com",
            "/songs/{id}",
        ),
        Source::JLyric => config(
            source,
            "j-lyric",
            "J-Lyric",
            "https://j-lyric.net",
            "/artist/{id}.html",
        ),
        Source::JTotal => config(
            source,
            "j-total",
            "J-Total",
            "https://j-total.net",
            "/data/{id}.html",
        ),
        Source::Kashinavi => config(
            source,
            "kashinavi",
            "Kashinavi",
            "https://kashinavi.com",
            "/song_view.html?{id}",
        ),
        Source::Kkbox => config(
            source,
            "kkbox",
            "KKBOX",
            "https://www.kkbox.com",
            "/tw/tc/song/{id}",
        ),
        Source::LyricalNonsense => config(
            source,
            "lyrical-nonsense",
            "Lyrical Nonsense",
            "https://www.lyrical-nonsense.com",
            "/lyrics/{id}/",
        ),
        Source::RockLyric => config(
            source,
            "rocklyric",
            "RockLyric",
            "https://rocklyric.jp",
            "/lyric/{id}",
        ),
        Source::Songtexte => config(
            source,
            "songtexte",
            "Songtexte",
            "https://www.songtexte.com",
            "/songtext/{id}.html",
        ),
        Source::TuneCore => config(
            source,
            "tunecore",
            "TuneCore",
            "https://linkco.re",
            "/{id}/songs",
        ),
        Source::UtaNet => config(
            source,
            "uta-net",
            "Uta-Net",
            "https://www.uta-net.com",
            "/song/{id}/",
        ),
        Source::Utamap => config(
            source,
            "utamap",
            "UtaMap",
            "https://www.utamap.com",
            "/showkasi.php?surl={id}",
        ),
        _ => {
            return Err(Error::Provider(format!(
                "{} is not a public web lyric source",
                source.cli_name()
            )))
        }
    };

    if matches!(
        source,
        Source::JLyric | Source::JTotal | Source::Kashinavi | Source::Utamap
    ) {
        config.decode = DecodeMode::ShiftJis;
    }
    if matches!(source, Source::Kkbox) {
        config.use_json_ld = true;
    }
    if matches!(
        source,
        Source::Awa | Source::TuneCore | Source::LyricalNonsense
    ) {
        config.use_next_data = true;
    }
    if matches!(source, Source::LyricalNonsense) {
        config.romanized_selectors = &[".lyrics-romaji", ".lyric-romaji", ".romaji"];
        config.reading_selectors = &[".lyrics-kana", ".lyric-kana", ".kana"];
    }
    if matches!(
        source,
        Source::UtaNet | Source::JTotal | Source::Kashinavi | Source::Utamap | Source::RockLyric
    ) {
        config.id_regex = Some(r"^[A-Za-z0-9_-]+$");
    }

    Ok(config)
}

fn config(
    source: Source,
    key: &'static str,
    display_name: &'static str,
    base_url: &'static str,
    path_template: &'static str,
) -> WebSourceConfig {
    WebSourceConfig {
        source,
        key,
        display_name,
        base_url: base_url.to_string(),
        path_template,
        id_regex: None,
        text_selectors: COMMON_TEXT_SELECTORS,
        title_selectors: COMMON_TITLE_SELECTORS,
        artist_selectors: COMMON_ARTIST_SELECTORS,
        reading_selectors: &[],
        romanized_selectors: &[],
        decode: DecodeMode::Utf8,
        use_json_ld: false,
        use_next_data: false,
    }
}

fn extract_document(config: &WebSourceConfig, html: &str) -> Result<LyricDocument> {
    let document = Html::parse_document(html);
    let json_ld = if config.use_json_ld {
        extract_json_ld(&document)
    } else {
        JsonLdExtract::default()
    };
    let next_text = if config.use_next_data {
        extract_next_data_text(&document)
    } else {
        None
    };
    let text = json_ld
        .lyric
        .clone()
        .or(next_text)
        .or_else(|| extract_by_selectors(&document, config.text_selectors))
        .or_else(|| extract_longest_visible_text(&document))
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| {
            Error::Provider(format!("{} lyric text was not found", config.display_name))
        })?;
    let mut lyric_document = text_to_document(config.key, &text)?;
    lyric_document.meta = LyricMeta {
        title: json_ld
            .title
            .or_else(|| extract_inline_by_selectors(&document, config.title_selectors)),
        artist: json_ld
            .artist
            .or_else(|| extract_inline_by_selectors(&document, config.artist_selectors)),
        source: Some(config.key.to_string()),
        ..Default::default()
    };
    apply_annotation_text(
        &mut lyric_document,
        extract_by_selectors(&document, config.reading_selectors).as_deref(),
        extract_by_selectors(&document, config.romanized_selectors).as_deref(),
    );

    Ok(lyric_document)
}

#[derive(Debug, Default)]
struct JsonLdExtract {
    lyric: Option<String>,
    title: Option<String>,
    artist: Option<String>,
}

fn extract_json_ld(document: &Html) -> JsonLdExtract {
    let mut extracted = JsonLdExtract::default();
    let selector = selector("script[type='application/ld+json']");
    for script in document.select(&selector) {
        let raw = script.text().collect::<Vec<_>>().join("");
        if raw.trim().is_empty() {
            continue;
        }
        if let Ok(value) = serde_json::from_str::<Value>(&raw) {
            extracted.lyric = extracted.lyric.or_else(|| json_find_lyrics(&value));
            extracted.title = extracted
                .title
                .or_else(|| json_find_string(&value, &["name", "title"]));
            extracted.artist = extracted.artist.or_else(|| json_find_artist_string(&value));
        }
    }

    extracted
}

fn extract_next_data_text(document: &Html) -> Option<String> {
    let selector = selector("script#__NEXT_DATA__");
    document.select(&selector).find_map(|script| {
        let raw = script.text().collect::<Vec<_>>().join("");
        let value = serde_json::from_str::<Value>(&raw).ok()?;
        json_find_string(
            &value,
            &["lyrics", "lyric", "plainLyrics", "lyricsText", "lyricText"],
        )
    })
}

fn extract_by_selectors(document: &Html, selectors: &[&str]) -> Option<String> {
    selectors.iter().find_map(|css| {
        let selector = Selector::parse(css).ok()?;
        document.select(&selector).find_map(|element| {
            let text = html_fragment_to_text(&element.inner_html());
            if text.trim().is_empty() {
                None
            } else {
                Some(text)
            }
        })
    })
}

fn extract_inline_by_selectors(document: &Html, selectors: &[&str]) -> Option<String> {
    selectors.iter().find_map(|css| {
        let selector = Selector::parse(css).ok()?;
        document.select(&selector).find_map(|element| {
            let text = normalize_inline(element.text().collect::<Vec<_>>().join(""));
            if text.trim().is_empty() {
                None
            } else {
                Some(text)
            }
        })
    })
}

fn extract_longest_visible_text(document: &Html) -> Option<String> {
    let selector = selector("main, article, .content, .entry-content");
    document
        .select(&selector)
        .filter_map(|element| {
            let text = html_fragment_to_text(&element.inner_html());
            if text.lines().count() >= 2 && text.chars().count() >= 20 {
                Some(text)
            } else {
                None
            }
        })
        .max_by_key(|text| text.chars().count())
}

fn text_to_document(source: &str, value: &str) -> Result<LyricDocument> {
    let mut document = text::parse(&ensure_trailing_newline(value.to_string()))?;
    document.meta.source = Some(source.to_string());
    Ok(document)
}

fn apply_annotation_text(
    document: &mut LyricDocument,
    reading: Option<&str>,
    romanized: Option<&str>,
) {
    let reading_lines = annotation_lines(reading);
    let romanized_lines = annotation_lines(romanized);
    for (index, line) in document.lines.iter_mut().enumerate() {
        if line.reading.is_none() {
            line.reading = reading_lines.get(index).cloned();
        }
        if line.romanized.is_none() {
            line.romanized = romanized_lines.get(index).cloned();
        }
    }
}

fn annotation_lines(value: Option<&str>) -> Vec<String> {
    value
        .unwrap_or_default()
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

fn html_fragment_to_text(html: &str) -> String {
    let hidden_re = Regex::new(
        r#"(?is)<[^>]*(?:style\s*=\s*["'][^"']*display\s*:\s*none|class\s*=\s*["'][^"']*(?:hidden|d-none|hide|none)[^"']*)[^>]*>.*?</[^>]+>"#,
    )
    .expect("static hidden element regex should compile");
    let script_re = Regex::new(r"(?is)<(?:script|style)\b[^>]*>.*?</(?:script|style)>")
        .expect("static script/style regex should compile");
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
    let cleaned = script_re.replace_all(&cleaned, "").to_string();
    let cleaned = br_re.replace_all(&cleaned, "\n");
    let cleaned = block_re.replace_all(&cleaned, "\n");
    let fragment = Html::parse_fragment(&cleaned);

    normalize_preserve_lines(fragment.root_element().text().collect::<Vec<_>>().join(""))
}

fn document_text(document: &LyricDocument) -> String {
    ensure_trailing_newline(
        document
            .lines
            .iter()
            .map(|line| line.text.as_str())
            .collect::<Vec<_>>()
            .join("\n"),
    )
}

fn decode_bytes(bytes: &[u8], mode: DecodeMode) -> String {
    match mode {
        DecodeMode::Utf8 => String::from_utf8_lossy(bytes).into_owned(),
        DecodeMode::ShiftJis => {
            let (decoded, _, _) = SHIFT_JIS.decode(bytes);
            decoded.into_owned()
        }
    }
}

fn parse_direct_id(value: &str, id_regex: Option<&str>) -> Result<Option<String>> {
    if let Some(id) = value.strip_prefix("id:") {
        let id = id.trim();
        if !id.is_empty() {
            return Ok(Some(id.to_string()));
        }
    }
    let Some(pattern) = id_regex else {
        return Ok(None);
    };
    let re = Regex::new(pattern).map_err(|err| Error::Provider(err.to_string()))?;
    if re.is_match(value) {
        Ok(Some(value.to_string()))
    } else {
        Ok(None)
    }
}

fn parse_line_music_id(value: &str) -> Result<Option<String>> {
    let trimmed = value.trim();
    if !is_http_url(trimmed) {
        return parse_direct_id(trimmed, Some(r"^[A-Za-z0-9_-]+$"));
    }

    let re = Regex::new(r"(?:track/|item=)([A-Za-z0-9_-]+)")
        .map_err(|err| Error::Provider(err.to_string()))?;
    Ok(re
        .captures(trimmed)
        .and_then(|capture| capture.get(1))
        .map(|value| value.as_str().to_string()))
}

fn json_find_lyrics(value: &Value) -> Option<String> {
    match value {
        Value::Array(items) => items.iter().find_map(json_find_lyrics),
        Value::Object(map) => {
            if let Some(lyrics) = map.get("lyrics").or_else(|| map.get("lyric")) {
                match lyrics {
                    Value::String(value) if !value.trim().is_empty() => return Some(value.clone()),
                    Value::Object(_) | Value::Array(_) => {
                        if let Some(value) = json_find_string(
                            lyrics,
                            &["text", "plainText", "body", "lyricsText", "lyricText"],
                        ) {
                            return Some(value);
                        }
                    }
                    _ => {}
                }
            }
            map.values().find_map(json_find_lyrics)
        }
        _ => None,
    }
}

fn json_find_artist_string(value: &Value) -> Option<String> {
    match value {
        Value::Array(items) => items.iter().find_map(json_find_artist_string),
        Value::Object(map) => {
            if let Some(artist) = map
                .get("byArtist")
                .or_else(|| map.get("artist"))
                .or_else(|| map.get("artists"))
            {
                if let Some(value) = json_find_string(artist, &["name", "title"]) {
                    return Some(value);
                }
            }
            map.values().find_map(json_find_artist_string)
        }
        _ => None,
    }
}

fn json_find_string(value: &Value, keys: &[&str]) -> Option<String> {
    match value {
        Value::String(value) => {
            if value.trim().is_empty() {
                None
            } else {
                Some(value.clone())
            }
        }
        Value::Array(items) => items.iter().find_map(|item| json_find_string(item, keys)),
        Value::Object(map) => {
            for key in keys {
                if let Some(value) = map.get(*key) {
                    match value {
                        Value::String(value) if !value.trim().is_empty() => {
                            return Some(value.clone())
                        }
                        Value::Object(_) | Value::Array(_) => {
                            if let Some(value) = json_find_string(value, keys) {
                                return Some(value);
                            }
                        }
                        _ => {}
                    }
                }
            }
            map.values().find_map(|value| json_find_string(value, keys))
        }
        _ => None,
    }
}

fn web_client(cookie: Option<String>, timeout_ms: u64) -> Result<reqwest::Client> {
    let mut headers = HeaderMap::new();
    headers.insert(USER_AGENT, HeaderValue::from_static(USER_AGENT_VALUE));
    headers.insert(
        ACCEPT,
        HeaderValue::from_static("text/html,application/xhtml+xml,application/json,text/plain,*/*"),
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

    Ok(crate::provider::apply_client_timeout(
        reqwest::Client::builder()
            .default_headers(headers)
            .cookie_store(true),
        timeout_ms,
    )
    .build()?)
}

fn is_http_url(value: &str) -> bool {
    value.starts_with("http://") || value.starts_with("https://")
}

fn url_id(value: &str) -> String {
    value
        .trim_end_matches('/')
        .rsplit('/')
        .next()
        .filter(|part| !part.is_empty())
        .unwrap_or(value)
        .split(['?', '#'])
        .next()
        .unwrap_or(value)
        .to_string()
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

fn selector(css: &str) -> Selector {
    Selector::parse(css).expect("static selector should parse")
}

#[cfg(test)]
mod tests {
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::*;

    fn provider(config: WebSourceConfig) -> PublicWebProvider {
        PublicWebProvider::new(None, config, 12_000).unwrap()
    }

    #[tokio::test]
    async fn generic_sources_support_direct_urls_and_extract_text_metadata() {
        let server = MockServer::start().await;
        let sources = [
            Source::Animesongz,
            Source::Awa,
            Source::Azlyrics,
            Source::Genius,
            Source::JTotal,
            Source::Kashinavi,
            Source::Kkbox,
            Source::RockLyric,
            Source::Songtexte,
            Source::TuneCore,
            Source::UtaNet,
            Source::Utamap,
        ];

        for source in sources {
            let config = config_for(source).unwrap().with_base_url(server.uri());
            let path_value = format!("/{}/lyrics", config.key);
            Mock::given(method("GET"))
                .and(path(path_value.as_str()))
                .respond_with(ResponseTemplate::new(200).set_body_string(
                    r#"
                    <html>
                      <h1 class="title">Song</h1>
                      <div class="artist">Artist</div>
                      <div class="lyrics">line one<br><span class="hidden">junk</span>line two</div>
                    </html>
                    "#,
                ))
                .mount(&server)
                .await;

            let provider = provider(config);
            let results = provider
                .search(&format!("{}/{}/lyrics", server.uri(), provider.config.key))
                .await
                .unwrap();
            assert_eq!(results[0].source, source);

            let fetched = provider.fetch(&results[0]).await.unwrap();
            assert_eq!(fetched.input_format, InputFormat::Text);
            assert_eq!(
                String::from_utf8(fetched.raw).unwrap(),
                "line one\nline two\n"
            );
            let document = fetched.document.unwrap();
            assert_eq!(document.meta.title.as_deref(), Some("Song"));
            assert_eq!(document.meta.artist.as_deref(), Some("Artist"));
        }
    }

    #[tokio::test]
    async fn kkbox_extracts_json_ld_lyrics() {
        let server = MockServer::start().await;
        let config = config_for(Source::Kkbox)
            .unwrap()
            .with_base_url(server.uri());
        Mock::given(method("GET"))
            .and(path("/kkbox/lyrics"))
            .respond_with(ResponseTemplate::new(200).set_body_string(
                r#"
                <script type="application/ld+json">
                {
                  "@type": "MusicRecording",
                  "name": "Json Song",
                  "byArtist": { "name": "Json Artist" },
                  "lyrics": { "text": "json line one\njson line two" }
                }
                </script>
                "#,
            ))
            .mount(&server)
            .await;

        let provider = provider(config);
        let results = provider
            .search(&format!("{}/kkbox/lyrics", server.uri()))
            .await
            .unwrap();
        let fetched = provider.fetch(&results[0]).await.unwrap();
        let document = fetched.document.unwrap();
        assert_eq!(document.meta.title.as_deref(), Some("Json Song"));
        assert_eq!(document.meta.artist.as_deref(), Some("Json Artist"));
        assert_eq!(document.lines[0].text, "json line one");
    }

    #[tokio::test]
    async fn lyrical_nonsense_maps_romaji_to_romanized() {
        let server = MockServer::start().await;
        let config = config_for(Source::LyricalNonsense)
            .unwrap()
            .with_base_url(server.uri());
        Mock::given(method("GET"))
            .and(path("/lyrical/lyrics"))
            .respond_with(ResponseTemplate::new(200).set_body_string(
                r#"
                <h1>Song</h1>
                <div class="lyrics">日々<br>歌う</div>
                <div class="lyrics-romaji">hibi<br>utau</div>
                "#,
            ))
            .mount(&server)
            .await;

        let provider = provider(config);
        let results = provider
            .search(&format!("{}/lyrical/lyrics", server.uri()))
            .await
            .unwrap();
        let document = provider.fetch(&results[0]).await.unwrap().document.unwrap();
        assert_eq!(document.lines[0].romanized.as_deref(), Some("hibi"));
        assert_eq!(document.lines[1].romanized.as_deref(), Some("utau"));
    }

    #[tokio::test]
    async fn shift_jis_sources_decode_bytes() {
        let server = MockServer::start().await;
        let config = config_for(Source::JLyric)
            .unwrap()
            .with_base_url(server.uri());
        let (body, _, _) =
            SHIFT_JIS.encode(r#"<html><h1>曲</h1><div class="lyrics">日々<br>歌う</div></html>"#);

        Mock::given(method("GET"))
            .and(path("/jlyric/lyrics"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(body.into_owned()))
            .mount(&server)
            .await;

        let provider = provider(config);
        let results = provider
            .search(&format!("{}/jlyric/lyrics", server.uri()))
            .await
            .unwrap();
        let fetched = provider.fetch(&results[0]).await.unwrap();
        assert_eq!(String::from_utf8(fetched.raw).unwrap(), "日々\n歌う\n");
    }

    #[tokio::test]
    async fn direct_id_builds_configured_url() {
        let server = MockServer::start().await;
        let config = config_for(Source::UtaNet)
            .unwrap()
            .with_base_url(server.uri());
        Mock::given(method("GET"))
            .and(path("/song/123/"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_string(r#"<div class="lyrics">line one<br>line two</div>"#),
            )
            .mount(&server)
            .await;

        let provider = provider(config);
        let results = provider.search("123").await.unwrap();
        let fetched = provider.fetch(&results[0]).await.unwrap();
        assert_eq!(
            String::from_utf8(fetched.raw).unwrap(),
            "line one\nline two\n"
        );
    }

    #[tokio::test]
    async fn missing_lyrics_are_readable() {
        let server = MockServer::start().await;
        let config = config_for(Source::Genius)
            .unwrap()
            .with_base_url(server.uri());
        Mock::given(method("GET"))
            .and(path("/empty"))
            .respond_with(ResponseTemplate::new(200).set_body_string("<html></html>"))
            .mount(&server)
            .await;

        let provider = provider(config);
        let results = provider
            .search(&format!("{}/empty", server.uri()))
            .await
            .unwrap();
        let err = provider.fetch(&results[0]).await.unwrap_err().to_string();
        assert!(err.contains("Genius lyric text was not found"));
    }

    #[tokio::test]
    async fn line_music_fetches_public_h5_json() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api2/tracks/track-1.v1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "response": {
                    "trackName": "Song",
                    "artistName": "Artist"
                }
            })))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/api2/track/track-1/lyrics.v1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "response": {
                    "lyrics": "line one\nline two"
                }
            })))
            .mount(&server)
            .await;

        let provider = LineMusicProvider::with_base_url(None, server.uri(), 12_000).unwrap();
        let results = provider
            .search("https://music.line.me/webapp/track/track-1")
            .await
            .unwrap();
        let fetched = provider.fetch(&results[0]).await.unwrap();
        assert_eq!(
            String::from_utf8(fetched.raw).unwrap(),
            "line one\nline two\n"
        );
        let document = fetched.document.unwrap();
        assert_eq!(document.meta.title.as_deref(), Some("Song"));
        assert_eq!(document.meta.artist.as_deref(), Some("Artist"));
    }
}
