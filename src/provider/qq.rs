use std::time::Duration;

use async_trait::async_trait;
use base64::Engine;
use reqwest::header::{HeaderMap, HeaderValue, COOKIE, REFERER, USER_AGENT};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::decoder::singing_annotations;
use crate::decoder::InputFormat;
use crate::model::Annotation;
use crate::provider::{FetchedLyric, LyricProvider, SearchResult, Source};
use crate::{Error, Result};

const USER_AGENT_VALUE: &str = concat!(
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) rosettrism/",
    env!("CARGO_PKG_VERSION")
);

pub struct QqProvider {
    client: reqwest::Client,
    search_url: String,
    qrc_url: String,
    lrc_url: String,
    annotations_url: String,
}

impl QqProvider {
    pub fn new(cookie: Option<String>) -> Result<Self> {
        let url = "https://u.y.qq.com/cgi-bin/musicu.fcg";
        Self::with_endpoints(
            cookie,
            url,
            "https://c.y.qq.com/qqmusic/fcgi-bin/lyric_download.fcg",
            "https://c.y.qq.com/lyric/fcgi-bin/fcg_query_lyric_new.fcg",
            url,
        )
    }

    fn with_endpoints(
        cookie: Option<String>,
        search_url: impl Into<String>,
        qrc_url: impl Into<String>,
        lrc_url: impl Into<String>,
        annotations_url: impl Into<String>,
    ) -> Result<Self> {
        let mut headers = HeaderMap::new();
        headers.insert(USER_AGENT, HeaderValue::from_static(USER_AGENT_VALUE));
        headers.insert(
            REFERER,
            HeaderValue::from_static("https://y.qq.com/portal/player.html"),
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
            .timeout(Duration::from_secs(12))
            .build()?;

        Ok(Self {
            client,
            search_url: search_url.into(),
            qrc_url: qrc_url.into(),
            lrc_url: lrc_url.into(),
            annotations_url: annotations_url.into(),
        })
    }

    async fn search_songs(&self, query: &str) -> Result<Vec<QqSong>> {
        let mut songs = Vec::new();
        for page in 1..=5 {
            let page_songs = self.search_songs_page(query, page).await?;
            if page_songs.is_empty() {
                break;
            }
            let page_len = page_songs.len();
            songs.extend(page_songs);
            if page_len < 20 {
                break;
            }
        }
        Ok(songs)
    }

    async fn search_songs_page(&self, query: &str, page: u32) -> Result<Vec<QqSong>> {
        let request = json!({
            "comm": {
                "ct": "19",
                "cv": "1859",
                "uin": "0"
            },
            "req": {
                "method": "DoSearchForQQMusicDesktop",
                "module": "music.search.SearchCgiService",
                "param": {
                    "grp": 1,
                    "num_per_page": 20,
                    "page_num": page,
                    "query": query,
                    "search_type": 0
                }
            }
        });

        let response = self
            .client
            .post(&self.search_url)
            .json(&request)
            .send()
            .await?
            .error_for_status()?
            .json::<QqSearchResponse>()
            .await?;

        if response.code != 0 {
            return Err(Error::Provider(format!(
                "QQ search failed with code {}",
                response.code
            )));
        }

        if let Some(module_code) = response.module_code() {
            if module_code != 0 {
                return Err(Error::Provider(format!(
                    "QQ search module failed with code {module_code}"
                )));
            }
        }

        Ok(response.into_songs())
    }

    async fn download_qrc_xml(&self, song_id: u64) -> Result<Vec<u8>> {
        let song_id = song_id.to_string();
        let response = self
            .client
            .get(&self.qrc_url)
            .query(&[
                ("version", "15"),
                ("miniversion", "82"),
                ("lrctype", "4"),
                ("musicid", song_id.as_str()),
            ])
            .send()
            .await?
            .error_for_status()?;

        Ok(response.bytes().await?.to_vec())
    }

    async fn download_lrc(&self, song_mid: &str) -> Result<Vec<u8>> {
        let response = self
            .client
            .get(&self.lrc_url)
            .query(&[
                ("songmid", song_mid),
                ("pcachetime", "0"),
                ("g_tk", "5381"),
                ("loginUin", "0"),
                ("hostUin", "0"),
                ("format", "json"),
                ("inCharset", "utf8"),
                ("outCharset", "utf-8"),
                ("notice", "0"),
                ("platform", "yqq"),
                ("needNewCode", "0"),
            ])
            .send()
            .await?
            .error_for_status()?;

        let text = response.text().await?;
        let json = parse_json_or_jsonp::<QqLyricResponse>(&text)?;
        if let Some(lyric) = json.lyric {
            return base64::engine::general_purpose::STANDARD
                .decode(lyric.trim())
                .map_err(|err| Error::Provider(format!("invalid QQ lyric base64: {err}")));
        }

        Err(Error::Provider(
            "QQ lyric response did not include lyric".into(),
        ))
    }

    async fn has_singing_annotations(&self, song_id: u64) -> Result<bool> {
        Ok(!self
            .fetch_singing_annotations_lyric(song_id)
            .await?
            .is_empty())
    }

    async fn fetch_singing_annotations_lyric(&self, song_id: u64) -> Result<String> {
        let request = singing_annotations_request(song_id);
        let response = self
            .client
            .post(&self.annotations_url)
            .json(&request)
            .send()
            .await?
            .error_for_status()?
            .json::<Value>()
            .await?;

        Ok(response
            .get("req")
            .and_then(|req| req.get("data"))
            .and_then(|data| data.get("singingAnnotationsLyric"))
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string())
    }

    async fn fetch_singing_annotations(&self, song_id: u64) -> Result<Vec<Annotation>> {
        let lyric = self.fetch_singing_annotations_lyric(song_id).await?;

        if lyric.is_empty() {
            return Ok(Vec::new());
        }

        // The singingAnnotationsLyric field is hex-encoded encrypted data (same as QRC)
        // Try to decrypt it using the QRC decryption logic
        let decrypted = match crate::decoder::qrc::decrypt_payload(lyric.as_bytes()) {
            Ok(text) => text,
            Err(_) => {
                // If decryption fails, try using the raw string directly
                lyric.to_string()
            }
        };

        // The decrypted content may be QRC XML wrapping the actual lyric content
        // Extract the lyric content from XML if needed
        let content = if decrypted.contains("<?xml") || decrypted.contains("LyricContent") {
            match crate::decoder::qrc::decode_raw_lyric_content(decrypted.as_bytes()) {
                Ok(extracted) => extracted,
                Err(_) => decrypted,
            }
        } else {
            decrypted
        };

        // Parse annotations from QRC-format lines
        // QRC lines look like: [16346,3408]^久(16346,349)未(16695,431)放(17126,463)`晴(17589,548)
        // Annotation symbols (^ ` _ ↑ ↓) appear BEFORE the character they annotate
        Ok(singing_annotations::parse_qrc(&content))
    }
}

#[async_trait]
impl LyricProvider for QqProvider {
    async fn search(&self, query: &str) -> Result<Vec<SearchResult>> {
        let songs = self.search_songs(query).await?;
        let mut results = Vec::new();
        for song in songs {
            let QqSong {
                mid,
                id,
                name,
                title,
                album,
                albumname,
                interval,
                singer,
                has_singing_annotations,
            } = song;
            let Some(song_mid) = mid else {
                continue;
            };
            let Some(song_id) = id else {
                continue;
            };
            let has_singing_annotations = has_singing_annotations
                || self.has_singing_annotations(song_id).await.unwrap_or(false);
            let title = name.or(title).unwrap_or_default();
            let album = albumname.or_else(|| album.as_ref().and_then(QqAlbum::display_name));
            let artist = singer
                .iter()
                .filter_map(|singer| singer.display_name())
                .collect::<Vec<_>>()
                .join("/");
            results.push(SearchResult {
                source: Source::Qq,
                id: song_mid.clone(),
                title,
                artist,
                album,
                duration_ms: Some(interval.saturating_mul(1_000)),
                extra: json!({
                    "songmid": song_mid,
                    "songid": song_id,
                    "has_singing_annotations": has_singing_annotations,
                }),
            });
        }

        Ok(results)
    }

