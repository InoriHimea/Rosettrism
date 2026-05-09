use std::time::Duration;

use async_trait::async_trait;
use base64::Engine;
use reqwest::header::{HeaderMap, HeaderValue, ACCEPT, COOKIE, REFERER, USER_AGENT};
use serde::de::DeserializeOwned;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::decoder::InputFormat;
use crate::provider::{FetchedLyric, LyricProvider, SearchResult, Source};
use crate::{Error, Result};

const USER_AGENT_VALUE: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36";
const SONG_SEARCH_PAGE_SIZE: usize = 100;
const MAX_SEARCH_RESULTS: usize = 100;

pub struct KugouProvider {
    client: reqwest::Client,
    song_search_endpoints: Vec<KugouSongSearchEndpoint>,
    lyric_search_endpoints: Vec<KugouLyricSearchEndpoint>,
    download_urls: Vec<String>,
}

#[derive(Debug, Clone)]
struct KugouSongSearchEndpoint {
    name: &'static str,
    url: String,
    kind: KugouSongSearchKind,
}

impl KugouSongSearchEndpoint {
    fn legacy(name: &'static str, url: impl Into<String>) -> Self {
        Self {
            name,
            url: url.into(),
            kind: KugouSongSearchKind::Legacy,
        }
    }

    fn mobile(name: &'static str, url: impl Into<String>) -> Self {
        Self {
            name,
            url: url.into(),
            kind: KugouSongSearchKind::Mobile,
        }
    }

