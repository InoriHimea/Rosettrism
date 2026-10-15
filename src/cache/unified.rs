use super::*;

pub(super) fn delete_unified_dependents(
    transaction: &Transaction<'_>,
    dependency_key: &str,
) -> Result<usize> {
    let dependents = {
        let mut statement =
            transaction.prepare("SELECT id, dependencies_json FROM unified_cache")?;
        let rows = statement.query_map([], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
        })?;
        rows.collect::<std::result::Result<Vec<_>, _>>()?
    };

    let mut deleted = 0;
    for (id, dependencies_json) in dependents {
        let dependencies =
            serde_json::from_str::<Vec<String>>(&dependencies_json).unwrap_or_else(|_| Vec::new());
        if dependencies
            .iter()
            .any(|dependency| dependency == dependency_key)
        {
            deleted +=
                transaction.execute("DELETE FROM unified_cache WHERE id = ?1", params![id])?;
        }
    }
    Ok(deleted)
}

impl UpstreamCache {
    pub async fn get_unified_fresh(&self, key: &str) -> Result<Option<UnifiedCacheHit>> {
        let key = key.to_string();
        self.spawn_blocking(move |cache| cache.get_unified_fresh_sync(&key))
            .await
    }

    pub(super) fn get_unified_fresh_sync(&self, key: &str) -> Result<Option<UnifiedCacheHit>> {
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

    pub async fn put_unified(
        &self,
        key: &str,
        body: &[u8],
        dependency_keys: &[String],
        ttl: Duration,
    ) -> Result<i64> {
        let key = key.to_string();
        let body = body.to_vec();
        let dependency_keys = dependency_keys.to_vec();
        self.spawn_blocking(move |cache| cache.put_unified_sync(&key, &body, &dependency_keys, ttl))
            .await
    }

    pub(super) fn put_unified_sync(
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

    pub async fn unified_body(&self, id: i64) -> Result<Option<Vec<u8>>> {
        self.spawn_blocking(move |cache| cache.unified_body_sync(id))
            .await
    }

    pub(super) fn unified_body_sync(&self, id: i64) -> Result<Option<Vec<u8>>> {
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

    pub async fn unified_detail(&self, id: i64) -> Result<Option<UnifiedCacheEntryDetail>> {
        self.spawn_blocking(move |cache| cache.unified_detail_sync(id))
            .await
    }

    pub(super) fn unified_detail_sync(&self, id: i64) -> Result<Option<UnifiedCacheEntryDetail>> {
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

    pub async fn list_unified(&self, limit: usize) -> Result<Vec<UnifiedCacheEntrySummary>> {
        self.spawn_blocking(move |cache| cache.list_unified_sync(limit))
            .await
    }

    pub(super) fn list_unified_sync(&self, limit: usize) -> Result<Vec<UnifiedCacheEntrySummary>> {
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

    pub async fn delete_unified(&self, id: i64) -> Result<bool> {
        self.spawn_blocking(move |cache| cache.delete_unified_sync(id))
            .await
    }

    pub(super) fn delete_unified_sync(&self, id: i64) -> Result<bool> {
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
}
