use async_trait::async_trait;
use reqwest::header::{HeaderMap, HeaderValue, ACCEPT, COOKIE, REFERER, USER_AGENT};
use serde::de::DeserializeOwned;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::decoder::{decode_bytes, InputFormat};
use crate::provider::{FetchedLyric, LyricProvider, SearchResult, Source};
use crate::{Error, Result};

const USER_AGENT_VALUE: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36";

pub struct NeteaseProvider {
    client: reqwest::Client,
    search_urls: Vec<String>,
    lyric_urls: Vec<String>,
}

impl NeteaseProvider {
    pub fn new(cookie: Option<String>, timeout_ms: u64) -> Result<Self> {
        Self::with_urls(
            cookie,
            vec![
                "https://music.163.com/api/search/get/web".to_string(),
                "http://music.163.com/api/search/get/web".to_string(),
            ],
            vec![
                "https://music.163.com/api/song/lyric/v1".to_string(),
                "https://music.163.com/api/song/lyric".to_string(),
                "http://music.163.com/api/song/lyric/v1".to_string(),
                "http://music.163.com/api/song/lyric".to_string(),
            ],
            timeout_ms,
        )
    }

    #[cfg(test)]
    fn with_endpoints(
        cookie: Option<String>,
        search_url: impl Into<String>,
        lyric_url: impl Into<String>,
        timeout_ms: u64,
    ) -> Result<Self> {
        Self::with_urls(
            cookie,
            vec![search_url.into()],
            vec![lyric_url.into()],
            timeout_ms,
        )
    }

    fn with_urls(
        cookie: Option<String>,
        search_urls: Vec<String>,
        lyric_urls: Vec<String>,
        timeout_ms: u64,
    ) -> Result<Self> {
        let mut headers = HeaderMap::new();
        headers.insert(USER_AGENT, HeaderValue::from_static(USER_AGENT_VALUE));
        headers.insert(
            ACCEPT,
            HeaderValue::from_static("application/json, text/plain, */*"),
        );
        headers.insert(REFERER, HeaderValue::from_static("https://music.163.com/"));
        if let Some(cookie) = cookie {
            if !cookie.trim().is_empty() {
                headers.insert(
                    COOKIE,
                    HeaderValue::from_str(cookie.trim())
                        .map_err(|err| Error::Provider(format!("invalid cookie header: {err}")))?,
                );
            }
        }

        let client = crate::provider::apply_client_timeout(
            reqwest::Client::builder().default_headers(headers),
            timeout_ms,
        )
        .build()?;

        Ok(Self {
            client,
            search_urls,
            lyric_urls,
        })
    }

    async fn search_songs(&self, query: &str) -> Result<Vec<NeteaseSong>> {
        let mut errors = Vec::new();

        for url in &self.search_urls {
            match self.search_songs_from_url(url, query).await {
                Ok(songs) => return Ok(songs),
                Err(err) => errors.push(format!("{url}: {err}")),
            }
        }

        Err(Error::Provider(format!(
            "Netease search failed: {}",
            errors.join("; ")
        )))
    }

    async fn search_songs_from_url(&self, url: &str, query: &str) -> Result<Vec<NeteaseSong>> {
        let response = send_json::<NeteaseSearchResponse>(
            self.client.post(url).form(&[
                ("s", query),
                ("type", "1"),
                ("offset", "0"),
                ("total", "true"),
                ("limit", "100"),
            ]),
            "Netease search",
        )
        .await?;

        if response.code != 200 {
            return Err(Error::Provider(format!(
                "Netease search failed with code {}",
                response.code
            )));
        }

        Ok(response
            .result
            .map(|result| result.songs)
            .unwrap_or_default())
    }

    async fn download_lyric(&self, song_id: u64) -> Result<FetchedLyric> {
        let mut errors = Vec::new();

        for url in &self.lyric_urls {
            match self.download_lyric_from_endpoint(url, song_id).await {
                Ok(Some(fetched)) => return Ok(fetched),
                Ok(None) => errors.push(format!("{url}: no usable lyric")),
                Err(err) => errors.push(format!("{url}: {err}")),
            }
        }

        Err(Error::Provider(format!(
            "Netease lyric download failed: {}",
            errors.join("; ")
        )))
    }

