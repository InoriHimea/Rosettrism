use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};

use crate::provider::{SearchResult, Source};
use crate::{Error, Result};

const REQUEST_VERSION: &str = "provider-op-v1";
const UNIFIED_VERSION: &str = "unified-v1";

#[derive(Clone)]
pub struct UpstreamCache {
    inner: Arc<Mutex<Connection>>,
}

#[derive(Debug, Clone)]
pub struct CacheHit {
    pub id: i64,
    pub key: String,
    pub source: String,
    pub operation: String,
    pub status_code: u16,
    pub body: Vec<u8>,
    pub metadata: serde_json::Value,
    pub created_at: i64,
    pub expires_at: i64,
}

#[derive(Debug, Clone)]
pub struct UnifiedCacheHit {
    pub id: i64,
    pub key: String,
    pub body: Vec<u8>,
    pub created_at: i64,
    pub expires_at: i64,
}

pub struct CachePut<'a> {
    pub key: &'a str,
    pub source: Source,
    pub operation: &'a str,
    pub status_code: u16,
    pub body: &'a [u8],
    pub metadata: &'a serde_json::Value,
    pub ttl: Duration,
}

#[derive(Debug, Clone, Serialize)]
pub struct CacheEntrySummary {
    pub id: i64,
    pub cache_key: String,
    pub source: String,
    pub operation: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub query: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub item_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub artist: Option<String>,
    pub status_code: u16,
    pub body_len: usize,
    pub body_hash: String,
    pub created_at: i64,
    pub expires_at: i64,
    pub fresh: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct CacheEntryDetail {
    pub id: i64,
    pub cache_key: String,
    pub source: String,
    pub operation: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub query: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub item_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub artist: Option<String>,
    pub status_code: u16,
    pub response_headers: serde_json::Value,
    pub metadata: serde_json::Value,
    pub body_base64: String,
    pub body_text_preview: String,
    pub body_hash: String,
    pub created_at: i64,
    pub expires_at: i64,
    pub fresh: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct CacheStats {
    pub upstream_entries: i64,
    pub unified_entries: i64,
    pub fresh_upstream_entries: i64,
    pub expired_upstream_entries: i64,
}

impl UpstreamCache {
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)?;
            }
        }

        let connection = Connection::open(path)?;
        let cache = Self {
            inner: Arc::new(Mutex::new(connection)),
        };
        cache.migrate()?;
        Ok(cache)
    }

    pub fn open_default() -> Result<Self> {
        Self::open(default_cache_path())
    }

    pub fn default_path() -> PathBuf {
        default_cache_path()
    }

    pub fn cache_key<T: Serialize>(source: Source, operation: &str, request: &T) -> Result<String> {
        let normalized = serde_json::to_vec(request)?;
        let mut hasher = Sha256::new();
        hasher.update(REQUEST_VERSION.as_bytes());
        hasher.update([0]);
        hasher.update(source.cli_name().as_bytes());
        hasher.update([0]);
        hasher.update(operation.as_bytes());
        hasher.update([0]);
        hasher.update(normalized);
        Ok(format!("{:x}", hasher.finalize()))
    }

    pub fn unified_key<T: Serialize>(request: &T) -> Result<String> {
        let normalized = serde_json::to_vec(request)?;
        let mut hasher = Sha256::new();
        hasher.update(UNIFIED_VERSION.as_bytes());
        hasher.update([0]);
        hasher.update(normalized);
        Ok(format!("{:x}", hasher.finalize()))
    }

    pub fn get_fresh(&self, key: &str) -> Result<Option<CacheHit>> {
        let now = now_unix();
        let connection = self.lock()?;
        let hit = connection
            .query_row(
                "SELECT id, cache_key, source, operation, status_code, body, metadata_json, created_at, expires_at
                 FROM upstream_cache
                 WHERE cache_key = ?1 AND expires_at > ?2",
                params![key, now],
                |row| {
                    let metadata_json: String = row.get(6)?;
                    Ok(CacheHit {
                        id: row.get(0)?,
                        key: row.get(1)?,
                        source: row.get(2)?,
                        operation: row.get(3)?,
                        status_code: row.get::<_, i64>(4)? as u16,
                        body: row.get(5)?,
                        metadata: serde_json::from_str(&metadata_json).unwrap_or_else(|_| json!({})),
                        created_at: row.get(7)?,
                        expires_at: row.get(8)?,
                    })
                },
            )
            .optional()?;
        Ok(hit)
    }

    pub fn put(&self, request: CachePut<'_>) -> Result<i64> {
        let now = now_unix();
        let expires_at = now.saturating_add(request.ttl.as_secs().min(i64::MAX as u64) as i64);
        let body_hash = hash_hex(request.body);
        let metadata_json = serde_json::to_string(request.metadata)?;
        let response_headers = json!({ "cached_at_layer": "provider_operation" }).to_string();
        let connection = self.lock()?;
        connection.execute(
            "INSERT INTO upstream_cache
             (cache_key, source, operation, status_code, response_headers_json, metadata_json, body, body_hash, created_at, expires_at, request_version)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
             ON CONFLICT(cache_key) DO UPDATE SET
                source = excluded.source,
                operation = excluded.operation,
                status_code = excluded.status_code,
                response_headers_json = excluded.response_headers_json,
                metadata_json = excluded.metadata_json,
                body = excluded.body,
                body_hash = excluded.body_hash,
                created_at = excluded.created_at,
                expires_at = excluded.expires_at,
                request_version = excluded.request_version",
            params![
                request.key,
                request.source.cli_name(),
                request.operation,
                i64::from(request.status_code),
                response_headers,
                metadata_json,
                request.body,
                body_hash,
                now,
                expires_at,
                REQUEST_VERSION,
            ],
        )?;

        let id = connection.query_row(
            "SELECT id FROM upstream_cache WHERE cache_key = ?1",
            params![request.key],
            |row| row.get(0),
        )?;
        Ok(id)
    }

    pub fn get_unified_fresh(&self, key: &str) -> Result<Option<UnifiedCacheHit>> {
        let now = now_unix();
        let connection = self.lock()?;
        let hit = connection
            .query_row(
                "SELECT id, cache_key, body, created_at, expires_at
                 FROM unified_cache
                 WHERE cache_key = ?1 AND expires_at > ?2",
                params![key, now],
                |row| {
                    Ok(UnifiedCacheHit {
                        id: row.get(0)?,
                        key: row.get(1)?,
                        body: row.get(2)?,
                        created_at: row.get(3)?,
                        expires_at: row.get(4)?,
                    })
                },
            )
            .optional()?;
        Ok(hit)
    }

    pub fn put_unified(
        &self,
        key: &str,
        body: &[u8],
        dependency_keys: &[String],
        ttl: Duration,
    ) -> Result<i64> {
        let now = now_unix();
        let expires_at = now.saturating_add(ttl.as_secs().min(i64::MAX as u64) as i64);
        let body_hash = hash_hex(body);
        let dependencies_json = serde_json::to_string(dependency_keys)?;
        let connection = self.lock()?;
        connection.execute(
            "INSERT INTO unified_cache
             (cache_key, body, body_hash, dependencies_json, created_at, expires_at, request_version)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT(cache_key) DO UPDATE SET
                body = excluded.body,
                body_hash = excluded.body_hash,
                dependencies_json = excluded.dependencies_json,
                created_at = excluded.created_at,
                expires_at = excluded.expires_at,
                request_version = excluded.request_version",
            params![
                key,
                body,
                body_hash,
                dependencies_json,
                now,
                expires_at,
                UNIFIED_VERSION,
            ],
        )?;

        let id = connection.query_row(
            "SELECT id FROM unified_cache WHERE cache_key = ?1",
            params![key],
            |row| row.get(0),
        )?;
        Ok(id)
    }

    pub fn list(&self, limit: usize) -> Result<Vec<CacheEntrySummary>> {
        let now = now_unix();
        let limit = i64::try_from(limit).unwrap_or(100).clamp(1, 500);
        let connection = self.lock()?;
        let mut statement = connection.prepare(
            "SELECT id, cache_key, source, operation, status_code, length(body), body_hash, created_at, expires_at, metadata_json, body
             FROM upstream_cache
             ORDER BY created_at DESC
             LIMIT ?1",
        )?;
        let rows = statement.query_map(params![limit], |row| {
            let expires_at: i64 = row.get(8)?;
            let operation: String = row.get(3)?;
            let metadata_json: String = row.get(9)?;
            let metadata = serde_json::from_str(&metadata_json).unwrap_or_else(|_| json!({}));
            let body: Vec<u8> = row.get(10)?;
            let display = cache_display_fields(&operation, &metadata, &body);
            Ok(CacheEntrySummary {
                id: row.get(0)?,
                cache_key: row.get(1)?,
                source: row.get(2)?,
                operation,
                query: display.query,
                item_id: display.item_id,
                title: display.title,
                artist: display.artist,
                status_code: row.get::<_, i64>(4)? as u16,
                body_len: row.get::<_, i64>(5)? as usize,
                body_hash: row.get(6)?,
                created_at: row.get(7)?,
                expires_at,
                fresh: expires_at > now,
            })
        })?;

        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Error::from)
    }

    pub fn detail(&self, id: i64) -> Result<Option<CacheEntryDetail>> {
        use base64::Engine;

        let now = now_unix();
        let connection = self.lock()?;
        let detail = connection
            .query_row(
                "SELECT id, cache_key, source, operation, status_code, response_headers_json, metadata_json, body, body_hash, created_at, expires_at
                 FROM upstream_cache
                 WHERE id = ?1",
                params![id],
                |row| {
                    let headers_json: String = row.get(5)?;
                    let metadata_json: String = row.get(6)?;
                    let body: Vec<u8> = row.get(7)?;
                    let operation: String = row.get(3)?;
                    let metadata = serde_json::from_str(&metadata_json).unwrap_or_else(|_| json!({}));
                    let display = cache_display_fields(&operation, &metadata, &body);
                    let expires_at: i64 = row.get(10)?;
                    Ok(CacheEntryDetail {
                        id: row.get(0)?,
                        cache_key: row.get(1)?,
                        source: row.get(2)?,
                        operation,
                        query: display.query,
                        item_id: display.item_id,
                        title: display.title,
                        artist: display.artist,
                        status_code: row.get::<_, i64>(4)? as u16,
                        response_headers: serde_json::from_str(&headers_json)
                            .unwrap_or_else(|_| json!({})),
                        metadata,
                        body_base64: base64::engine::general_purpose::STANDARD.encode(&body),
                        body_text_preview: text_preview(&body),
                        body_hash: row.get(8)?,
                        created_at: row.get(9)?,
                        expires_at,
                        fresh: expires_at > now,
                    })
                },
            )
            .optional()?;
        Ok(detail)
    }

    pub fn delete(&self, id: i64) -> Result<bool> {
        let connection = self.lock()?;
        let changed =
            connection.execute("DELETE FROM upstream_cache WHERE id = ?1", params![id])?;
        Ok(changed > 0)
    }

    pub fn stats(&self) -> Result<CacheStats> {
        let now = now_unix();
        let connection = self.lock()?;
        let upstream_entries = count(&connection, "SELECT count(*) FROM upstream_cache", &[])?;
        let unified_entries = count(&connection, "SELECT count(*) FROM unified_cache", &[])?;
        let fresh_upstream_entries = connection.query_row(
            "SELECT count(*) FROM upstream_cache WHERE expires_at > ?1",
            params![now],
            |row| row.get(0),
        )?;
        let expired_upstream_entries = upstream_entries - fresh_upstream_entries;
        Ok(CacheStats {
            upstream_entries,
            unified_entries,
            fresh_upstream_entries,
            expired_upstream_entries,
        })
    }

    fn migrate(&self) -> Result<()> {
        let connection = self.lock()?;
        connection.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS schema_migrations (
                version INTEGER PRIMARY KEY,
                applied_at INTEGER NOT NULL
            );

            CREATE TABLE IF NOT EXISTS upstream_cache (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                cache_key TEXT NOT NULL UNIQUE,
                source TEXT NOT NULL,
                operation TEXT NOT NULL,
                status_code INTEGER NOT NULL,
                response_headers_json TEXT NOT NULL,
                metadata_json TEXT NOT NULL DEFAULT '{}',
                body BLOB NOT NULL,
                body_hash TEXT NOT NULL,
                created_at INTEGER NOT NULL,
                expires_at INTEGER NOT NULL,
                request_version TEXT NOT NULL
            );

            CREATE INDEX IF NOT EXISTS upstream_cache_source_operation_idx
                ON upstream_cache(source, operation);
            CREATE INDEX IF NOT EXISTS upstream_cache_expires_at_idx
                ON upstream_cache(expires_at);

            CREATE TABLE IF NOT EXISTS unified_cache (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                cache_key TEXT NOT NULL UNIQUE,
                body BLOB NOT NULL,
                body_hash TEXT NOT NULL,
                dependencies_json TEXT NOT NULL,
                created_at INTEGER NOT NULL,
                expires_at INTEGER NOT NULL,
                request_version TEXT NOT NULL
            );

            CREATE INDEX IF NOT EXISTS unified_cache_expires_at_idx
                ON unified_cache(expires_at);

            CREATE TABLE IF NOT EXISTS fetch_runs (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                query TEXT NOT NULL,
                source TEXT,
                mode TEXT NOT NULL,
                status TEXT NOT NULL,
                message TEXT,
                created_at INTEGER NOT NULL
            );

            CREATE TABLE IF NOT EXISTS ai_scores (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                unified_cache_id INTEGER,
                score_json TEXT NOT NULL,
                created_at INTEGER NOT NULL
            );
            "#,
        )?;
        Ok(())
    }

    fn lock(&self) -> Result<std::sync::MutexGuard<'_, Connection>> {
        self.inner
            .lock()
            .map_err(|err| Error::Storage(format!("cache mutex poisoned: {err}")))
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CachedFetchMetadata {
    pub input_format: crate::decoder::InputFormat,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result: Option<SearchResult>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub document: Option<crate::model::LyricDocument>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub annotations: Vec<crate::model::Annotation>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CachedErrorMetadata {
    pub error: String,
}

pub fn default_ttl() -> Duration {
    Duration::from_secs(7 * 24 * 60 * 60)
}

pub fn error_ttl(ttl: Duration) -> Duration {
    ttl.min(Duration::from_secs(5 * 60))
}

pub fn now_unix() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        .min(i64::MAX as u64) as i64
}

