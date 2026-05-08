use std::path::PathBuf;

use async_trait::async_trait;
use rusqlite::{Connection, OpenFlags, OptionalExtension, Row};
use serde_json::{json, Value};

use crate::decoder::{decode_bytes, detect_format, InputFormat};
use crate::model::LyricDocument;
use crate::provider::{FetchedLyric, LyricProvider, SearchResult, Source};
use crate::{Error, Result};

#[derive(Debug)]
pub struct OfflineDbProvider {
    db_path: PathBuf,
}

impl OfflineDbProvider {
    pub fn new(db_path: Option<String>) -> Result<Self> {
        let db_path = db_path
            .or_else(|| env_var_any(&["ROSETTRISM_OFFLINE_DB", "LRC_DECODE_OFFLINE_DB"]))
            .ok_or_else(|| {
                Error::Provider(
                    "offline-db requires a SQLite path via --offline-db or ROSETTRISM_OFFLINE_DB"
                        .into(),
                )
            })?;

        Ok(Self {
            db_path: PathBuf::from(db_path),
        })
    }

    fn connect(&self) -> Result<Connection> {
        Connection::open_with_flags(&self.db_path, OpenFlags::SQLITE_OPEN_READ_ONLY).map_err(
            |err| {
                Error::Provider(format!(
                    "Offline DB open failed for {}: {err}",
                    self.db_path.display()
                ))
            },
        )
    }

    fn search_db(&self, query: &str) -> Result<Vec<SearchResult>> {
        let connection = self.connect()?;
        let like = format!("%{}%", escape_like(query));
        let mut statement = connection
            .prepare(
                r#"
                SELECT id, title, artist, album, duration_ms, source, format, text, reading, romanized, metadata_json
                FROM lyrics
                WHERE lower(title) LIKE lower(?1) ESCAPE '\'
                   OR lower(artist) LIKE lower(?1) ESCAPE '\'
                   OR lower(coalesce(album, '')) LIKE lower(?1) ESCAPE '\'
                ORDER BY
                    CASE WHEN lower(title) LIKE lower(?1) ESCAPE '\' THEN 0 ELSE 1 END,
                    title,
                    artist
                LIMIT 20
                "#,
            )
            .map_err(|err| sqlite_provider_error("prepare search", err))?;

        let rows = statement
            .query_map([like], row_to_lyric)
            .map_err(|err| sqlite_provider_error("query search", err))?;

        let mut results = Vec::new();
        for row in rows {
            let lyric = row.map_err(|err| sqlite_provider_error("read search row", err))?;
            results.push(lyric.into_result());
        }

        Ok(results)
    }

    fn fetch_db(&self, id: &str) -> Result<FetchedLyric> {
        let connection = self.connect()?;
        let mut statement = connection
            .prepare(
                r#"
                SELECT id, title, artist, album, duration_ms, source, format, text, reading, romanized, metadata_json
                FROM lyrics
                WHERE id = ?1
                LIMIT 1
                "#,
            )
            .map_err(|err| sqlite_provider_error("prepare fetch", err))?;

        let lyric = statement
            .query_row([id], row_to_lyric)
            .optional()
            .map_err(|err| sqlite_provider_error("query fetch", err))?
            .ok_or_else(|| Error::Provider(format!("Offline DB lyric {id} was not found")))?;

        lyric.into_fetched()
    }
}

#[async_trait]
impl LyricProvider for OfflineDbProvider {
    async fn search(&self, query: &str) -> Result<Vec<SearchResult>> {
        self.search_db(query)
    }

    async fn fetch(&self, result: &SearchResult) -> Result<FetchedLyric> {
        let id = result
            .extra
            .get("offline_db_id")
            .and_then(|value| value.as_str())
            .unwrap_or(result.id.as_str());
        self.fetch_db(id)
    }
}

#[derive(Debug)]
struct OfflineLyric {
    id: String,
    title: String,
    artist: String,
    album: Option<String>,
    duration_ms: Option<u32>,
    source: Option<String>,
    format: Option<String>,
    text: String,
    reading: Option<String>,
    romanized: Option<String>,
    metadata_json: Option<String>,
}

impl OfflineLyric {
    fn into_result(self) -> SearchResult {
        SearchResult {
            source: Source::OfflineDb,
            id: self.id.clone(),
            title: self.title,
            artist: self.artist,
            album: self.album,
            duration_ms: self.duration_ms,
            extra: json!({
                "offline_db_id": self.id,
                "format": self.format,
                "source": self.source,
                "metadata": parse_metadata(self.metadata_json.as_deref()),
            }),
        }
    }

