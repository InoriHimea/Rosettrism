use super::*;

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
