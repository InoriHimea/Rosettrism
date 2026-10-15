use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use rusqlite::{params, params_from_iter, Connection, OptionalExtension, Transaction};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};

use crate::provider::{SearchResult, Source};
use crate::{Error, Result};

mod ai_scores;
mod fetch_runs;
mod maintenance;
#[cfg(test)]
mod tests;
mod types;
mod unified;
mod upstream;

pub use types::*;
use unified::*;
use upstream::*;

const REQUEST_VERSION: &str = "provider-op-v1";

const UNIFIED_VERSION: &str = "unified-v1";

#[derive(Clone)]
pub struct UpstreamCache {
    inner: Arc<Mutex<Connection>>,
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

fn ensure_fetch_run_columns(connection: &Connection) -> Result<()> {
    let mut statement = connection.prepare("PRAGMA table_info(fetch_runs)")?;
    let rows = statement.query_map([], |row| row.get::<_, String>(1))?;
    let columns = rows.collect::<std::result::Result<std::collections::HashSet<_>, _>>()?;
    let migrations = [
        (
            "started_at",
            "ALTER TABLE fetch_runs ADD COLUMN started_at INTEGER",
        ),
        (
            "finished_at",
            "ALTER TABLE fetch_runs ADD COLUMN finished_at INTEGER",
        ),
        (
            "duration_ms",
            "ALTER TABLE fetch_runs ADD COLUMN duration_ms INTEGER",
        ),
        (
            "provider_count",
            "ALTER TABLE fetch_runs ADD COLUMN provider_count INTEGER",
        ),
        (
            "candidate_count",
            "ALTER TABLE fetch_runs ADD COLUMN candidate_count INTEGER",
        ),
        (
            "cache_event",
            "ALTER TABLE fetch_runs ADD COLUMN cache_event TEXT",
        ),
    ];
    for (column, sql) in migrations {
        if !columns.contains(column) {
            connection.execute(sql, [])?;
        }
    }
    connection.execute(
        "UPDATE fetch_runs SET started_at = created_at WHERE started_at IS NULL",
        [],
    )?;
    Ok(())
}

fn ensure_ai_score_columns(connection: &Connection) -> Result<()> {
    let mut statement = connection.prepare("PRAGMA table_info(ai_scores)")?;
    let rows = statement.query_map([], |row| row.get::<_, String>(1))?;
    let columns = rows.collect::<std::result::Result<std::collections::HashSet<_>, _>>()?;
    let migrations = [
        (
            "query_hash",
            "ALTER TABLE ai_scores ADD COLUMN query_hash TEXT",
        ),
        ("model", "ALTER TABLE ai_scores ADD COLUMN model TEXT"),
        (
            "prompt_version",
            "ALTER TABLE ai_scores ADD COLUMN prompt_version TEXT",
        ),
    ];
    for (column, sql) in migrations {
        if !columns.contains(column) {
            connection.execute(sql, [])?;
        }
    }
    connection.execute(
        "UPDATE ai_scores
         SET query_hash = json_extract(score_json, '$.candidate_summary_hash'),
             model = json_extract(score_json, '$.model'),
             prompt_version = json_extract(score_json, '$.prompt_version')
         WHERE query_hash IS NULL OR model IS NULL OR prompt_version IS NULL",
        [],
    )?;
    connection.execute(
        "CREATE INDEX IF NOT EXISTS ai_scores_query_idx
         ON ai_scores(query_hash, model, prompt_version, created_at)",
        [],
    )?;
    Ok(())
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

impl UpstreamCache {
    pub async fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        tokio::task::spawn_blocking(move || Self::open_sync(path))
            .await
            .map_err(Error::from)?
    }

    pub async fn open_default() -> Result<Self> {
        Self::open(default_cache_path()).await
    }

    fn open_sync(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)?;
            }
        }

        let connection = Connection::open(path)?;
        connection.pragma_update(None, "foreign_keys", "ON")?;
        connection.query_row("PRAGMA journal_mode = WAL", [], |_| Ok(()))?;
        connection.busy_timeout(Duration::from_millis(5_000))?;
        let cache = Self {
            inner: Arc::new(Mutex::new(connection)),
        };
        cache.migrate()?;
        Ok(cache)
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
                created_at INTEGER NOT NULL,
                started_at INTEGER NOT NULL,
                finished_at INTEGER,
                duration_ms INTEGER,
                provider_count INTEGER,
                candidate_count INTEGER,
                cache_event TEXT
            );

            CREATE TABLE IF NOT EXISTS ai_scores (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                unified_cache_id INTEGER NOT NULL,
                score_json TEXT NOT NULL,
                query_hash TEXT,
                model TEXT,
                prompt_version TEXT,
                created_at INTEGER NOT NULL,
                FOREIGN KEY(unified_cache_id) REFERENCES unified_cache(id) ON DELETE CASCADE
            );

            CREATE INDEX IF NOT EXISTS ai_scores_unified_cache_id_idx
                ON ai_scores(unified_cache_id);
            "#,
        )?;
        ensure_fetch_run_columns(&connection)?;
        ensure_ai_score_columns(&connection)?;
        Ok(())
    }

    async fn spawn_blocking<T, F>(&self, op: F) -> Result<T>
    where
        F: FnOnce(&Self) -> Result<T> + Send + 'static,
        T: Send + 'static,
    {
        let cache = self.clone();
        tokio::task::spawn_blocking(move || op(&cache))
            .await
            .map_err(Error::from)?
    }

    fn lock(&self) -> Result<std::sync::MutexGuard<'_, Connection>> {
        self.inner
            .lock()
            .map_err(|err| Error::Storage(format!("cache mutex poisoned: {err}")))
    }
}