    fn ios(name: &'static str, url: impl Into<String>) -> Self {
        Self {
            name,
            url: url.into(),
            kind: KugouSongSearchKind::Ios,
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum KugouSongSearchKind {
    Legacy,
    Mobile,
    Ios,
}

#[derive(Debug, Clone)]
struct KugouLyricSearchEndpoint {
    name: &'static str,
    url: String,
    man: &'static str,
}

impl KugouLyricSearchEndpoint {
    fn new(name: &'static str, url: impl Into<String>, man: &'static str) -> Self {
        Self {
            name,
            url: url.into(),
            man,
        }
    }
}

impl KugouProvider {
    pub fn new(cookie: Option<String>) -> Result<Self> {
        Self::with_endpoint_sets(
            cookie,
            vec![
                KugouSongSearchEndpoint::mobile(
                    "msearchcdn song search",
                    "http://msearchcdn.kugou.com/api/v3/search/song",
                ),
                KugouSongSearchEndpoint::ios(
                    "ioscdn song search",
                    "http://ioscdn.kugou.com/api/v3/search/song",
                ),
                KugouSongSearchEndpoint::mobile(
                    "mobilecdn song search",
                    "http://mobilecdn.kugou.com/api/v3/search/song",
                ),
                KugouSongSearchEndpoint::mobile(
                    "msearchcdn song search (https)",
                    "https://msearchcdn.kugou.com/api/v3/search/song",
                ),
                KugouSongSearchEndpoint::legacy(
                    "songsearch_v2",
                    "https://songsearch.kugou.com/song_search_v2",
                ),
            ],
            vec![
                KugouLyricSearchEndpoint::new(
                    "krcs lyric search",
                    "http://krcs.kugou.com/search",
                    "no",
                ),
                KugouLyricSearchEndpoint::new(
                    "krcs lyric search (https)",
                    "https://krcs.kugou.com/search",
                    "no",
                ),
                KugouLyricSearchEndpoint::new(
                    "legacy lyric search",
                    "https://lyrics.kugou.com/search",
                    "yes",
                ),
            ],
            vec![
                "http://lyrics2.kugou.com/download".to_string(),
                "https://lyrics2.kugou.com/download".to_string(),
            ],
        )
    }

    #[cfg(test)]
    fn with_endpoints(
        cookie: Option<String>,
        song_search_url: impl Into<String>,
        lyric_search_url: impl Into<String>,
        download_url: impl Into<String>,
    ) -> Result<Self> {
        Self::with_endpoint_sets(
            cookie,
            vec![KugouSongSearchEndpoint::legacy(
                "song search",
                song_search_url,
            )],
            vec![KugouLyricSearchEndpoint::new(
                "lyric search",
                lyric_search_url,
                "yes",
            )],
            vec![download_url.into()],
        )
    }

    fn with_endpoint_sets(
        cookie: Option<String>,
        song_search_endpoints: Vec<KugouSongSearchEndpoint>,
        lyric_search_endpoints: Vec<KugouLyricSearchEndpoint>,
        download_urls: Vec<String>,
    ) -> Result<Self> {
        let mut headers = HeaderMap::new();
        headers.insert(USER_AGENT, HeaderValue::from_static(USER_AGENT_VALUE));
        headers.insert(
            ACCEPT,
            HeaderValue::from_static("application/json, text/plain, */*"),
        );
        headers.insert(REFERER, HeaderValue::from_static("https://www.kugou.com/"));
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
            .timeout(Duration::from_secs(12))
            .build()?;

        Ok(Self {
            client,
            song_search_endpoints,
            lyric_search_endpoints,
            download_urls,
        })
    }

    async fn search_songs(&self, query: &str) -> Result<Vec<KugouSong>> {
        let mut errors = Vec::new();
        let mut ranked = Vec::new();
        let mut order = 0usize;

        for search_query in kugou_search_queries(query) {
            match self.search_songs_once(&search_query).await {
                Ok(songs) if !songs.is_empty() => {
                    for song in songs {
                        ranked.push(RankedKugouSong {
                            score: song_relevance(&song, query),
                            order,
                            song,
                        });
                        order += 1;
                    }
                }
                Ok(_) => errors.push(format!("{search_query}: no songs")),
                Err(err) => errors.push(format!("{search_query}: {err}")),
            }
        }

        dedupe_ranked_songs(&mut ranked);
        ranked.sort_by(|left, right| {
            right
                .score
                .partial_cmp(&left.score)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| left.order.cmp(&right.order))
        });

        let songs = ranked
            .into_iter()
            .map(|ranked| ranked.song)
            .collect::<Vec<_>>();
        if !songs.is_empty() {
            return Ok(songs);
        }

        Err(Error::Provider(format!(
            "Kugou song search failed: {}",
            errors.join("; ")
        )))
    }

    async fn search_songs_once(&self, query: &str) -> Result<Vec<KugouSong>> {
        let mut errors = Vec::new();

        for endpoint in &self.song_search_endpoints {
            match self.search_songs_from_endpoint(endpoint, query).await {
                Ok(songs) if !songs.is_empty() => return Ok(songs),
                Ok(_) => errors.push(format!("{}: no songs", endpoint.name)),
                Err(err) => errors.push(format!("{}: {err}", endpoint.name)),
            }
        }

        Err(Error::Provider(errors.join("; ")))
    }

    async fn search_songs_from_endpoint(
        &self,
        endpoint: &KugouSongSearchEndpoint,
        query: &str,
    ) -> Result<Vec<KugouSong>> {
        let page_size = SONG_SEARCH_PAGE_SIZE.to_string();
        let request = match endpoint.kind {
            KugouSongSearchKind::Legacy => self.client.get(&endpoint.url).query(&[
                ("keyword", query),
                ("page", "1"),
                ("pagesize", page_size.as_str()),
                ("platform", "WebFilter"),
            ]),
            KugouSongSearchKind::Mobile => self.client.get(&endpoint.url).query(&[
                ("format", "json"),
                ("keyword", query),
                ("page", "1"),
                ("pagesize", page_size.as_str()),
                ("plat", "0"),
                ("version", "9108"),
            ]),
            KugouSongSearchKind::Ios => self.client.get(&endpoint.url).query(&[
                ("keyword", query),
                ("page", "1"),
                ("pagesize", page_size.as_str()),
                ("showtype", "10"),
                ("plat", "2"),
                ("version", "7910"),
                ("tag", "1"),
                ("correct", "1"),
                ("privilege", "1"),
                ("sver", "5"),
            ]),
        };

        let response = send_json::<KugouSongSearchResponse>(request, endpoint.name).await?;

        Ok(flatten_songs(response.data.info))
    }

    async fn search_lyrics(&self, song: &KugouSong, keyword: &str) -> Result<Vec<SearchResult>> {
        let mut errors = Vec::new();

        for endpoint in &self.lyric_search_endpoints {
            match self
                .search_lyrics_from_endpoint(endpoint, song, keyword)
                .await
            {
                Ok(results) if !results.is_empty() => return Ok(results),
                Ok(_) => errors.push(format!("{}: no lyric candidates", endpoint.name)),
                Err(err) => errors.push(format!("{}: {err}", endpoint.name)),
            }
        }

        Err(Error::Provider(format!(
            "Kugou lyric search failed: {}",
            errors.join("; ")
        )))
    }

    async fn search_lyrics_from_endpoint(
        &self,
        endpoint: &KugouLyricSearchEndpoint,
        song: &KugouSong,
        keyword: &str,
    ) -> Result<Vec<SearchResult>> {
        let duration_ms = song.duration.map(|seconds| seconds.saturating_mul(1_000));
        let duration_param = duration_ms.unwrap_or_default().to_string();
        let hash = song.hash.as_deref().unwrap_or_default();
        let album_audio_id = song.album_audio_id.unwrap_or_default().to_string();

        let response = send_json::<KugouLyricSearchResponse>(
            self.client.get(&endpoint.url).query(&[
                ("ver", "1"),
                ("man", endpoint.man),
                ("client", "pc"),
                ("keyword", keyword),
                ("duration", duration_param.as_str()),
                ("hash", hash),
                ("album_audio_id", album_audio_id.as_str()),
                ("lrctxt", "1"),
            ]),
            endpoint.name,
        )
        .await?;

        let title = song
            .songname
            .clone()
            .or_else(|| song.filename.clone())
            .unwrap_or_else(|| keyword.to_string());
        let artist = song.singername.clone().unwrap_or_default();
        let album = song.album_name.clone();

        let mut candidates = response.candidates;
        rank_lyric_candidates(&mut candidates, song);

        let results = candidates
            .into_iter()
            .map(|candidate| SearchResult {
                source: Source::Kugou,
                id: candidate.id.clone(),
                title: candidate.song.clone().unwrap_or_else(|| title.clone()),
                artist: candidate.singer.clone().unwrap_or_else(|| artist.clone()),
                album: album.clone(),
                duration_ms,
                extra: json!({
                    "id": candidate.id,
                    "accesskey": candidate.accesskey,
                    "fmt": candidate.fmt.unwrap_or_else(|| "krc".to_string()),
                    "hash": hash,
                    "album_audio_id": song.album_audio_id
                }),
            })
            .collect();

        Ok(results)
    }

    async fn download(&self, id: &str, accesskey: &str, fmt: &str) -> Result<Vec<u8>> {
        let mut errors = Vec::new();

        for url in &self.download_urls {
            match self.download_from_endpoint(url, id, accesskey, fmt).await {
                Ok(raw) if !raw.is_empty() => return Ok(raw),
                Ok(_) => errors.push(format!("{url}: empty response")),
                Err(err) => errors.push(format!("{url}: {err}")),
            }
        }

        Err(Error::Provider(format!(
            "Kugou download endpoint failed: {}",
            errors.join("; ")
        )))
    }

    async fn download_from_endpoint(
        &self,
        url: &str,
        id: &str,
        accesskey: &str,
        fmt: &str,
    ) -> Result<Vec<u8>> {
        let response = self
            .client
            .get(url)
            .query(&[
                ("ver", "1"),
                ("client", "pc"),
                ("id", id),
                ("accesskey", accesskey),
                ("fmt", fmt),
                ("charset", "utf8"),
            ])
            .send()
            .await?
            .error_for_status()?;

        let bytes = response.bytes().await?.to_vec();
        if let Ok(json) = serde_json::from_slice::<KugouDownloadResponse>(&bytes) {
            if let Some(content) = json.content {
                let decoded = base64::engine::general_purpose::STANDARD
                    .decode(content.trim())
                    .map_err(|err| Error::Provider(format!("invalid Kugou lyric base64: {err}")))?;
                return Ok(decoded);
            }
        }

        Ok(bytes)
    }
}

async fn send_json<T>(request: reqwest::RequestBuilder, context: &str) -> Result<T>
where
    T: DeserializeOwned,
{
    let response = request.send().await?;
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

#[async_trait]
impl LyricProvider for KugouProvider {
    async fn search(&self, query: &str) -> Result<Vec<SearchResult>> {
        let songs = self.search_songs(query).await?;
        let mut results = songs
            .into_iter()
            .take(MAX_SEARCH_RESULTS)
            .map(|song| song.into_search_result())
            .collect::<Vec<_>>();
        rank_search_results(&mut results, query);

        if results.is_empty() {
            Err(Error::Provider("Kugou search returned no songs".into()))
        } else {
            Ok(results)
        }
    }

    async fn fetch(&self, result: &SearchResult) -> Result<FetchedLyric> {
        let resolved;
        let lyric_result = if result
            .extra
            .get("accesskey")
            .and_then(Value::as_str)
            .is_some()
        {
            result
        } else {
            resolved = self.resolve_lyric_result(result).await?;
            &resolved
        };
        let id = lyric_result
            .extra
            .get("id")
            .and_then(Value::as_str)
            .unwrap_or(lyric_result.id.as_str());
        let accesskey = lyric_result
            .extra
            .get("accesskey")
            .and_then(Value::as_str)
            .ok_or_else(|| Error::Provider("Kugou candidate is missing accesskey".into()))?;

        let mut errors = Vec::new();
        for fmt in download_format_order(lyric_result) {
            match self.download(id, accesskey, fmt).await {
                Ok(raw) if !raw.is_empty() => {
                    let input_format = input_format_for_download(fmt);
                    if let Err(err) = validate_download_payload(input_format, &raw) {
                        errors.push(format!("{fmt}: {err}"));
                        continue;
                    }
                    return Ok(FetchedLyric {
                        input_format,
                        raw,
                        document: None,
                        annotations: Vec::new(),
                    });
                }
                Ok(_) => errors.push(format!("{fmt}: empty response")),
                Err(err) => errors.push(format!("{fmt}: {err}")),
            }
        }

        Err(Error::Provider(format!(
            "Kugou download failed: {}",
            errors.join("; ")
        )))
    }
}

impl KugouProvider {
    async fn resolve_lyric_result(&self, result: &SearchResult) -> Result<SearchResult> {
        let song = KugouSong::from_search_result(result);
        let keyword = song
            .lyric_keyword()
            .unwrap_or_else(|| format!("{} {}", result.title, result.artist));
        self.search_lyrics(&song, &keyword)
            .await?
            .into_iter()
            .next()
            .ok_or_else(|| Error::Provider("Kugou returned no lyric candidates for song".into()))
    }
}

fn input_format_for_download(fmt: &str) -> InputFormat {
    if fmt.eq_ignore_ascii_case("krc") {
        InputFormat::Krc
    } else {
        InputFormat::Lrc
    }
}

fn download_format_order(result: &SearchResult) -> Vec<&'static str> {
    let mut formats = Vec::new();
    push_download_format(&mut formats, "krc");
    if let Some(fmt) = result.extra.get("fmt").and_then(Value::as_str) {
        push_download_format(&mut formats, fmt);
    }
    push_download_format(&mut formats, "lrc");
    formats
}

fn push_download_format(formats: &mut Vec<&'static str>, fmt: &str) {
    let fmt = if fmt.eq_ignore_ascii_case("krc") {
        "krc"
    } else if fmt.eq_ignore_ascii_case("lrc") {
        "lrc"
    } else {
        return;
    };

    if !formats.contains(&fmt) {
        formats.push(fmt);
    }
}

fn validate_download_payload(input_format: InputFormat, raw: &[u8]) -> Result<()> {
    if input_format == InputFormat::Krc {
        validate_krc_payload(raw)?;
    }
    Ok(())
}

fn validate_krc_payload(raw: &[u8]) -> Result<()> {
    if raw.starts_with(b"krc1") {
        let text = crate::decoder::krc::decode_raw(raw)
            .map_err(|err| Error::Provider(format!("invalid KRC payload: {err}")))?;
        if text.lines().any(is_plain_krc_line) {
            return Ok(());
        }
        return Err(Error::Provider(
            "invalid KRC payload: decoded KRC has no word-timed lines".into(),
        ));
    }

    if let Ok(text) = std::str::from_utf8(raw) {
        if text.lines().any(is_plain_krc_line) {
            return Ok(());
        }
    }

    Err(Error::Provider(
        "invalid KRC payload: KRC request returned non-KRC text".into(),
    ))
}

fn is_plain_krc_line(line: &str) -> bool {
    let line = line.trim_start_matches('\u{feff}').trim();
    let Some(rest) = line.strip_prefix('[') else {
        return false;
    };
    let Some((stamp, body)) = rest.split_once(']') else {
        return false;
    };
    let Some((start, duration)) = stamp.split_once(',') else {
        return false;
    };

    start.parse::<u32>().is_ok() && duration.parse::<u32>().is_ok() && body.contains('<')
}

#[derive(Debug, Deserialize)]
struct KugouSongSearchResponse {
    #[serde(default)]
    data: KugouSongSearchData,
}

#[derive(Debug, Default, Deserialize)]
struct KugouSongSearchData {
    #[serde(default, alias = "lists")]
    info: Vec<KugouSong>,
}

#[derive(Debug, Deserialize)]
struct KugouSong {
    #[serde(default, alias = "FileName")]
    filename: Option<String>,
    #[serde(default, alias = "SongName")]
    songname: Option<String>,
    #[serde(default, alias = "SingerName")]
    singername: Option<String>,
    #[serde(default, alias = "AlbumName")]
    album_name: Option<String>,
    #[serde(default, alias = "FileHash", alias = "Hash")]
    hash: Option<String>,
    #[serde(default, alias = "Duration")]
    duration: Option<u32>,
    #[serde(default, alias = "AlbumAudioID", alias = "Audioid", alias = "Scid")]
    album_audio_id: Option<u64>,
    #[serde(default)]
    group: Vec<KugouSong>,
}

impl KugouSong {
    fn display_title(&self) -> Option<&str> {
        self.songname
            .as_deref()
            .or(self.filename.as_deref())
            .map(str::trim)
            .filter(|value| !value.is_empty())
    }

    fn lyric_keyword(&self) -> Option<String> {
        let title = self.display_title()?;
        let artist = self.singername.as_deref().unwrap_or_default().trim();
        if artist.is_empty() {
            Some(title.to_string())
        } else {
            Some(format!("{title} {artist}"))
        }
    }

    fn into_search_result(self) -> SearchResult {
        let title = self
            .songname
            .clone()
            .or_else(|| self.filename.clone())
            .unwrap_or_else(|| "Kugou song".to_string());
        let artist = self.singername.clone().unwrap_or_default();
        let id = self
            .album_audio_id
            .map(|id| id.to_string())
            .or_else(|| self.hash.clone())
            .or_else(|| self.filename.clone())
            .unwrap_or_else(|| title.clone());

        SearchResult {
            source: Source::Kugou,
            id,
            title,
            artist,
            album: self.album_name.clone(),
            duration_ms: self.duration.map(|seconds| seconds.saturating_mul(1_000)),
            extra: json!({
                "provider_result": "song",
                "fmt": "krc",
                "hash": self.hash,
                "album_audio_id": self.album_audio_id,
                "filename": self.filename
            }),
        }
    }

    fn from_search_result(result: &SearchResult) -> Self {
        Self {
            filename: result
                .extra
                .get("filename")
                .and_then(Value::as_str)
                .map(ToOwned::to_owned),
            songname: Some(result.title.clone()),
            singername: Some(result.artist.clone()),
            album_name: result.album.clone(),
            hash: result
                .extra
                .get("hash")
                .and_then(Value::as_str)
                .map(ToOwned::to_owned),
            duration: result.duration_ms.map(|duration| duration / 1_000),
            album_audio_id: value_as_u64(result.extra.get("album_audio_id")),
            group: Vec::new(),
        }
    }
}

fn flatten_songs(songs: Vec<KugouSong>) -> Vec<KugouSong> {
    let mut flattened = Vec::new();
    for mut song in songs {
        let group = std::mem::take(&mut song.group);
        flattened.push(song);
        flattened.extend(flatten_songs(group));
    }
    flattened
}

fn value_as_u64(value: Option<&Value>) -> Option<u64> {
    value.and_then(|value| {
        value
            .as_u64()
            .or_else(|| value.as_str().and_then(|value| value.parse().ok()))
    })
}

#[derive(Debug)]
struct RankedKugouSong {
    song: KugouSong,
    score: f32,
    order: usize,
}

fn kugou_search_queries(query: &str) -> Vec<String> {
    let mut queries = Vec::new();
    push_unique_query(&mut queries, query);

    for token in query.split_whitespace() {
        if token.chars().count() >= 2 {
            push_unique_query(&mut queries, token);
        }
    }

    queries
}

fn push_unique_query(queries: &mut Vec<String>, query: &str) {
    let query = query.trim();
    if query.is_empty() {
        return;
    }
    if !queries.iter().any(|existing| existing == query) {
        queries.push(query.to_string());
    }
}

fn dedupe_ranked_songs(ranked: &mut Vec<RankedKugouSong>) {
    let mut deduped: Vec<RankedKugouSong> = Vec::new();
    for item in ranked.drain(..) {
        if let Some(existing) = deduped
            .iter_mut()
            .find(|existing| same_kugou_song(&existing.song, &item.song))
        {
            if item.score > existing.score {
                existing.score = item.score;
            }
            existing.order = existing.order.min(item.order);
        } else {
            deduped.push(item);
        }
    }
    *ranked = deduped;
}

fn same_kugou_song(left: &KugouSong, right: &KugouSong) -> bool {
    if left
        .album_audio_id
        .zip(right.album_audio_id)
        .is_some_and(|(left, right)| left == right)
    {
        return true;
    }
    if left
        .hash
        .as_deref()
        .zip(right.hash.as_deref())
        .is_some_and(|(left, right)| left.eq_ignore_ascii_case(right))
    {
        return true;
    }

    normalized_text(left.display_title().unwrap_or_default())
        == normalized_text(right.display_title().unwrap_or_default())
        && normalized_text(left.singername.as_deref().unwrap_or_default())
            == normalized_text(right.singername.as_deref().unwrap_or_default())
}

fn song_relevance(song: &KugouSong, query: &str) -> f32 {
    let terms = query
        .split_whitespace()
        .map(normalized_text)
        .filter(|term| !term.is_empty())
        .collect::<Vec<_>>();
    if terms.is_empty() {
        return 0.0;
    }

    let title = normalized_text(song.display_title().unwrap_or_default());
    let artist = normalized_text(song.singername.as_deref().unwrap_or_default());
    let combined = normalized_text(&format!("{}{}", title, artist));
    let mut score = 0.0;

    for term in &terms {
        let title_score = text_similarity(&title, term);
        let artist_score = text_similarity(&artist, term);
        let combined_score = text_similarity(&combined, term) * 0.75;
        score += title_score.max(artist_score).max(combined_score);
    }

    let term_score = score / terms.len() as f32;
    let full_query = normalized_text(query);
    let metadata_score = text_similarity(&full_query, &title) * 0.65
        + if artist.is_empty() {
            0.0
        } else {
            text_similarity(&full_query, &artist) * 0.35
        };

    term_score.max(metadata_score)
}

fn text_similarity(haystack: &str, needle: &str) -> f32 {
    if haystack.is_empty() || needle.is_empty() {
        return 0.0;
    }
    if haystack == needle {
        return 1.0;
    }
    if haystack.contains(needle) {
        return containment_score(haystack, needle);
    }

    let reversed = needle.chars().rev().collect::<String>();
    if haystack == reversed {
        return 0.97;
    }
    if reversed != needle && haystack.contains(&reversed) {
        return containment_score(haystack, &reversed) * 0.98;
    }

    common_char_ratio(haystack, needle) * 0.8
}

fn containment_score(haystack: &str, needle: &str) -> f32 {
    let haystack_len = haystack.chars().count();
    let needle_len = needle.chars().count();
    if haystack_len == 0 {
        return 0.0;
    }

    let coverage = needle_len as f32 / haystack_len as f32;
    0.82 + coverage.min(1.0) * 0.14
}

fn common_char_ratio(haystack: &str, needle: &str) -> f32 {
    let mut chars = haystack.chars().collect::<Vec<_>>();
    let mut matched = 0usize;
    for ch in needle.chars() {
        if let Some(index) = chars.iter().position(|candidate| *candidate == ch) {
            chars.remove(index);
            matched += 1;
        }
    }

    let total = needle.chars().count();
    if total == 0 {
        0.0
    } else {
        matched as f32 / total as f32
    }
}

fn normalized_text(value: &str) -> String {
    value
        .chars()
        .flat_map(char::to_lowercase)
        .filter(|ch| !ch.is_whitespace() && !ch.is_ascii_punctuation())
        .collect()
}

fn rank_lyric_candidates(candidates: &mut Vec<KugouLyricCandidate>, song: &KugouSong) {
    let mut ranked = candidates
        .drain(..)
        .enumerate()
        .map(|(order, candidate)| {
            let score = lyric_candidate_relevance(&candidate, song);
            (candidate, score, order)
        })
        .collect::<Vec<_>>();

    let best_score = ranked
        .iter()
        .map(|(_, score, _)| *score)
        .fold(0.0_f32, f32::max);
    if best_score >= 0.7 {
        ranked.retain(|(_, score, _)| *score >= 0.55);
    }

    ranked.sort_by(|left, right| {
        right
            .1
            .partial_cmp(&left.1)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| left.2.cmp(&right.2))
    });

    *candidates = ranked
        .into_iter()
        .map(|(candidate, _, _)| candidate)
        .collect();
}

fn lyric_candidate_relevance(candidate: &KugouLyricCandidate, song: &KugouSong) -> f32 {
    let song_title = normalized_text(song.display_title().unwrap_or_default());
    let song_artist = normalized_text(song.singername.as_deref().unwrap_or_default());
    let candidate_title = normalized_text(candidate.song.as_deref().unwrap_or_default());
    let candidate_artist = normalized_text(candidate.singer.as_deref().unwrap_or_default());

    let title_score = text_similarity(&candidate_title, &song_title);
    let artist_score = if song_artist.is_empty() || candidate_artist.is_empty() {
        0.0
    } else {
        text_similarity(&candidate_artist, &song_artist)
    };

    title_score * 0.75 + artist_score * 0.25
}

fn rank_search_results(results: &mut Vec<SearchResult>, query: &str) {
    let mut ranked = results
        .drain(..)
        .enumerate()
        .map(|(order, result)| {
            let score = result_relevance(&result, query);
            (result, score, order)
        })
        .collect::<Vec<_>>();

    ranked.sort_by(|left, right| {
        right
            .1
            .partial_cmp(&left.1)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| left.2.cmp(&right.2))
    });

