use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use rusqlite::{params, params_from_iter, Connection, OptionalExtension};
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
    pub ai_score_entries: i64,
    pub fetch_run_entries: i64,
    pub fresh_upstream_entries: i64,
    pub expired_upstream_entries: i64,
    pub fresh_unified_entries: i64,
    pub expired_unified_entries: i64,
}

#[derive(Debug, Clone, Copy)]
pub struct CachePruneOptions {
    pub dry_run: bool,
    pub keep_fetch_runs: usize,
    pub keep_ai_scores: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct CachePruneReport {
    pub dry_run: bool,
    pub expired_upstream_entries: i64,
    pub expired_unified_entries: i64,
    pub old_fetch_run_entries: i64,
    pub old_ai_score_entries: i64,
    pub total_entries: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CacheExportFormat {
    Jsonl,
    PrettyJson,
}

#[derive(Debug, Clone, Copy)]
pub struct CacheExportOptions {
    pub format: CacheExportFormat,
    pub upstream: bool,
    pub unified: bool,
    pub fetch_runs: bool,
    pub ai_scores: bool,
    pub limit: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct FetchRunRecord {
    pub id: i64,
    pub query: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    pub mode: String,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    pub created_at: i64,
    pub started_at: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finished_at: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider_count: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub candidate_count: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_event: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct FetchRunMetadata {
    pub provider_count: Option<i64>,
    pub candidate_count: Option<i64>,
    pub cache_event: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProviderHealthRecord {
    pub source: String,
    pub sample_size: i64,
    pub success_count: i64,
    pub warning_count: i64,
    pub error_count: i64,
    pub success_rate: f64,
    pub warning_rate: f64,
    pub error_rate: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub average_duration_ms: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_finished_at: Option<i64>,
    pub status: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct FetchRunStatusCount {
    pub status: String,
    pub count: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct AiScoreRecord {
    pub id: i64,
    pub unified_cache_id: i64,
    pub score_json: serde_json::Value,
    pub created_at: i64,
}

#[derive(Debug, Clone, Default)]
pub struct AiScoreQuery {
    pub query_hash: Option<String>,
    pub model: Option<String>,
    pub prompt_version: Option<String>,
    pub created_at_start: Option<i64>,
    pub created_at_end: Option<i64>,
    pub limit: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct UnifiedCacheEntrySummary {
    pub id: i64,
    pub cache_key: String,
    pub body_hash: String,
    pub created_at: i64,
    pub expires_at: i64,
    pub fresh: bool,
    pub dependency_count: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct UnifiedCacheEntryDetail {
    pub id: i64,
    pub cache_key: String,
    pub body_text_preview: String,
    pub body_hash: String,
    pub dependencies: serde_json::Value,
    pub created_at: i64,
    pub expires_at: i64,
    pub fresh: bool,
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
        connection.pragma_update(None, "foreign_keys", "ON")?;
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

    pub fn put_ai_score(
        &self,
        unified_cache_id: i64,
        score_json: &serde_json::Value,
    ) -> Result<i64> {
        let now = now_unix();
        let sanitized_score = sanitize_ai_score_json(score_json)?;
        let score_json = serde_json::to_string(&sanitized_score)?;
        let query_hash = sanitized_score
            .get("candidate_summary_hash")
            .and_then(serde_json::Value::as_str);
        let model = sanitized_score
            .get("model")
            .and_then(serde_json::Value::as_str);
        let prompt_version = sanitized_score
            .get("prompt_version")
            .and_then(serde_json::Value::as_str);
        let connection = self.lock()?;
        connection.execute(
            "INSERT INTO ai_scores (unified_cache_id, score_json, query_hash, model, prompt_version, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![unified_cache_id, score_json, query_hash, model, prompt_version, now],
        )?;
        Ok(connection.last_insert_rowid())
    }

    pub fn list_ai_scores(&self, unified_cache_id: i64) -> Result<Vec<AiScoreRecord>> {
        let connection = self.lock()?;
        let mut statement = connection.prepare(
            "SELECT id, unified_cache_id, score_json, created_at
             FROM ai_scores
             WHERE unified_cache_id = ?1
             ORDER BY created_at DESC, id DESC",
        )?;
        let rows = statement.query_map(params![unified_cache_id], ai_score_from_row)?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Error::from)
    }

    pub fn list_recent_ai_scores(&self, limit: usize) -> Result<Vec<AiScoreRecord>> {
        self.query_ai_scores(AiScoreQuery {
            limit,
            ..AiScoreQuery::default()
        })
    }

    pub fn query_ai_scores(&self, query: AiScoreQuery) -> Result<Vec<AiScoreRecord>> {
        let limit = i64::try_from(query.limit).unwrap_or(20).clamp(1, 100);
        let mut sql = String::from(
            "SELECT id, unified_cache_id, score_json, created_at FROM ai_scores WHERE 1 = 1",
        );
        let mut values: Vec<rusqlite::types::Value> = Vec::new();
        if let Some(query_hash) = query.query_hash {
            sql.push_str(" AND query_hash = ?");
            values.push(query_hash.into());
        }
        if let Some(model) = query.model {
            sql.push_str(" AND model = ?");
            values.push(model.into());
        }
        if let Some(prompt_version) = query.prompt_version {
            sql.push_str(" AND prompt_version = ?");
            values.push(prompt_version.into());
        }
        if let Some(start) = query.created_at_start {
            sql.push_str(" AND created_at >= ?");
            values.push(start.into());
        }
        if let Some(end) = query.created_at_end {
            sql.push_str(" AND created_at <= ?");
            values.push(end.into());
        }
        sql.push_str(" ORDER BY created_at DESC, id DESC LIMIT ?");
        values.push(limit.into());

        let connection = self.lock()?;
        let mut statement = connection.prepare(&sql)?;
        let rows = statement.query_map(params_from_iter(values), ai_score_from_row)?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Error::from)
    }

    pub fn start_fetch_run(&self, query: &str, source: Option<Source>, mode: &str) -> Result<i64> {
        let now = now_unix();
        let source = source.map(|source| source.cli_name().to_string());
        let connection = self.lock()?;
        connection.execute(
            "INSERT INTO fetch_runs (query, source, mode, status, message, created_at, started_at)
             VALUES (?1, ?2, ?3, 'started', NULL, ?4, ?4)",
            params![query, source, mode, now],
        )?;
        Ok(connection.last_insert_rowid())
    }

    pub fn finish_fetch_run(
        &self,
        id: i64,
        status: &str,
        message: Option<&str>,
        metadata: FetchRunMetadata,
    ) -> Result<()> {
        let finished_at = now_unix();
        let cache_event = metadata.cache_event.as_deref();
        let connection = self.lock()?;
        connection.execute(
            "UPDATE fetch_runs
             SET status = ?1, message = ?2, finished_at = ?3,
                 duration_ms = max(0, (?3 - started_at) * 1000),
                 provider_count = ?4, candidate_count = ?5, cache_event = ?6
             WHERE id = ?7",
            params![
                status,
                message,
                finished_at,
                metadata.provider_count,
                metadata.candidate_count,
                cache_event,
                id
            ],
        )?;
        Ok(())
    }

    pub fn list_fetch_runs(&self, limit: usize) -> Result<Vec<FetchRunRecord>> {
        let limit = i64::try_from(limit).unwrap_or(50).clamp(1, 500);
        let connection = self.lock()?;
        let mut statement = connection.prepare(
            "SELECT id, query, source, mode, status, message, created_at, started_at, finished_at,
                    duration_ms, provider_count, candidate_count, cache_event
             FROM fetch_runs
             ORDER BY started_at DESC, id DESC
             LIMIT ?1",
        )?;
        let rows = statement.query_map(params![limit], fetch_run_from_row)?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Error::from)
    }

    pub fn provider_health(&self, limit_per_source: usize) -> Result<Vec<ProviderHealthRecord>> {
        let limit_per_source = limit_per_source.clamp(1, 500);
        let connection = self.lock()?;
        let mut statement = connection.prepare(
            "SELECT id, query, source, mode, status, message, created_at, started_at, finished_at,
                    duration_ms, provider_count, candidate_count, cache_event
             FROM fetch_runs
             WHERE source IS NOT NULL
             ORDER BY source ASC, started_at DESC, id DESC",
        )?;
        let rows = statement.query_map([], fetch_run_from_row)?;
        let mut buckets: std::collections::BTreeMap<String, Vec<FetchRunRecord>> =
            std::collections::BTreeMap::new();
        for row in rows {
            let run = row?;
            let Some(source) = run.source.clone() else {
                continue;
            };
            let bucket = buckets.entry(source).or_default();
            if bucket.len() < limit_per_source {
                bucket.push(run);
            }
        }

        let mut records = buckets
            .into_iter()
            .map(|(source, runs)| provider_health_from_runs(source, &runs))
            .collect::<Vec<_>>();
        records.sort_by(|left, right| {
            left.success_rate
                .partial_cmp(&right.success_rate)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| {
                    right
                        .error_rate
                        .partial_cmp(&left.error_rate)
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
                .then_with(|| left.source.cmp(&right.source))
        });
        Ok(records)
    }

    pub fn fetch_run_status_counts(&self) -> Result<Vec<FetchRunStatusCount>> {
        let connection = self.lock()?;
        let mut statement = connection.prepare(
            "SELECT status, count(*)
             FROM fetch_runs
             GROUP BY status
             ORDER BY count(*) DESC, status ASC",
        )?;
        let rows = statement.query_map([], |row| {
            Ok(FetchRunStatusCount {
                status: row.get(0)?,
                count: row.get(1)?,
            })
        })?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Error::from)
    }

    pub fn unified_body(&self, id: i64) -> Result<Option<Vec<u8>>> {
        let connection = self.lock()?;
        connection
            .query_row(
                "SELECT body FROM unified_cache WHERE id = ?1",
                params![id],
                |row| row.get(0),
            )
            .optional()
            .map_err(Error::from)
    }

    pub fn unified_detail(&self, id: i64) -> Result<Option<UnifiedCacheEntryDetail>> {
        let now = now_unix();
        let connection = self.lock()?;
        let detail = connection
            .query_row(
                "SELECT id, cache_key, body, body_hash, dependencies_json, created_at, expires_at
                 FROM unified_cache
                 WHERE id = ?1",
                params![id],
                |row| {
                    let body: Vec<u8> = row.get(2)?;
                    let dependencies_json: String = row.get(4)?;
                    let expires_at: i64 = row.get(6)?;
                    Ok(UnifiedCacheEntryDetail {
                        id: row.get(0)?,
                        cache_key: row.get(1)?,
                        body_text_preview: text_preview(&body),
                        body_hash: row.get(3)?,
                        dependencies: serde_json::from_str(&dependencies_json)
                            .unwrap_or_else(|_| json!([])),
                        created_at: row.get(5)?,
                        expires_at,
                        fresh: expires_at > now,
                    })
                },
            )
            .optional()?;
        Ok(detail)
    }

    pub fn list_unified(&self, limit: usize) -> Result<Vec<UnifiedCacheEntrySummary>> {
        let now = now_unix();
        let limit = i64::try_from(limit).unwrap_or(100).clamp(1, 500);
        let connection = self.lock()?;
        let mut statement = connection.prepare(
            "SELECT id, cache_key, body_hash, dependencies_json, created_at, expires_at
             FROM unified_cache
             ORDER BY created_at DESC
             LIMIT ?1",
        )?;
        let rows = statement.query_map(params![limit], |row| {
            let dependencies_json: String = row.get(3)?;
            let dependencies = serde_json::from_str::<Vec<String>>(&dependencies_json)
                .map(|dependencies| dependencies.len())
                .unwrap_or(0);
            let expires_at: i64 = row.get(5)?;
            Ok(UnifiedCacheEntrySummary {
                id: row.get(0)?,
                cache_key: row.get(1)?,
                body_hash: row.get(2)?,
                created_at: row.get(4)?,
                expires_at,
                fresh: expires_at > now,
                dependency_count: dependencies,
            })
        })?;

        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Error::from)
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

    pub fn delete_unified(&self, id: i64) -> Result<bool> {
        let mut connection = self.lock()?;
        let transaction = connection.transaction()?;
        transaction.execute(
            "DELETE FROM ai_scores WHERE unified_cache_id = ?1",
            params![id],
        )?;
        let changed =
            transaction.execute("DELETE FROM unified_cache WHERE id = ?1", params![id])?;
        transaction.commit()?;
        Ok(changed > 0)
    }

    pub fn stats(&self) -> Result<CacheStats> {
        let now = now_unix();
        let connection = self.lock()?;
        let upstream_entries = count(&connection, "SELECT count(*) FROM upstream_cache", &[])?;
        let unified_entries = count(&connection, "SELECT count(*) FROM unified_cache", &[])?;
        let ai_score_entries = count(&connection, "SELECT count(*) FROM ai_scores", &[])?;
        let fetch_run_entries = count(&connection, "SELECT count(*) FROM fetch_runs", &[])?;
        let fresh_upstream_entries = connection.query_row(
            "SELECT count(*) FROM upstream_cache WHERE expires_at > ?1",
            params![now],
            |row| row.get(0),
        )?;
        let fresh_unified_entries = connection.query_row(
            "SELECT count(*) FROM unified_cache WHERE expires_at > ?1",
            params![now],
            |row| row.get(0),
        )?;
        let expired_upstream_entries = upstream_entries - fresh_upstream_entries;
        let expired_unified_entries = unified_entries - fresh_unified_entries;
        Ok(CacheStats {
            upstream_entries,
            unified_entries,
            ai_score_entries,
            fetch_run_entries,
            fresh_upstream_entries,
            expired_upstream_entries,
            fresh_unified_entries,
            expired_unified_entries,
        })
    }

    pub fn prune(&self, options: CachePruneOptions) -> Result<CachePruneReport> {
        let now = now_unix();
        let keep_fetch_runs = i64::try_from(options.keep_fetch_runs).unwrap_or(i64::MAX);
        let keep_ai_scores = i64::try_from(options.keep_ai_scores).unwrap_or(i64::MAX);
        let mut connection = self.lock()?;
        let expired_upstream_entries = connection.query_row(
            "SELECT count(*) FROM upstream_cache WHERE expires_at <= ?1",
            params![now],
            |row| row.get(0),
        )?;
        let expired_unified_entries = connection.query_row(
            "SELECT count(*) FROM unified_cache WHERE expires_at <= ?1",
            params![now],
            |row| row.get(0),
        )?;
        let old_fetch_run_entries = connection.query_row(
            "SELECT count(*) FROM fetch_runs WHERE id NOT IN (
                SELECT id FROM fetch_runs ORDER BY started_at DESC, id DESC LIMIT ?1
             )",
            params![keep_fetch_runs],
            |row| row.get(0),
        )?;
        let old_ai_score_entries = connection.query_row(
            "SELECT count(*) FROM ai_scores WHERE id NOT IN (
                SELECT id FROM ai_scores ORDER BY created_at DESC, id DESC LIMIT ?1
             )",
            params![keep_ai_scores],
            |row| row.get(0),
        )?;

        let report = CachePruneReport {
            dry_run: options.dry_run,
            expired_upstream_entries,
            expired_unified_entries,
            old_fetch_run_entries,
            old_ai_score_entries,
            total_entries: expired_upstream_entries
                + expired_unified_entries
                + old_fetch_run_entries
                + old_ai_score_entries,
        };

        if !options.dry_run {
            let transaction = connection.transaction()?;
            transaction.execute(
                "DELETE FROM upstream_cache WHERE expires_at <= ?1",
                params![now],
            )?;
            transaction.execute(
                "DELETE FROM unified_cache WHERE expires_at <= ?1",
                params![now],
            )?;
            transaction.execute(
                "DELETE FROM fetch_runs WHERE id NOT IN (
                    SELECT id FROM fetch_runs ORDER BY started_at DESC, id DESC LIMIT ?1
                 )",
                params![keep_fetch_runs],
            )?;
            transaction.execute(
                "DELETE FROM ai_scores WHERE id NOT IN (
                    SELECT id FROM ai_scores ORDER BY created_at DESC, id DESC LIMIT ?1
                 )",
                params![keep_ai_scores],
            )?;
            transaction.commit()?;
        }

        Ok(report)
    }

    pub fn vacuum(&self) -> Result<()> {
        let connection = self.lock()?;
        connection.execute_batch("VACUUM")?;
        Ok(())
    }

    pub fn export(&self, options: CacheExportOptions) -> Result<Vec<u8>> {
        let mut records = Vec::new();
        if options.upstream {
            for record in self.list(options.limit)? {
                records.push(json!({ "section": "upstream", "record": record }));
            }
        }
        if options.unified {
            for record in self.list_unified(options.limit)? {
                records.push(json!({ "section": "unified", "record": record }));
            }
        }
        if options.fetch_runs {
            for record in self.list_fetch_runs(options.limit)? {
                records.push(json!({ "section": "fetch_runs", "record": record }));
            }
        }
        if options.ai_scores {
            for record in self.list_recent_ai_scores(options.limit)? {
                records.push(json!({ "section": "ai_scores", "record": record }));
            }
        }

        match options.format {
            CacheExportFormat::Jsonl => {
                let mut output = Vec::new();
                for record in records {
                    serde_json::to_writer(&mut output, &record)?;
                    output.push(b'\n');
                }
                Ok(output)
            }
            CacheExportFormat::PrettyJson => Ok(serde_json::to_vec_pretty(&records)?),
        }
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
            CREATE INDEX IF NOT EXISTS ai_scores_query_idx
                ON ai_scores(query_hash, model, prompt_version, created_at);
            "#,
        )?;
        ensure_fetch_run_columns(&connection)?;
        ensure_ai_score_columns(&connection)?;
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
    Ok(())
}

fn sanitize_ai_score_json(score_json: &serde_json::Value) -> Result<serde_json::Value> {
    const MAX_AI_SCORE_JSON_BYTES: usize = 64 * 1024;
    let mut sanitized = score_json.clone();
    redact_secret_fields(&mut sanitized);
    let size = serde_json::to_vec(&sanitized)?.len();
    if size > MAX_AI_SCORE_JSON_BYTES {
        return Err(Error::Storage(format!(
            "AI score payload exceeds {MAX_AI_SCORE_JSON_BYTES} bytes after redaction"
        )));
    }
    Ok(sanitized)
}

fn redact_secret_fields(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::Object(map) => {
            for (key, child) in map.iter_mut() {
                let normalized = key.to_ascii_lowercase().replace(['-', '_'], "");
                if matches!(
                    normalized.as_str(),
                    "apikey" | "authorization" | "accesstoken" | "bearertoken" | "token"
                ) {
                    *child = json!("[REDACTED]");
                } else {
                    redact_secret_fields(child);
                }
            }
        }
        serde_json::Value::Array(items) => {
            for item in items {
                redact_secret_fields(item);
            }
        }
        _ => {}
    }
}

fn provider_health_from_runs(source: String, runs: &[FetchRunRecord]) -> ProviderHealthRecord {
    let sample_size = runs.len() as i64;
    let success_count = runs
        .iter()
        .filter(|run| is_success_status(&run.status))
        .count() as i64;
    let warning_count = runs
        .iter()
        .filter(|run| is_warning_status(&run.status))
        .count() as i64;
    let error_count = runs
        .iter()
        .filter(|run| is_error_status(&run.status))
        .count() as i64;
    let durations = runs
        .iter()
        .filter_map(|run| run.duration_ms)
        .collect::<Vec<_>>();
    let average_duration_ms = if durations.is_empty() {
        None
    } else {
        Some(durations.iter().sum::<i64>() as f64 / durations.len() as f64)
    };
    let last_error = runs
        .iter()
        .find(|run| is_error_status(&run.status) || is_warning_status(&run.status))
        .and_then(|run| run.message.clone().or_else(|| Some(run.status.clone())));
    let last_finished_at = runs.iter().filter_map(|run| run.finished_at).max();
    let denominator = sample_size.max(1) as f64;
    let success_rate = success_count as f64 / denominator;
    let warning_rate = warning_count as f64 / denominator;
    let error_rate = error_count as f64 / denominator;
    let status = if sample_size == 0 {
        "unknown"
    } else if error_rate >= 0.5 || success_rate < 0.5 {
        "critical"
    } else if warning_rate >= 0.25 || error_rate > 0.0 || success_rate < 0.8 {
        "degraded"
    } else {
        "healthy"
    }
    .to_string();

    ProviderHealthRecord {
        source,
        sample_size,
        success_count,
        warning_count,
        error_count,
        success_rate,
        warning_rate,
        error_rate,
        average_duration_ms,
        last_error,
        last_finished_at,
        status,
    }
}

fn is_success_status(status: &str) -> bool {
    matches!(status, "success" | "cache_hit" | "cache_store")
}

fn is_warning_status(status: &str) -> bool {
    matches!(
        status,
        "provider_warning" | "ai_skipped" | "no_lyrics_found"
    )
}

fn is_error_status(status: &str) -> bool {
    status == "error"
}

fn fetch_run_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<FetchRunRecord> {
    Ok(FetchRunRecord {
        id: row.get(0)?,
        query: row.get(1)?,
        source: row.get(2)?,
        mode: row.get(3)?,
        status: row.get(4)?,
        message: row.get(5)?,
        created_at: row.get(6)?,
        started_at: row.get(7)?,
        finished_at: row.get(8)?,
        duration_ms: row.get(9)?,
        provider_count: row.get(10)?,
        candidate_count: row.get(11)?,
        cache_event: row.get(12)?,
    })
}

fn ai_score_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<AiScoreRecord> {
    let score_json: String = row.get(2)?;
    Ok(AiScoreRecord {
        id: row.get(0)?,
        unified_cache_id: row.get(1)?,
        score_json: serde_json::from_str(&score_json).unwrap_or_else(|_| json!({})),
        created_at: row.get(3)?,
    })
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

    #[test]
    fn records_and_counts_fetch_runs() {
        let path = std::env::temp_dir().join(format!(
            "rosettrism-fetch-runs-{}-{}.sqlite",
            now_unix(),
            std::process::id()
        ));
        let cache = UpstreamCache::open(&path).unwrap();

        let first_id = cache
            .start_fetch_run("Song Artist", Some(Source::Lrclib), "fetch_source_result")
            .unwrap();
        cache
            .finish_fetch_run(
                first_id,
                "cache_hit",
                Some("served_unified_cache: id=42"),
                FetchRunMetadata {
                    provider_count: Some(1),
                    candidate_count: Some(1),
                    cache_event: Some("cache_hit".into()),
                },
            )
            .unwrap();
        let second_id = cache
            .start_fetch_run("Other Song", None, "aggregate_fetch")
            .unwrap();
        cache
            .finish_fetch_run(
                second_id,
                "no_lyrics_found",
                Some("no_lyrics_found: all selected sources returned no usable lyric"),
                FetchRunMetadata {
                    provider_count: Some(2),
                    candidate_count: Some(0),
                    cache_event: None,
                },
            )
            .unwrap();

        let runs = cache.list_fetch_runs(10).unwrap();
        assert_eq!(runs.len(), 2);
        assert_eq!(runs[0].id, second_id);
        assert_eq!(runs[0].status, "no_lyrics_found");
        assert_eq!(runs[1].source.as_deref(), Some("lrclib"));
        assert_eq!(
            runs[1].message.as_deref(),
            Some("served_unified_cache: id=42")
        );

        let counts = cache.fetch_run_status_counts().unwrap();
        assert!(counts
            .iter()
            .any(|count| count.status == "cache_hit" && count.count == 1));
        assert!(counts
            .iter()
            .any(|count| count.status == "no_lyrics_found" && count.count == 1));
        assert_eq!(cache.stats().unwrap().fetch_run_entries, 2);

        drop(cache);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn stores_and_lists_ai_scores_for_unified_cache() {
        let path = std::env::temp_dir().join(format!(
            "rosettrism-ai-score-{}-{}.sqlite",
            now_unix(),
            std::process::id()
        ));
        let cache = UpstreamCache::open(&path).unwrap();
        let unified_id = cache
            .put_unified(
                "unified-ai-score-test",
                br#"{"mode":"tracks","results":[]}"#,
                &[],
                Duration::from_secs(60),
            )
            .unwrap();
        let score = json!({
            "model": "gpt-4o-mini",
            "base_url": "https://api.openai.com/v1",
            "prompt_version": "ai-score-prompt-v1",
            "candidate_summary_hash": "abc123",
            "request_payload": { "api_key": "secret-key", "messages": ["safe"] },
            "best_index": 0,
            "scores": [{"index": 0, "source": "qq", "heuristic_score": 90.0, "ai_score": 95.0, "reason": "best timing"}],
            "reason": "best timing",
            "created_at": now_unix()
        });

        cache.put_ai_score(unified_id, &score).unwrap();

        let scores = cache.list_ai_scores(unified_id).unwrap();
        assert_eq!(scores.len(), 1);
        assert_eq!(scores[0].unified_cache_id, unified_id);
        assert_eq!(scores[0].score_json["candidate_summary_hash"], "abc123");
        assert_eq!(
            scores[0].score_json["request_payload"]["api_key"],
            "[REDACTED]"
        );
        assert!(!serde_json::to_string(&scores[0].score_json)
            .unwrap()
            .contains("secret-key"));

        let queried = cache
            .query_ai_scores(AiScoreQuery {
                query_hash: Some("abc123".into()),
                model: Some("gpt-4o-mini".into()),
                prompt_version: Some("ai-score-prompt-v1".into()),
                created_at_start: Some(0),
                created_at_end: Some(now_unix() + 60),
                limit: 10,
            })
            .unwrap();
        assert_eq!(queried.len(), 1);
        assert_eq!(cache.stats().unwrap().ai_score_entries, 1);

        drop(cache);
        let _ = std::fs::remove_file(path);
    }
}
