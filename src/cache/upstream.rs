use super::*;

pub(super) struct OwnedCachePut {
    pub(super) key: String,
    pub(super) source: Source,
    pub(super) operation: String,
    pub(super) status_code: u16,
    pub(super) body: Vec<u8>,
    pub(super) metadata: serde_json::Value,
    pub(super) ttl: Duration,
}

#[derive(Debug, Default)]
pub(super) struct CacheDisplayFields {
    pub(super) query: Option<String>,
    pub(super) item_id: Option<String>,
    pub(super) title: Option<String>,
    pub(super) artist: Option<String>,
}

pub(super) fn cache_display_fields(
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

pub(super) fn merge_result_fields(fields: &mut CacheDisplayFields, result: &serde_json::Value) {
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

pub(super) fn text_preview(body: &[u8]) -> String {
    const MAX_PREVIEW: usize = 64 * 1024;
    let slice = &body[..body.len().min(MAX_PREVIEW)];
    String::from_utf8_lossy(slice).to_string()
}

impl UpstreamCache {
    pub async fn get_fresh(&self, key: &str) -> Result<Option<CacheHit>> {
        let key = key.to_string();
        self.spawn_blocking(move |cache| cache.get_fresh_sync(&key))
            .await
    }

    pub(super) fn get_fresh_sync(&self, key: &str) -> Result<Option<CacheHit>> {
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

    pub async fn put(&self, request: CachePut<'_>) -> Result<i64> {
        let request = OwnedCachePut {
            key: request.key.to_string(),
            source: request.source,
            operation: request.operation.to_string(),
            status_code: request.status_code,
            body: request.body.to_vec(),
            metadata: request.metadata.clone(),
            ttl: request.ttl,
        };
        self.spawn_blocking(move |cache| cache.put_sync(&request))
            .await
    }

    pub(super) fn put_sync(&self, request: &OwnedCachePut) -> Result<i64> {
        let now = now_unix();
        let expires_at = now.saturating_add(request.ttl.as_secs().min(i64::MAX as u64) as i64);
        let body_hash = hash_hex(&request.body);
        let metadata_json = serde_json::to_string(&request.metadata)?;
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

    pub async fn list(&self, limit: usize) -> Result<Vec<CacheEntrySummary>> {
        self.spawn_blocking(move |cache| cache.list_sync(limit))
            .await
    }

    pub(super) fn list_sync(&self, limit: usize) -> Result<Vec<CacheEntrySummary>> {
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

    pub async fn detail(&self, id: i64) -> Result<Option<CacheEntryDetail>> {
        self.spawn_blocking(move |cache| cache.detail_sync(id))
            .await
    }

    pub(super) fn detail_sync(&self, id: i64) -> Result<Option<CacheEntryDetail>> {
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

    pub async fn delete(&self, id: i64) -> Result<bool> {
        self.spawn_blocking(move |cache| cache.delete_sync(id))
            .await
    }

    pub(super) fn delete_sync(&self, id: i64) -> Result<bool> {
        let mut connection = self.lock()?;
        let transaction = connection.transaction()?;
        let cache_key = transaction
            .query_row(
                "SELECT cache_key FROM upstream_cache WHERE id = ?1",
                params![id],
                |row| row.get::<_, String>(0),
            )
            .optional()?;
        let Some(cache_key) = cache_key else {
            transaction.commit()?;
            return Ok(false);
        };
        let changed =
            transaction.execute("DELETE FROM upstream_cache WHERE id = ?1", params![id])?;
        if changed > 0 {
            delete_unified_dependents(&transaction, &cache_key)?;
        }
        transaction.commit()?;
        Ok(changed > 0)
    }
}