    *results = ranked.into_iter().map(|(result, _, _)| result).collect();
}

fn result_relevance(result: &SearchResult, query: &str) -> f32 {
    metadata_relevance(&result.title, &result.artist, query)
}

fn metadata_relevance(title: &str, artist: &str, query: &str) -> f32 {
    let terms = query
        .split_whitespace()
        .map(normalized_text)
        .filter(|term| !term.is_empty())
        .collect::<Vec<_>>();
    let title = normalized_text(title);
    let artist = normalized_text(artist);

    let term_score = if terms.is_empty() {
        0.0
    } else {
        let mut score = 0.0;
        for term in &terms {
            score += text_similarity(&title, term).max(text_similarity(&artist, term));
        }
        score / terms.len() as f32
    };

    let full_query = normalized_text(query);
    let metadata_score = text_similarity(&full_query, &title) * 0.65
        + if artist.is_empty() {
            0.0
        } else {
            text_similarity(&full_query, &artist) * 0.35
        };

    term_score.max(metadata_score)
}

#[derive(Debug, Default, Deserialize)]
struct KugouLyricSearchResponse {
    #[serde(default)]
    candidates: Vec<KugouLyricCandidate>,
}

#[derive(Debug, Deserialize)]
struct KugouLyricCandidate {
    id: String,
    accesskey: String,
    #[serde(default)]
    song: Option<String>,
    #[serde(default)]
    singer: Option<String>,
    #[serde(default)]
    fmt: Option<String>,
}

