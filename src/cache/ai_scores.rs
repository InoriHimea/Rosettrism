use super::*;

pub(super) fn sanitize_ai_score_json(score_json: &serde_json::Value) -> Result<serde_json::Value> {
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

pub(super) fn redact_secret_fields(value: &mut serde_json::Value) {
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

pub(super) fn ai_score_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<AiScoreRecord> {
    let score_json: String = row.get(2)?;
    Ok(AiScoreRecord {
        id: row.get(0)?,
        unified_cache_id: row.get(1)?,
        score_json: serde_json::from_str(&score_json).unwrap_or_else(|_| json!({})),
        created_at: row.get(3)?,
    })
}

impl UpstreamCache {
    pub async fn put_ai_score(
        &self,
        unified_cache_id: i64,
        score_json: &serde_json::Value,
    ) -> Result<i64> {
        let score_json = score_json.clone();
        self.spawn_blocking(move |cache| cache.put_ai_score_sync(unified_cache_id, &score_json))
            .await
    }

    pub(super) fn put_ai_score_sync(
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

    pub async fn list_ai_scores(&self, unified_cache_id: i64) -> Result<Vec<AiScoreRecord>> {
        self.spawn_blocking(move |cache| cache.list_ai_scores_sync(unified_cache_id))
            .await
    }

    pub(super) fn list_ai_scores_sync(&self, unified_cache_id: i64) -> Result<Vec<AiScoreRecord>> {
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

    pub async fn list_recent_ai_scores(&self, limit: usize) -> Result<Vec<AiScoreRecord>> {
        self.spawn_blocking(move |cache| {
            cache.query_ai_scores_sync(AiScoreQuery {
                limit,
                ..AiScoreQuery::default()
            })
        })
        .await
    }

    pub async fn query_ai_scores(&self, query: AiScoreQuery) -> Result<Vec<AiScoreRecord>> {
        self.spawn_blocking(move |cache| cache.query_ai_scores_sync(query))
            .await
    }

    pub(super) fn query_ai_scores_sync(&self, query: AiScoreQuery) -> Result<Vec<AiScoreRecord>> {
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
}
