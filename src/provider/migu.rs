use async_trait::async_trait;
use regex::Regex;
use reqwest::header::{HeaderMap, HeaderValue, ACCEPT, COOKIE, REFERER, USER_AGENT};
use serde::de::DeserializeOwned;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::decoder::InputFormat;
use crate::provider::{FetchedLyric, LyricProvider, SearchResult, Source};
use crate::{Error, Result};

const USER_AGENT_VALUE: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36";

pub struct MiguProvider {
    client: reqwest::Client,
    search_url: String,
    lyric_url: String,
}

impl MiguProvider {
    pub fn new(cookie: Option<String>, timeout_ms: u64) -> Result<Self> {
        Self::with_endpoints(
            cookie,
            "https://m.music.migu.cn/migu/remoting/scr_search_tag",
            "https://music.migu.cn/v3/api/music/audioPlayer/getLyric",
            timeout_ms,
        )
    }

    fn with_endpoints(
        cookie: Option<String>,
        search_url: impl Into<String>,
        lyric_url: impl Into<String>,
        timeout_ms: u64,
    ) -> Result<Self> {
        let mut headers = HeaderMap::new();
        headers.insert(USER_AGENT, HeaderValue::from_static(USER_AGENT_VALUE));
        headers.insert(
            ACCEPT,
            HeaderValue::from_static("application/json, text/plain, */*"),
        );
        headers.insert(
            REFERER,
            HeaderValue::from_static("http://m.music.migu.cn/v3"),
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

        let client = crate::provider::apply_client_timeout(
            reqwest::Client::builder().default_headers(headers),
            timeout_ms,
        )
        .build()?;

        Ok(Self {
            client,
            search_url: search_url.into(),
            lyric_url: lyric_url.into(),
        })
    }

    async fn search_h5(&self, query: &str) -> Result<Vec<SearchResult>> {
        let body = self
            .client
            .get(&self.search_url)
            .query(&[
                ("keyword", query),
                ("pgc", "1"),
                ("rows", "100"),
                ("type", "2"),
            ])
            .send()
            .await?
            .error_for_status()?
            .text()
            .await?;
        let response: MiguSearchResponse = parse_json_or_jsonp(&body)?;

        Ok(response
            .musics
            .into_iter()
            .filter_map(MiguSong::into_result)
            .collect())
    }

    async fn download_lyric(&self, copyright_id: &str) -> Result<FetchedLyric> {
        let body = self
            .client
            .get(&self.lyric_url)
            .query(&[("copyrightId", copyright_id)])
            .send()
            .await?
            .error_for_status()?
            .text()
            .await?;
        let response: MiguLyricResponse = parse_json_or_jsonp(&body)?;
        let lyric = response
            .lyric
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| {
                let detail = response
                    .msg
                    .filter(|value| !value.trim().is_empty())
                    .unwrap_or_else(|| "empty lyric response".to_string());
                Error::Provider(format!("Migu lyric fetch failed: {detail}"))
            })?;

        Ok(FetchedLyric {
            input_format: if looks_like_lrc(&lyric) {
                InputFormat::Lrc
            } else {
                InputFormat::Text
            },
            raw: ensure_trailing_newline(lyric).into_bytes(),
            document: None,
            annotations: Vec::new(),
        })
    }
}

#[async_trait]
impl LyricProvider for MiguProvider {
    async fn search(&self, query: &str) -> Result<Vec<SearchResult>> {
        self.search_h5(query).await
    }

    async fn fetch(&self, result: &SearchResult) -> Result<FetchedLyric> {
        let copyright_id = result
            .extra
            .get("copyright_id")
            .and_then(value_to_string)
            .unwrap_or_else(|| result.id.clone());

        if copyright_id.trim().is_empty() {
            return Err(Error::Provider(
                "Migu result did not include a copyright id".into(),
            ));
        }

        self.download_lyric(&copyright_id).await
    }
}

#[derive(Debug, Default, Deserialize)]
struct MiguSearchResponse {
    #[serde(default)]
    musics: Vec<MiguSong>,
}

#[derive(Debug, Deserialize)]
struct MiguSong {
    #[serde(default, rename = "songName")]
    song_name: Option<String>,
    #[serde(default, rename = "singerName")]
    singer_name: Option<String>,
    #[serde(default, rename = "albumName")]
    album_name: Option<String>,
    #[serde(default)]
    id: Option<Value>,
    #[serde(default, rename = "copyrightId")]
    copyright_id: Option<Value>,
}

impl MiguSong {
    fn into_result(self) -> Option<SearchResult> {
        let copyright_id = self.copyright_id.as_ref().and_then(value_to_string)?;
        let title = self
            .song_name
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| format!("Migu {copyright_id}"));
        let artist = self.singer_name.unwrap_or_default();

        Some(SearchResult {
            source: Source::Migu,
            id: copyright_id.clone(),
            title,
            artist,
            album: self.album_name.filter(|value| !value.trim().is_empty()),
            duration_ms: None,
            extra: json!({
                "copyright_id": copyright_id,
                "song_id": self.id.and_then(|value| value_to_string(&value))
            }),
        })
    }
}