#[derive(Debug, Deserialize)]
struct KugouDownloadResponse {
    #[serde(default)]
    content: Option<String>,
}

#[cfg(test)]
mod tests {
    use base64::Engine;
    use serde_json::json;
    use wiremock::matchers::{header, method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::*;

    #[test]
    fn relevance_prefers_fuzzy_title_and_artist_over_artist_only() {
        let target = KugouSong {
            filename: None,
            songname: Some("退后".into()),
            singername: Some("周杰伦".into()),
            album_name: None,
            hash: Some("target".into()),
            duration: Some(261),
            album_audio_id: Some(1),
            group: Vec::new(),
        };
        let popular = KugouSong {
            filename: None,
            songname: Some("告白气球".into()),
            singername: Some("周杰伦".into()),
            album_name: None,
            hash: Some("popular".into()),
            duration: Some(215),
            album_audio_id: Some(2),
            group: Vec::new(),
        };
        let lyric_match = KugouSong {
            filename: None,
            songname: Some("最美的爱情，回忆里待续".into()),
            singername: Some("你好，周杰伦".into()),
            album_name: None,
            hash: Some("lyric".into()),
            duration: Some(260),
            album_audio_id: Some(3),
            group: Vec::new(),
        };

        assert!(song_relevance(&target, "后退 周杰伦") > song_relevance(&popular, "后退 周杰伦"));
        assert!(
            song_relevance(&target, "后退 周杰伦") > song_relevance(&lyric_match, "后退 周杰伦")
        );
    }

    #[test]
    fn query_variants_include_whitespace_terms() {
        assert_eq!(
            kugou_search_queries("后退 周杰伦"),
            vec![
                "后退 周杰伦".to_string(),
                "后退".to_string(),
                "周杰伦".to_string()
            ]
        );
    }

    #[test]
    fn lyric_candidate_ranking_prefers_platform_song_metadata() {
        let song = KugouSong {
            filename: None,
            songname: Some("退后".into()),
            singername: Some("周杰伦".into()),
            album_name: None,
            hash: Some("song".into()),
            duration: Some(261),
            album_audio_id: Some(1),
            group: Vec::new(),
        };
        let mut candidates = vec![
            KugouLyricCandidate {
                id: "lyric".into(),
                accesskey: "bad".into(),
                song: Some("最美的爱情，回忆里待续".into()),
                singer: Some("你好，周杰伦".into()),
                fmt: Some("krc".into()),
            },
            KugouLyricCandidate {
                id: "song".into(),
                accesskey: "good".into(),
                song: Some("退后".into()),
                singer: Some("周杰伦".into()),
                fmt: Some("krc".into()),
            },
        ];

        rank_lyric_candidates(&mut candidates, &song);

        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].id, "song");
    }

    #[test]
    fn final_result_ranking_prefers_metadata_matches() {
        let mut results = vec![
            SearchResult {
                source: Source::Kugou,
                id: "lyric".into(),
                title: "最美的爱情，回忆里待续".into(),
                artist: "你好，周杰伦".into(),
                album: None,
                duration_ms: Some(260_000),
                extra: json!({}),
            },
            SearchResult {
                source: Source::Kugou,
                id: "song".into(),
                title: "退后".into(),
                artist: "周杰伦".into(),
                album: None,
                duration_ms: Some(261_000),
                extra: json!({}),
            },
        ];

        rank_search_results(&mut results, "后退 周杰伦");

        assert_eq!(results[0].id, "song");
        assert_eq!(results.len(), 2);
    }

    #[tokio::test]
    async fn searches_and_fetches_with_cookie() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/song"))
            .and(header("cookie", "kg=1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "data": {
                    "lists": [{
                        "SongName": "Song",
                        "SingerName": "Artist",
                        "AlbumName": "Album",
                        "FileHash": "HASH",
                        "Duration": 1,
                        "Audioid": 10
                    }]
                }
            })))
            .mount(&server)
            .await;

        Mock::given(method("GET"))
            .and(path("/lyric"))
            .and(header("cookie", "kg=1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "candidates": [{
                    "id": "kid",
                    "accesskey": "akey",
                    "song": "Song",
                    "singer": "Artist",
                    "fmt": "lrc"
                }]
            })))
            .mount(&server)
            .await;

        let encoded = base64::engine::general_purpose::STANDARD.encode("[00:01.00]Hi\n");
        Mock::given(method("GET"))
            .and(path("/download"))
            .and(header("cookie", "kg=1"))
            .and(query_param("fmt", "krc"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "content": ""
            })))
            .mount(&server)
            .await;

        Mock::given(method("GET"))
            .and(path("/download"))
            .and(header("cookie", "kg=1"))
            .and(query_param("fmt", "lrc"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "content": encoded
            })))
            .mount(&server)
            .await;

        let provider = KugouProvider::with_endpoints(
            Some("kg=1".into()),
            format!("{}/song", server.uri()),
            format!("{}/lyric", server.uri()),
            format!("{}/download", server.uri()),
        )
        .unwrap();

        let results = provider.search("Song Artist").await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title, "Song");

        let fetched = provider.fetch(&results[0]).await.unwrap();
        assert_eq!(fetched.input_format, InputFormat::Lrc);
        assert_eq!(String::from_utf8(fetched.raw).unwrap(), "[00:01.00]Hi\n");
    }

    #[tokio::test]
    async fn search_falls_back_when_song_endpoint_returns_500() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/song-bad"))
            .respond_with(ResponseTemplate::new(500).set_body_string("song search down"))
            .mount(&server)
            .await;

        Mock::given(method("GET"))
            .and(path("/song-ok"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "data": {
                    "info": [{
                        "songname": "Song",
                        "singername": "Artist",
                        "album_name": "Album",
                        "hash": "HASH",
                        "duration": 1,
                        "album_audio_id": 10
                    }]
                }
            })))
            .mount(&server)
            .await;

        Mock::given(method("GET"))
            .and(path("/lyric"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "candidates": [{
                    "id": "kid",
                    "accesskey": "akey",
                    "song": "Song",
                    "singer": "Artist"
                }]
            })))
            .mount(&server)
            .await;

        let provider = KugouProvider::with_endpoint_sets(
            None,
            vec![
                KugouSongSearchEndpoint::legacy(
                    "bad song search",
                    format!("{}/song-bad", server.uri()),
                ),
                KugouSongSearchEndpoint::mobile(
                    "ok song search",
                    format!("{}/song-ok", server.uri()),
                ),
            ],
            vec![KugouLyricSearchEndpoint::new(
                "lyric search",
                format!("{}/lyric", server.uri()),
                "yes",
            )],
            vec![format!("{}/download", server.uri())],
        )
        .unwrap();

        let results = provider.search("Song Artist").await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].artist, "Artist");
    }

    #[tokio::test]
    async fn search_falls_back_when_lyric_endpoint_returns_500() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/song"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "data": {
                    "lists": [{
                        "SongName": "Song",
                        "SingerName": "Artist",
                        "AlbumName": "Album",
                        "FileHash": "HASH",
                        "Duration": 1,
                        "Audioid": 10
                    }]
                }
            })))
            .mount(&server)
            .await;

        Mock::given(method("GET"))
            .and(path("/lyric-bad"))
            .respond_with(ResponseTemplate::new(500).set_body_string("lyric search down"))
            .mount(&server)
            .await;

        Mock::given(method("GET"))
            .and(path("/lyric-ok"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "candidates": [{
                    "id": "kid",
                    "accesskey": "akey",
                    "song": "Song",
                    "singer": "Artist"
                }]
            })))
            .mount(&server)
            .await;

        let provider = KugouProvider::with_endpoint_sets(
            None,
            vec![KugouSongSearchEndpoint::legacy(
                "song search",
                format!("{}/song", server.uri()),
            )],
            vec![
                KugouLyricSearchEndpoint::new(
                    "bad lyric search",
                    format!("{}/lyric-bad", server.uri()),
                    "yes",
                ),
                KugouLyricSearchEndpoint::new(
                    "ok lyric search",
                    format!("{}/lyric-ok", server.uri()),
                    "yes",
                ),
            ],
            vec![format!("{}/download", server.uri())],
        )
        .unwrap();

        let song = KugouSong {
            filename: None,
            songname: Some("Song".into()),
            singername: Some("Artist".into()),
            album_name: Some("Album".into()),
            hash: Some("HASH".into()),
            duration: Some(1),
            album_audio_id: Some(10),
            group: Vec::new(),
        };
        let results = provider.search_lyrics(&song, "Song Artist").await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, "kid");
    }

    #[tokio::test]
    async fn fetch_falls_back_when_download_endpoint_returns_500() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/download-bad"))
            .respond_with(ResponseTemplate::new(500).set_body_string("download down"))
            .mount(&server)
            .await;

        let encoded = base64::engine::general_purpose::STANDARD.encode("[1000,500]<0,500,0>Hi\n");
        Mock::given(method("GET"))
            .and(path("/download-ok"))
            .and(query_param("fmt", "krc"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "content": encoded
            })))
            .mount(&server)
            .await;

        let provider = KugouProvider::with_endpoint_sets(
            None,
            Vec::new(),
            Vec::new(),
            vec![
                format!("{}/download-bad", server.uri()),
                format!("{}/download-ok", server.uri()),
            ],
        )
        .unwrap();

        let result = SearchResult {
            source: Source::Kugou,
            id: "kid".into(),
            title: "Song".into(),
            artist: "Artist".into(),
            album: None,
            duration_ms: Some(1_000),
            extra: json!({
                "id": "kid",
                "accesskey": "akey"
            }),
        };

        let fetched = provider.fetch(&result).await.unwrap();
        assert_eq!(fetched.input_format, InputFormat::Krc);
        assert_eq!(
            String::from_utf8(fetched.raw).unwrap(),
            "[1000,500]<0,500,0>Hi\n"
        );
    }

    #[tokio::test]
    async fn fetch_falls_back_to_lrc_when_krc_payload_is_invalid() {
        let server = MockServer::start().await;

        let invalid_krc = base64::engine::general_purpose::STANDARD.encode(b"krc1not-deflate");
        Mock::given(method("GET"))
            .and(path("/download"))
            .and(query_param("fmt", "krc"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "content": invalid_krc
            })))
            .mount(&server)
            .await;

        let encoded_lrc = base64::engine::general_purpose::STANDARD.encode("[00:01.00]Fallback\n");
        Mock::given(method("GET"))
            .and(path("/download"))
            .and(query_param("fmt", "lrc"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "content": encoded_lrc
            })))
            .mount(&server)
            .await;

        let provider = KugouProvider::with_endpoint_sets(
            None,
            Vec::new(),
            Vec::new(),
            vec![format!("{}/download", server.uri())],
        )
        .unwrap();

        let result = SearchResult {
            source: Source::Kugou,
            id: "kid".into(),
            title: "Song".into(),
            artist: "Artist".into(),
            album: None,
            duration_ms: Some(1_000),
            extra: json!({
                "id": "kid",
                "accesskey": "akey"
            }),
        };

        let fetched = provider.fetch(&result).await.unwrap();
        assert_eq!(fetched.input_format, InputFormat::Lrc);
        assert_eq!(
            String::from_utf8(fetched.raw).unwrap(),
            "[00:01.00]Fallback\n"
        );
    }
}