fn hash_hex(body: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(body);
    format!("{:x}", hasher.finalize())
}

#[derive(Debug, Default)]
struct CacheDisplayFields {
    query: Option<String>,
    item_id: Option<String>,
    title: Option<String>,
    artist: Option<String>,
}

fn cache_display_fields(
    operation: &str,
    metadata: &serde_json::Value,
    body: &[u8],
) -> CacheDisplayFields {
    let mut fields = CacheDisplayFields {
        query: metadata
            .get("query")
            .and_then(serde_json::Value::as_str)
            .map(ToOwned::to_owned),
        item_id: None,
        title: None,
        artist: None,
    };

    if let Some(result) = metadata
        .get("result")
        .or_else(|| metadata.get("first_result"))
    {
        merge_result_fields(&mut fields, result);
    }

    if operation == "search" && fields.title.is_none() {
        if let Ok(results) = serde_json::from_slice::<Vec<SearchResult>>(body) {
            if let Some(result) = results.first() {
                fields.item_id = Some(result.id.clone());
                fields.title = Some(result.title.clone());
                fields.artist = Some(result.artist.clone());
            }
        }
    }

    fields
}

fn merge_result_fields(fields: &mut CacheDisplayFields, result: &serde_json::Value) {
    if fields.item_id.is_none() {
        fields.item_id = result
            .get("id")
            .and_then(serde_json::Value::as_str)
            .map(ToOwned::to_owned);
    }
    if fields.title.is_none() {
        fields.title = result
            .get("title")
            .and_then(serde_json::Value::as_str)
            .map(ToOwned::to_owned);
    }
    if fields.artist.is_none() {
        fields.artist = result
            .get("artist")
            .and_then(serde_json::Value::as_str)
            .map(ToOwned::to_owned);
    }
}