    fn into_fetched(self) -> Result<FetchedLyric> {
        let format = offline_format(self.format.as_deref(), &self.text);
        let raw = if matches!(format, InputFormat::Lrc | InputFormat::Text) {
            ensure_trailing_newline(self.text.clone()).into_bytes()
        } else {
            self.text.clone().into_bytes()
        };

        let mut document = match format {
            InputFormat::Json => serde_json::from_str::<LyricDocument>(&self.text)?,
            InputFormat::Lrc
            | InputFormat::Text
            | InputFormat::AppleMusic
            | InputFormat::Krc
            | InputFormat::Qrc
            | InputFormat::Yrc => decode_bytes(&raw, format)?,
            InputFormat::Auto => decode_bytes(&raw, InputFormat::Auto)?,
        };

        fill_meta(&mut document, &self);
        apply_line_annotations(
            &mut document,
            self.reading.as_deref(),
            self.romanized.as_deref(),
        );

        Ok(FetchedLyric {
            input_format: format,
            raw,
            document: Some(document),
        })
    }
}

fn row_to_lyric(row: &Row<'_>) -> rusqlite::Result<OfflineLyric> {
    let duration_ms = row
        .get::<_, Option<i64>>(4)?
        .and_then(|value| u32::try_from(value).ok());

    Ok(OfflineLyric {
        id: row.get(0)?,
        title: row.get::<_, Option<String>>(1)?.unwrap_or_default(),
        artist: row.get::<_, Option<String>>(2)?.unwrap_or_default(),
        album: row.get(3)?,
        duration_ms,
        source: row.get(5)?,
        format: row.get(6)?,
        text: row.get::<_, Option<String>>(7)?.unwrap_or_default(),
        reading: row.get(8)?,
        romanized: row.get(9)?,
        metadata_json: row.get(10)?,
    })
}

fn fill_meta(document: &mut LyricDocument, lyric: &OfflineLyric) {
    if document.meta.title.is_none() && !lyric.title.trim().is_empty() {
        document.meta.title = Some(lyric.title.clone());
    }
    if document.meta.artist.is_none() && !lyric.artist.trim().is_empty() {
        document.meta.artist = Some(lyric.artist.clone());
    }
    if document.meta.album.is_none() {
        document.meta.album = lyric.album.clone().filter(|value| !value.trim().is_empty());
    }
    if document.meta.source.is_none() {
        document.meta.source = lyric
            .source
            .clone()
            .filter(|value| !value.trim().is_empty())
            .or_else(|| Some("offline-db".into()));
    }
}

fn apply_line_annotations(
    document: &mut LyricDocument,
    reading: Option<&str>,
    romanized: Option<&str>,
) {
    let readings = split_annotation_lines(reading);
    let romanized_lines = split_annotation_lines(romanized);

    for (index, line) in document.lines.iter_mut().enumerate() {
        if line.reading.is_none() {
            line.reading = readings.get(index).cloned();
        }
        if line.romanized.is_none() {
            line.romanized = romanized_lines.get(index).cloned();
        }
    }
}

