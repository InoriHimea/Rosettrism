use super::*;

pub(super) fn provider_health_from_runs(
    source: String,
    runs: &[FetchRunRecord],
) -> ProviderHealthRecord {
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

pub(super) fn is_success_status(status: &str) -> bool {
    matches!(status, "success" | "cache_hit" | "cache_store")
}

pub(super) fn is_warning_status(status: &str) -> bool {
    matches!(
        status,
        "provider_warning" | "ai_skipped" | "no_lyrics_found"
    )
}

pub(super) fn is_error_status(status: &str) -> bool {
    status == "error"
}

pub(super) fn fetch_run_from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<FetchRunRecord> {
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

impl UpstreamCache {
    pub async fn start_fetch_run(
        &self,
        query: &str,
        source: Option<Source>,
        mode: &str,
    ) -> Result<i64> {
        let query = query.to_string();
        let mode = mode.to_string();
        self.spawn_blocking(move |cache| cache.start_fetch_run_sync(&query, source, &mode))
            .await
    }

    pub(super) fn start_fetch_run_sync(
        &self,
        query: &str,
        source: Option<Source>,
        mode: &str,
    ) -> Result<i64> {
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

    pub async fn finish_fetch_run(
        &self,
        id: i64,
        status: &str,
        message: Option<&str>,
        metadata: FetchRunMetadata,
    ) -> Result<()> {
        let status = status.to_string();
        let message = message.map(str::to_string);
        self.spawn_blocking(move |cache| {
            cache.finish_fetch_run_sync(id, &status, message.as_deref(), metadata)
        })
        .await
    }

    pub(super) fn finish_fetch_run_sync(
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

    pub async fn list_fetch_runs(&self, limit: usize) -> Result<Vec<FetchRunRecord>> {
        self.spawn_blocking(move |cache| cache.list_fetch_runs_sync(limit))
            .await
    }

    pub(super) fn list_fetch_runs_sync(&self, limit: usize) -> Result<Vec<FetchRunRecord>> {
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

    pub async fn provider_health(
        &self,
        limit_per_source: usize,
    ) -> Result<Vec<ProviderHealthRecord>> {
        self.spawn_blocking(move |cache| cache.provider_health_sync(limit_per_source))
            .await
    }

    pub(super) fn provider_health_sync(
        &self,
        limit_per_source: usize,
    ) -> Result<Vec<ProviderHealthRecord>> {
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

    pub async fn fetch_run_status_counts(&self) -> Result<Vec<FetchRunStatusCount>> {
        self.spawn_blocking(move |cache| cache.fetch_run_status_counts_sync())
            .await
    }

    pub(super) fn fetch_run_status_counts_sync(&self) -> Result<Vec<FetchRunStatusCount>> {
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
}