fn text_preview(body: &[u8]) -> String {
    const MAX_PREVIEW: usize = 64 * 1024;
    let slice = &body[..body.len().min(MAX_PREVIEW)];
    String::from_utf8_lossy(slice).to_string()
}

fn count(connection: &Connection, sql: &str, params: &[&dyn rusqlite::ToSql]) -> Result<i64> {
    Ok(connection.query_row(sql, params, |row| row.get(0))?)
}

fn default_cache_path() -> PathBuf {
    if let Ok(path) = std::env::var("ROSETTRISM_DB") {
        if !path.trim().is_empty() {
            return PathBuf::from(path);
        }
    }

    if let Ok(path) = std::env::var("LRC_DECODE_DB") {
        if !path.trim().is_empty() {
            return PathBuf::from(path);
        }
    }

    if cfg!(windows) {
        if let Ok(appdata) = std::env::var("APPDATA") {
            return PathBuf::from(appdata)
                .join("rosettrism")
                .join("cache.sqlite");
        }
    }

    if let Ok(home) = std::env::var("HOME") {
        return PathBuf::from(home)
            .join(".local")
            .join("share")
            .join("rosettrism")
            .join("cache.sqlite");
    }

    PathBuf::from(".rosettrism-cache.sqlite")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn list_and_detail_include_display_fields_and_preview() {
        let path = std::env::temp_dir().join(format!(
            "rosettrism-cache-display-{}-{}.sqlite",
            now_unix(),
            std::process::id()
        ));
        let cache = UpstreamCache::open(&path).unwrap();
        let result = SearchResult {
            source: Source::Lrclib,
            id: "abc-123".into(),
            title: "Song".into(),
            artist: "Artist".into(),
            album: None,
            duration_ms: Some(180_000),
            extra: json!({}),
        };
        let body = serde_json::to_vec(&vec![result.clone()]).unwrap();
        let metadata = json!({
            "payload": "search_results",
            "query": "Song Artist",
            "first_result": result,
        });

        cache
            .put(CachePut {
                key: "display-test",
                source: Source::Lrclib,
                operation: "search",
                status_code: 200,
                body: &body,
                metadata: &metadata,
                ttl: Duration::from_secs(60),
            })
            .unwrap();

        let entries = cache.list(10).unwrap();
        let entry = entries
            .iter()
            .find(|entry| entry.cache_key == "display-test")
            .unwrap();
        assert_eq!(entry.query.as_deref(), Some("Song Artist"));
        assert_eq!(entry.item_id.as_deref(), Some("abc-123"));
        assert_eq!(entry.title.as_deref(), Some("Song"));
        assert_eq!(entry.artist.as_deref(), Some("Artist"));

        let detail = cache.detail(entry.id).unwrap().unwrap();
        assert_eq!(detail.query.as_deref(), Some("Song Artist"));
        assert_eq!(detail.item_id.as_deref(), Some("abc-123"));
        assert!(detail.body_text_preview.contains("Song"));

        drop(cache);
        let _ = std::fs::remove_file(path);
    }
}