    async fn fetch(&self, result: &SearchResult) -> Result<FetchedLyric> {
        let song_mid = result
            .extra
            .get("songmid")
            .and_then(Value::as_str)
            .unwrap_or(result.id.as_str());
        let song_id = result.extra.get("songid").and_then(Value::as_u64);

        // Fetch singing annotations when song_id is available (independent of lyric format)
        let annotations = if let Some(song_id) = song_id {
            match self.fetch_singing_annotations(song_id).await {
                Ok(annotations) => annotations,
                Err(err) => {
                    eprintln!("[warn] singing annotations fetch failed: {err}");
                    Vec::new()
                }
            }
        } else {
            Vec::new()
        };

        let mut errors = Vec::new();

        if let Some(song_id) = song_id {
            match self.download_qrc_xml(song_id).await {
                Ok(raw) if looks_like_qrc_response(&raw) => {
                    return Ok(FetchedLyric {
                        input_format: InputFormat::Qrc,
                        raw,
                        document: None,
                        annotations,
                    });
                }
                Ok(_) => errors.push("qrc: empty or malformed response".to_string()),
                Err(err) => errors.push(format!("qrc: {err}")),
            }
        } else {
            errors.push("qrc: missing songid".to_string());
        }

        if !song_mid.trim().is_empty() {
            match self.download_lrc(song_mid).await {
                Ok(raw) if !raw.is_empty() => {
                    return Ok(FetchedLyric {
                        input_format: InputFormat::Lrc,
                        raw,
                        document: None,
                        annotations,
                    });
                }
                Ok(_) => errors.push("lrc: empty response".to_string()),
                Err(err) => errors.push(format!("lrc: {err}")),
            }
        } else {
            errors.push("lrc: missing songmid".to_string());
        }

        Err(Error::Provider(format!(
            "QQ download failed: {}",
            errors.join("; ")
        )))
    }
}

fn looks_like_qrc_response(raw: &[u8]) -> bool {
    if raw.is_empty() {
        return false;
    }

    if raw.starts_with(&[
        0x98, 0x25, 0xb0, 0xac, 0xe3, 0x02, 0x83, 0x68, 0xe8, 0xfc, 0x6c,
    ]) {
        return true;
    }

    let sample_len = raw.len().min(4096);
    let sample = String::from_utf8_lossy(&raw[..sample_len]);
    let trimmed = sample.trim_start_matches('\u{feff}').trim_start();
    trimmed.starts_with("<?xml")
        || trimmed.starts_with("<QrcInfos")
        || trimmed.contains("LyricContent=")
        || (trimmed.contains("<content") && trimmed.contains("<![CDATA["))
}

fn singing_annotations_request(song_id: u64) -> Value {
    json!({
        "comm": {
            "ct": "19",
            "cv": "1859",
            "uin": "0"
        },
        "req": {
            "module": "music.musichallSong.PlayLyricInfo",
            "method": "GetPlayLyricInfo",
            "param": {
                "songID": song_id,
                "type": 0,
                "cmd": 1,
                "qrc": 1,
                "trans": 1,
                "roma": 1,
                "crypt": 0,
                "needSingingAnnotations": true,
                "singingAnnotationsTs": 0,
                "needLTLyric": false,
                "lrc_t": 0,
                "qrc_t": 0,
                "trans_t": 0,
                "roma_t": 0,
                "lt_lyric_t": 0
            }
        }
    })
}

fn deserialize_annotation_flag<'de, D>(deserializer: D) -> std::result::Result<bool, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = Value::deserialize(deserializer)?;
    Ok(match value {
        Value::Bool(value) => value,
        Value::Number(value) => value.as_i64().is_some_and(|value| value != 0),
        Value::String(value) => {
            !value.trim().is_empty() && value != "0" && !value.eq_ignore_ascii_case("false")
        }
        Value::Array(value) => !value.is_empty(),
        Value::Object(value) => !value.is_empty(),
        Value::Null => false,
    })
}