#[derive(Debug, Default, Deserialize)]
struct MiguLyricResponse {
    #[serde(default)]
    msg: Option<String>,
    #[serde(default)]
    lyric: Option<String>,
}

fn parse_json_or_jsonp<T>(body: &str) -> Result<T>
where
    T: DeserializeOwned,
{
    let trimmed = body.trim();
    let json = if let Some(start) = trimmed.find('(') {
        if trimmed.ends_with(')') && !trimmed.starts_with('{') && !trimmed.starts_with('[') {
            &trimmed[start + 1..trimmed.len() - 1]
        } else {
            trimmed
        }
    } else {
        trimmed
    };

    Ok(serde_json::from_str(json)?)
}

fn value_to_string(value: &Value) -> Option<String> {
    if value.is_null() {
        return None;
    }

    value
        .as_str()
        .map(ToOwned::to_owned)
        .or_else(|| value.as_u64().map(|number| number.to_string()))
        .or_else(|| value.as_i64().map(|number| number.to_string()))
}

fn looks_like_lrc(value: &str) -> bool {
    Regex::new(r"(?m)\[\d{1,2}:\d{2}(?:[.:]\d{1,3})?\]")
        .ok()
        .is_some_and(|re| re.is_match(value))
}

fn ensure_trailing_newline(mut value: String) -> String {
    if !value.ends_with('\n') {
        value.push('\n');
    }
    value
}

#[cfg(test)]
mod tests {
    use wiremock::matchers::{method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::*;

    fn provider(server: &MockServer) -> MiguProvider {
        MiguProvider::with_endpoints(
            None,
            format!("{}/migu/remoting/scr_search_tag", server.uri()),
            format!("{}/v3/api/music/audioPlayer/getLyric", server.uri()),
            12_000,
        )
        .unwrap()
    }

    #[tokio::test]
    async fn searches_h5_results_and_skips_missing_copyright_ids() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/migu/remoting/scr_search_tag"))
            .and(query_param("keyword", "Song"))
            .and(query_param("type", "2"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "musics": [
                    {
                        "songName": "Song",
                        "singerName": "Artist",
                        "albumName": "Album",
                        "id": "sid-1",
                        "copyrightId": "cid-1"
                    },
                    {
                        "songName": "No Copyright",
                        "singerName": "Artist"
                    }
                ]
            })))
            .mount(&server)
            .await;

        let results = provider(&server).search("Song").await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].source, Source::Migu);
        assert_eq!(results[0].id, "cid-1");
        assert_eq!(results[0].title, "Song");
        assert_eq!(results[0].artist, "Artist");
        assert_eq!(results[0].album.as_deref(), Some("Album"));
        assert_eq!(results[0].extra["song_id"], "sid-1");
    }

    #[tokio::test]
    async fn fetches_lrc_lyrics() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/v3/api/music/audioPlayer/getLyric"))
            .and(query_param("copyrightId", "cid-1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "msg": "success",
                "lyric": "[00:01.00]Hi"
            })))
            .mount(&server)
            .await;

        let result = SearchResult {
            source: Source::Migu,
            id: "cid-1".into(),
            title: "Song".into(),
            artist: "Artist".into(),
            album: None,
            duration_ms: None,
            extra: json!({ "copyright_id": "cid-1" }),
        };

        let fetched = provider(&server).fetch(&result).await.unwrap();
        assert_eq!(fetched.input_format, InputFormat::Lrc);
        assert_eq!(String::from_utf8(fetched.raw).unwrap(), "[00:01.00]Hi\n");
    }

    #[tokio::test]
    async fn fetches_plain_text_lyrics() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/v3/api/music/audioPlayer/getLyric"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "msg": "success",
                "lyric": "Hi\nthere"
            })))
            .mount(&server)
            .await;

        let result = SearchResult {
            source: Source::Migu,
            id: "cid-1".into(),
            title: "Song".into(),
            artist: "Artist".into(),
            album: None,
            duration_ms: None,
            extra: json!({}),
        };

        let fetched = provider(&server).fetch(&result).await.unwrap();
        assert_eq!(fetched.input_format, InputFormat::Text);
        assert_eq!(String::from_utf8(fetched.raw).unwrap(), "Hi\nthere\n");
    }

    #[tokio::test]
    async fn lyric_errors_are_readable() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/v3/api/music/audioPlayer/getLyric"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "msg": "not available",
                "lyric": ""
            })))
            .mount(&server)
            .await;

        let result = SearchResult {
            source: Source::Migu,
            id: "cid-1".into(),
            title: "Song".into(),
            artist: "Artist".into(),
            album: None,
            duration_ms: None,
            extra: json!({}),
        };

        let err = provider(&server)
            .fetch(&result)
            .await
            .unwrap_err()
            .to_string();
        assert!(err.contains("Migu lyric fetch failed"));
        assert!(err.contains("not available"));
    }
}