    async fn download_lyric_from_endpoint(
        &self,
        url: &str,
        song_id: u64,
    ) -> Result<Option<FetchedLyric>> {
        let song_id = song_id.to_string();
        let response = send_json::<NeteaseLyricResponse>(
            self.client.get(url).query(&[
                ("id", song_id.as_str()),
                ("lv", "-1"),
                ("kv", "-1"),
                ("tv", "-1"),
                ("rv", "-1"),
                ("yv", "-1"),
                ("ytv", "-1"),
                ("yrv", "-1"),
                ("cp", "false"),
            ]),
            "Netease lyric",
        )
        .await?;

        if response.code != 200 {
            return Err(Error::Provider(format!(
                "Netease lyric failed with code {}",
                response.code
            )));
        }

        if response.nolyric || response.pure_music {
            return Ok(None);
        }

        let translation = response.tlyric.and_then(NeteaseLyricPart::into_lyric);
        let romanized = response.romalrc.and_then(NeteaseLyricPart::into_lyric);

        if let Some(raw) = response.yrc.and_then(NeteaseLyricPart::into_lyric) {
            return Ok(Some(with_extra_tracks(
                InputFormat::Yrc,
                raw,
                translation.as_deref(),
                romanized.as_deref(),
            )?));
        }

        if let Some(raw) = response.klyric.and_then(NeteaseLyricPart::into_lyric) {
            return Ok(Some(with_extra_tracks(
                InputFormat::Yrc,
                raw,
                translation.as_deref(),
                romanized.as_deref(),
            )?));
        }

        if let Some(raw) = response.lrc.and_then(NeteaseLyricPart::into_lyric) {
            return Ok(Some(with_extra_tracks(
                InputFormat::Lrc,
                raw,
                translation.as_deref(),
                romanized.as_deref(),
            )?));
        }

        Ok(None)
    }
}

#[async_trait]
impl LyricProvider for NeteaseProvider {
    async fn search(&self, query: &str) -> Result<Vec<SearchResult>> {
        let songs = self.search_songs(query).await?;
        let results = songs
            .into_iter()
            .filter_map(|song| {
                let id = song.id?;
                let title = song.name.unwrap_or_default();
                let artist = song
                    .artists
                    .iter()
                    .filter_map(|artist| artist.name.as_deref())
                    .collect::<Vec<_>>()
                    .join("/");
                let album = song.album.and_then(|album| album.name);

                Some(SearchResult {
                    source: Source::Netease,
                    id: id.to_string(),
                    title,
                    artist,
                    album,
                    duration_ms: song.duration_ms,
                    extra: json!({
                        "songid": id
                    }),
                })
            })
            .collect();

        Ok(results)
    }

    async fn fetch(&self, result: &SearchResult) -> Result<FetchedLyric> {
        let song_id = result
            .extra
            .get("songid")
            .and_then(Value::as_u64)
            .or_else(|| result.id.parse::<u64>().ok())
            .ok_or_else(|| Error::Provider("Netease candidate is missing songid".into()))?;

        self.download_lyric(song_id).await
    }
}

fn with_extra_tracks(
    input_format: InputFormat,
    raw: String,
    translation: Option<&str>,
    romanized: Option<&str>,
) -> Result<FetchedLyric> {
    let raw_bytes = raw.into_bytes();
    let mut document = decode_bytes(&raw_bytes, input_format)?;
    apply_timed_lrc_track(&mut document.lines, translation, |line, text| {
        line.translation = Some(text);
    })?;
    apply_timed_lrc_track(&mut document.lines, romanized, |line, text| {
        line.romanized = Some(text);
    })?;
    Ok(FetchedLyric {
        input_format,
        raw: raw_bytes,
        document: Some(document),
        annotations: Vec::new(),
    })
}