fn parse_json_or_jsonp<T>(text: &str) -> Result<T>
where
    T: for<'de> Deserialize<'de>,
{
    let trimmed = text.trim();
    let json_text = if trimmed.starts_with('{') {
        trimmed
    } else {
        let start = trimmed
            .find('(')
            .ok_or_else(|| Error::Provider("JSONP response missing opening paren".into()))?;
        let end = trimmed
            .rfind(')')
            .ok_or_else(|| Error::Provider("JSONP response missing closing paren".into()))?;
        &trimmed[start + 1..end]
    };

    Ok(serde_json::from_str(json_text)?)
}

#[cfg(test)]
mod tests {
    use serde::Deserialize;

    use super::*;

    #[derive(Debug, Deserialize, PartialEq)]
    struct Mini {
        code: u32,
    }

    #[test]
    fn parses_plain_json() {
        let parsed: Mini = parse_json_or_jsonp(r#"{"code":0}"#).unwrap();
        assert_eq!(parsed, Mini { code: 0 });
    }

    #[test]
    fn parses_jsonp() {
        let parsed: Mini = parse_json_or_jsonp(r#"callback({"code":0})"#).unwrap();
        assert_eq!(parsed, Mini { code: 0 });
    }

    #[tokio::test]
    async fn searches_and_falls_back_to_lrc_with_cookie() {
        use base64::Engine;
        use serde_json::json;
        use wiremock::matchers::{header, method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/search"))
            .and(header("cookie", "qq=1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "data": {
                    "song": {
                        "list": [{
                            "songmid": "mid",
                            "songid": 123,
                            "songname": "Song",
                            "albumname": "Album",
                            "interval": 1,
                            "singer": [{"name": "Artist"}]
                        }]
                    }
                }
            })))
            .mount(&server)
            .await;

        Mock::given(method("POST"))
            .and(path("/annotations"))
            .and(header("cookie", "qq=1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "req": { "data": { "singingAnnotationsLyric": "abc" } }
            })))
            .mount(&server)
            .await;

        Mock::given(method("GET"))
            .and(path("/qrc"))
            .and(header("cookie", "qq=1"))
            .respond_with(ResponseTemplate::new(200).set_body_string(""))
            .mount(&server)
            .await;

        let lyric = base64::engine::general_purpose::STANDARD.encode("[00:01.00]Hi\n");
        Mock::given(method("GET"))
            .and(path("/lrc"))
            .and(header("cookie", "qq=1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "lyric": lyric
            })))
            .mount(&server)
            .await;

        let provider = QqProvider::with_endpoints(
            Some("qq=1".into()),
            format!("{}/search", server.uri()),
            format!("{}/qrc", server.uri()),
            format!("{}/lrc", server.uri()),
            format!("{}/annotations", server.uri()),
        )
        .unwrap();

        let results = provider.search("Song Artist").await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].artist, "Artist");
        assert_eq!(results[0].extra["has_singing_annotations"], true);

        let fetched = provider.fetch(&results[0]).await.unwrap();
        assert_eq!(fetched.input_format, InputFormat::Lrc);
        assert_eq!(String::from_utf8(fetched.raw).unwrap(), "[00:01.00]Hi\n");
    }
}