fn split_annotation_lines(value: Option<&str>) -> Vec<String> {
    value
        .unwrap_or_default()
        .replace("\r\n", "\n")
        .replace('\r', "\n")
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

fn offline_format(value: Option<&str>, text: &str) -> InputFormat {
    match value
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase()
        .as_str()
    {
        "json" | "document" | "rosettrism-json" => InputFormat::Json,
        "lrc" => InputFormat::Lrc,
        "text" | "txt" | "plain" => InputFormat::Text,
        "ttml" | "apple-music" => InputFormat::AppleMusic,
        "krc" => InputFormat::Krc,
        "qrc" => InputFormat::Qrc,
        "yrc" => InputFormat::Yrc,
        _ => {
            let detected = detect_format(text.as_bytes());
            if detected == InputFormat::Auto {
                InputFormat::Text
            } else {
                detected
            }
        }
    }
}

fn parse_metadata(value: Option<&str>) -> Value {
    value
        .and_then(|value| serde_json::from_str(value).ok())
        .unwrap_or(Value::Null)
}

fn ensure_trailing_newline(mut value: String) -> String {
    if !value.ends_with('\n') {
        value.push('\n');
    }
    value
}

fn escape_like(value: &str) -> String {
    let mut escaped = String::new();
    for ch in value.chars() {
        if matches!(ch, '%' | '_' | '\\') {
            escaped.push('\\');
        }
        escaped.push(ch);
    }
    escaped
}

fn sqlite_provider_error(action: &str, err: rusqlite::Error) -> Error {
    Error::Provider(format!("Offline DB {action} failed: {err}"))
}

fn env_var_any(names: &[&str]) -> Option<String> {
    names
        .iter()
        .find_map(|name| std::env::var(name).ok())
        .filter(|value| !value.trim().is_empty())
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;
    use std::time::{SystemTime, UNIX_EPOCH};

    use crate::exporter::{export_document, OutputFormat};

    use super::*;

    fn temp_db_path(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "rosettrism-offline-db-{name}-{}-{}.sqlite",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }

    fn create_db(path: &Path) {
        let connection = Connection::open(path).unwrap();
        connection
            .execute_batch(
                r#"
                CREATE TABLE lyrics (
                    id TEXT PRIMARY KEY,
                    title TEXT,
                    artist TEXT,
                    album TEXT,
                    duration_ms INTEGER,
                    source TEXT,
                    format TEXT,
                    text TEXT,
                    reading TEXT,
                    romanized TEXT,
                    metadata_json TEXT
                );
                "#,
            )
            .unwrap();
    }

    fn provider(path: &Path) -> OfflineDbProvider {
        OfflineDbProvider::new(Some(path.display().to_string())).unwrap()
    }

    #[tokio::test]
    async fn searches_and_fetches_lrc_with_annotations() {
        let path = temp_db_path("lrc");
        create_db(&path);
        let connection = Connection::open(&path).unwrap();
        connection
            .execute(
                "INSERT INTO lyrics VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
                (
                    "song-1",
                    "Song",
                    "Artist",
                    "Album",
                    123_000_i64,
                    "petitlyrics-dump",
                    "lrc",
                    "[00:01.00]日々\n[00:02.00]歌う",
                    "ひび\nうたう",
                    "hibi\nutau",
                    r#"{"rank":1}"#,
                ),
            )
            .unwrap();

        let provider = provider(&path);
        let results = provider.search("Song").await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].source, Source::OfflineDb);
        assert_eq!(results[0].duration_ms, Some(123_000));
        assert_eq!(results[0].extra["metadata"]["rank"], 1);

        let fetched = provider.fetch(&results[0]).await.unwrap();
        assert_eq!(fetched.input_format, InputFormat::Lrc);
        let document = fetched.document.unwrap();
        assert_eq!(document.meta.source.as_deref(), Some("petitlyrics-dump"));
        assert_eq!(document.lines[0].reading.as_deref(), Some("ひび"));
        assert_eq!(document.lines[1].romanized.as_deref(), Some("utau"));

        let lrc = export_document(&document, OutputFormat::Lrc).unwrap();
        let lrc = String::from_utf8(lrc).unwrap();
        assert!(lrc.contains("[00:01.00]日々"));
        assert!(!lrc.contains("hibi"));

        drop(connection);
        fs::remove_file(path).unwrap();
    }

    #[tokio::test]
    async fn fetches_document_json_without_losing_reading_fields() {
        let path = temp_db_path("json");
        create_db(&path);
        let connection = Connection::open(&path).unwrap();
        let document = json!({
            "meta": {},
            "lines": [{
                "start_ms": 0,
                "duration_ms": null,
                "text": "日々",
                "words": [],
                "reading": "ひび"
            }]
        })
        .to_string();
        connection
            .execute(
                "INSERT INTO lyrics VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
                (
                    "song-1",
                    "Song",
                    "Artist",
                    Option::<String>::None,
                    Option::<i64>::None,
                    "offline-db",
                    "json",
                    document,
                    Option::<String>::None,
                    Some("hibi"),
                    Option::<String>::None,
                ),
            )
            .unwrap();

        let result = SearchResult {
            source: Source::OfflineDb,
            id: "song-1".into(),
            title: "Song".into(),
            artist: "Artist".into(),
            album: None,
            duration_ms: None,
            extra: json!({ "offline_db_id": "song-1" }),
        };

        let fetched = provider(&path).fetch(&result).await.unwrap();
        assert_eq!(fetched.input_format, InputFormat::Json);
        let document = fetched.document.unwrap();
        assert_eq!(document.lines[0].reading.as_deref(), Some("ひび"));
        assert_eq!(document.lines[0].romanized.as_deref(), Some("hibi"));

        drop(connection);
        fs::remove_file(path).unwrap();
    }

    #[tokio::test]
    async fn missing_table_errors_are_readable() {
        let path = temp_db_path("empty");
        Connection::open(&path).unwrap();

        let err = provider(&path)
            .search("Song")
            .await
            .unwrap_err()
            .to_string();
        assert!(err.contains("Offline DB"));
        assert!(err.contains("no such table"));

        fs::remove_file(path).unwrap();
    }

    #[test]
    fn missing_path_requires_configuration() {
        let err = OfflineDbProvider::new(None).unwrap_err().to_string();
        assert!(err.contains("offline-db requires"));
    }
}