fn apply_timed_lrc_track(
    lines: &mut [crate::model::LyricLine],
    raw: Option<&str>,
    mut apply: impl FnMut(&mut crate::model::LyricLine, String),
) -> Result<()> {
    let Some(raw) = raw.filter(|value| !value.trim().is_empty()) else {
        return Ok(());
    };
    let track = crate::decoder::lrc::parse(raw)?;
    for track_line in track.lines {
        if track_line.text.trim().is_empty() {
            continue;
        }
        if let Some(line) = lines
            .iter_mut()
            .min_by_key(|line| line.start_ms.abs_diff(track_line.start_ms))
        {
            if line.start_ms.abs_diff(track_line.start_ms) <= 800 {
                apply(line, track_line.text);
            }
        }
    }
    Ok(())
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

#[derive(Debug, Deserialize)]
struct NeteaseSearchResponse {
    #[serde(default)]
    code: i32,
    #[serde(default)]
    result: Option<NeteaseSearchResult>,
}

#[derive(Debug, Deserialize)]
struct NeteaseSearchResult {
    #[serde(default)]
    songs: Vec<NeteaseSong>,
}

#[derive(Debug, Deserialize)]
struct NeteaseSong {
    #[serde(default)]
    id: Option<u64>,
    #[serde(default)]
    name: Option<String>,
    #[serde(default, alias = "dt", alias = "duration")]
    duration_ms: Option<u32>,
    #[serde(default, alias = "ar")]
    artists: Vec<NeteaseArtist>,
    #[serde(default, alias = "al")]
    album: Option<NeteaseAlbum>,
}

#[derive(Debug, Deserialize)]
struct NeteaseArtist {
    #[serde(default)]
    name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct NeteaseAlbum {
    #[serde(default)]
    name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct NeteaseLyricResponse {
    #[serde(default)]
    code: i32,
    #[serde(default)]
    nolyric: bool,
    #[serde(default, alias = "pureMusic")]
    pure_music: bool,
    #[serde(default)]
    yrc: Option<NeteaseLyricPart>,
    #[serde(default)]
    klyric: Option<NeteaseLyricPart>,
    #[serde(default)]
    lrc: Option<NeteaseLyricPart>,
    #[serde(default)]
    tlyric: Option<NeteaseLyricPart>,
    #[serde(default)]
    romalrc: Option<NeteaseLyricPart>,
}

#[derive(Debug, Deserialize)]
struct NeteaseLyricPart {
    #[serde(default)]
    lyric: Option<String>,
}

impl NeteaseLyricPart {
    fn into_lyric(self) -> Option<String> {
        self.lyric.filter(|lyric| !lyric.trim().is_empty())
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use wiremock::matchers::{header, method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::*;

    #[tokio::test]
    async fn searches_and_fetches_yrc_with_cookie() {
        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/search"))
            .and(header("cookie", "nt=1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "code": 200,
                "result": {
                    "songs": [{
                        "id": 123,
                        "name": "Song",
                        "duration": 3090,
                        "artists": [{"name": "Artist"}],
                        "album": {"name": "Album"}
                    }]
                }
            })))
            .mount(&server)
            .await;

        let yrc = "[54260,3090](54260,900,0)Stop (55160,480,0)and\n";
        Mock::given(method("GET"))
            .and(path("/lyric"))
            .and(header("cookie", "nt=1"))
            .and(query_param("id", "123"))
            .and(query_param("yv", "-1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "code": 200,
                "yrc": {
                    "lyric": yrc
                }
            })))
            .mount(&server)
            .await;

        let provider = NeteaseProvider::with_endpoints(
            Some("nt=1".into()),
            format!("{}/search", server.uri()),
            format!("{}/lyric", server.uri()),
            12_000,
        )
        .unwrap();

        let results = provider.search("Song Artist").await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].source, Source::Netease);
        assert_eq!(results[0].artist, "Artist");

        let fetched = provider.fetch(&results[0]).await.unwrap();
        assert_eq!(fetched.input_format, InputFormat::Yrc);
        assert_eq!(String::from_utf8(fetched.raw).unwrap(), yrc);
    }

    #[tokio::test]
    async fn fetch_falls_back_to_lrc_when_yrc_is_empty() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/lyric"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "code": 200,
                "yrc": {
                    "lyric": ""
                },
                "lrc": {
                    "lyric": "[00:01.00]Hi\n"
                }
            })))
            .mount(&server)
            .await;

        let provider = NeteaseProvider::with_endpoints(
            None,
            format!("{}/search", server.uri()),
            format!("{}/lyric", server.uri()),
            12_000,
        )
        .unwrap();
        let result = SearchResult {
            source: Source::Netease,
            id: "123".into(),
            title: "Song".into(),
            artist: "Artist".into(),
            album: None,
            duration_ms: Some(1_000),
            extra: json!({
                "songid": 123
            }),
        };

        let fetched = provider.fetch(&result).await.unwrap();
        assert_eq!(fetched.input_format, InputFormat::Lrc);
        assert_eq!(String::from_utf8(fetched.raw).unwrap(), "[00:01.00]Hi\n");
    }
}
