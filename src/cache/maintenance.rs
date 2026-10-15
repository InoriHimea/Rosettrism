use super::*;

impl UpstreamCache {
    pub async fn stats(&self) -> Result<CacheStats> {
        self.spawn_blocking(move |cache| cache.stats_sync()).await
    }

    pub(super) fn stats_sync(&self) -> Result<CacheStats> {
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

    pub async fn prune(&self, options: CachePruneOptions) -> Result<CachePruneReport> {
        self.spawn_blocking(move |cache| cache.prune_sync(options))
            .await
    }

    pub(super) fn prune_sync(&self, options: CachePruneOptions) -> Result<CachePruneReport> {
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

    pub async fn vacuum(&self) -> Result<()> {
        self.spawn_blocking(move |cache| cache.vacuum_sync()).await
    }

    pub(super) fn vacuum_sync(&self) -> Result<()> {
        let connection = self.lock()?;
        connection.execute_batch("VACUUM")?;
        Ok(())
    }

    pub async fn export(&self, options: CacheExportOptions) -> Result<Vec<u8>> {
        self.spawn_blocking(move |cache| cache.export_sync(options))
            .await
    }

    pub(super) fn export_sync(&self, options: CacheExportOptions) -> Result<Vec<u8>> {
        let mut records = Vec::new();
        if options.upstream {
            for record in self.list_sync(options.limit)? {
                records.push(json!({ "section": "upstream", "record": record }));
            }
        }
        if options.unified {
            for record in self.list_unified_sync(options.limit)? {
                records.push(json!({ "section": "unified", "record": record }));
            }
        }
        if options.fetch_runs {
            for record in self.list_fetch_runs_sync(options.limit)? {
                records.push(json!({ "section": "fetch_runs", "record": record }));
            }
        }
        if options.ai_scores {
            let scores = self.query_ai_scores_sync(AiScoreQuery {
                limit: options.limit,
                ..AiScoreQuery::default()
            })?;
            for record in scores {
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
}