#[derive(Debug, Deserialize)]
struct QqSearchResponse {
    #[serde(default)]
    code: i32,
    #[serde(default)]
    req: Option<QqSearchModule>,
    #[serde(default)]
    data: Option<QqLegacySearchData>,
}

impl QqSearchResponse {
    fn module_code(&self) -> Option<i32> {
        self.req.as_ref().map(|req| req.code)
    }

    fn into_songs(self) -> Vec<QqSong> {
        if let Some(req) = self.req {
            if let Some(data) = req.data {
                return data.body.song.list;
            }
        }

        self.data.map(|data| data.song.list).unwrap_or_default()
    }
}

#[derive(Debug, Deserialize)]
struct QqSearchModule {
    #[serde(default)]
    code: i32,
    #[serde(default)]
    data: Option<QqSearchData>,
}

#[derive(Debug, Deserialize)]
struct QqSearchData {
    body: QqSearchBody,
}

#[derive(Debug, Deserialize)]
struct QqSearchBody {
    song: QqSongData,
}

#[derive(Debug, Deserialize)]
struct QqLegacySearchData {
    song: QqSongData,
}

#[derive(Debug, Deserialize)]
struct QqSongData {
    #[serde(default)]
    list: Vec<QqSong>,
}

#[derive(Debug, Deserialize)]
struct QqSong {
    #[serde(default, alias = "songmid")]
    mid: Option<String>,
    #[serde(default, alias = "songid")]
    id: Option<u64>,
    #[serde(default, alias = "songname")]
    name: Option<String>,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    album: Option<QqAlbum>,
    #[serde(default)]
    albumname: Option<String>,
    #[serde(default)]
    interval: u32,
    #[serde(default)]
    singer: Vec<QqSinger>,
    #[serde(
        default,
        alias = "hasSingingAnnotations",
        alias = "has_singing_annotations",
        alias = "hasSingingAnnotationsLyric",
        deserialize_with = "deserialize_annotation_flag"
    )]
    has_singing_annotations: bool,
}

#[derive(Debug, Deserialize)]
struct QqAlbum {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    title: Option<String>,
}

impl QqAlbum {
    fn display_name(&self) -> Option<String> {
        self.name.clone().or_else(|| self.title.clone())
    }
}

#[derive(Debug, Deserialize)]
struct QqSinger {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    title: Option<String>,
}

impl QqSinger {
    fn display_name(&self) -> Option<&str> {
        self.name.as_deref().or(self.title.as_deref())
    }
}

#[derive(Debug, Deserialize)]
struct QqLyricResponse {
    #[serde(default)]
    lyric: Option<String>,
}
