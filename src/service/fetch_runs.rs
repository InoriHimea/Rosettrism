use super::*;
use crate::cache::FetchRunMetadata;
use crate::Result;

pub(crate) trait FetchRunOutcome {
    fn fetch_run_warnings(&self) -> &[String];

    fn fetch_run_metadata(&self) -> FetchRunMetadata {
        FetchRunMetadata::default()
    }
}

impl FetchRunOutcome for AggregateFetchResponse {
    fn fetch_run_warnings(&self) -> &[String] {
        &self.warnings
    }

    fn fetch_run_metadata(&self) -> FetchRunMetadata {
        metadata_for_run(&self.metadata)
    }
}

impl FetchRunOutcome for MultiSourceSearchResponse {
    fn fetch_run_warnings(&self) -> &[String] {
        &self.warnings
    }

    fn fetch_run_metadata(&self) -> FetchRunMetadata {
        metadata_for_run(&self.metadata)
    }
}

impl FetchRunOutcome for UnifiedLyric {
    fn fetch_run_warnings(&self) -> &[String] {
        &self.warnings
    }

    fn fetch_run_metadata(&self) -> FetchRunMetadata {
        FetchRunMetadata {
            provider_count: Some(self.source_refs.len() as i64),
            candidate_count: Some(self.cache_refs.len() as i64),
            cache_event: None,
        }
    }
}

impl FetchRunOutcome for SpecificFetchResult {
    fn fetch_run_warnings(&self) -> &[String] {
        match self {
            SpecificFetchResult::Raw { .. } | SpecificFetchResult::Json { .. } => &[],
            SpecificFetchResult::RawMany { warnings, .. }
            | SpecificFetchResult::JsonMany { warnings, .. } => warnings,
        }
    }

    fn fetch_run_metadata(&self) -> FetchRunMetadata {
        match self {
            SpecificFetchResult::Raw { .. } | SpecificFetchResult::Json { .. } => {
                FetchRunMetadata {
                    provider_count: Some(1),
                    candidate_count: Some(1),
                    cache_event: None,
                }
            }
            SpecificFetchResult::RawMany { results, .. } => FetchRunMetadata {
                provider_count: Some(1),
                candidate_count: Some(results.len() as i64),
                cache_event: None,
            },
            SpecificFetchResult::JsonMany { results, .. } => FetchRunMetadata {
                provider_count: Some(1),
                candidate_count: Some(results.len() as i64),
                cache_event: None,
            },
        }
    }
}

pub(crate) fn fetch_run_metadata_from_result<T: FetchRunOutcome>(
    result: &Result<T>,
) -> FetchRunMetadata {
    result
        .as_ref()
        .map(FetchRunOutcome::fetch_run_metadata)
        .unwrap_or_default()
}

pub(crate) fn provider_warning_count(warnings: &[String]) -> usize {
    warnings
        .iter()
        .filter(|warning| {
            warning.starts_with("provider_warning:")
                || warning.contains("provider task failed")
                || warning.contains("enrichment skipped")
        })
        .count()
}

fn metadata_for_run(metadata: &FetchRunStructuredMetadata) -> FetchRunMetadata {
    FetchRunMetadata {
        provider_count: metadata.provider_count.map(|value| value as i64),
        candidate_count: metadata.candidate_count.map(|value| value as i64),
        cache_event: metadata.cache_event.clone(),
    }
}

pub(crate) fn is_default_fetch_metadata(metadata: &FetchRunStructuredMetadata) -> bool {
    metadata.provider_count.is_none()
        && metadata.candidate_count.is_none()
        && metadata.provider_warning_count.is_none()
        && metadata.cache_event.is_none()
}

pub(crate) fn classify_fetch_run_result<T: FetchRunOutcome>(
    result: &Result<T>,
) -> (&'static str, Option<String>) {
    match result {
        Ok(value) => classify_fetch_run_warnings(value.fetch_run_warnings()),
        Err(err) => {
            let message = err.to_string();
            let status = if message.contains("no_lyrics_found")
                || message.contains("no lyric candidates")
                || message.contains("no aggregate members")
            {
                FETCH_STATUS_NO_LYRICS
            } else if message.contains("ai_skipped")
                || message.contains("AI lyric selection skipped")
            {
                FETCH_STATUS_AI_SKIPPED
            } else if message.contains("provider_warning")
                || message.contains("provider task failed")
            {
                FETCH_STATUS_PROVIDER_WARNING
            } else {
                FETCH_STATUS_ERROR
            };
            (status, Some(message))
        }
    }
}

fn classify_fetch_run_warnings(warnings: &[String]) -> (&'static str, Option<String>) {
    if let Some(message) = warnings
        .iter()
        .find(|warning| warning.starts_with("served_unified_cache:"))
    {
        return (FETCH_STATUS_CACHE_HIT, Some(message.clone()));
    }
    if let Some(message) = warnings
        .iter()
        .find(|warning| warning.starts_with("ai_skipped:"))
    {
        return (FETCH_STATUS_AI_SKIPPED, Some(message.clone()));
    }
    if let Some(message) = warnings
        .iter()
        .find(|warning| warning.starts_with("provider_warning:"))
    {
        return (FETCH_STATUS_PROVIDER_WARNING, Some(message.clone()));
    }
    if let Some(message) = warnings
        .iter()
        .find(|warning| warning.starts_with("stored_unified_cache:"))
    {
        return (FETCH_STATUS_CACHE_STORE, Some(message.clone()));
    }
    (FETCH_STATUS_SUCCESS, None)
}

pub(crate) fn fetch_run_query_for_result(result: &SearchResult) -> String {
    [result.title.as_str(), result.artist.as_str()]
        .into_iter()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
        .if_empty_then(|| result.id.clone())
}

pub(crate) fn fetch_run_query_for_members(members: &[SearchResult]) -> String {
    members
        .first()
        .map(fetch_run_query_for_result)
        .unwrap_or_else(|| "aggregate members".into())
}
