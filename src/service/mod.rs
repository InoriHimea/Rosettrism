use std::path::PathBuf;
use std::time::Duration;

use clap::ValueEnum;
use serde::{Deserialize, Serialize};

use crate::cache::{default_ttl, UpstreamCache};
use crate::cached_provider::{fetch_cache_key, CachedProvider};
use crate::model::{Annotation, LyricDocument, UnifiedLyric, UnifiedLyricMode};
use crate::provider::{
    provider_for_with_options, FetchedLyric, LyricProvider, ProviderOptions, ProviderRequestPolicy,
    ProviderRuntime, SearchResult, Source,
};
use crate::{Error, Result};

mod ai_scoring;
mod fetch_runs;
mod unified_build;

#[cfg(test)]
mod tests;

pub(crate) use ai_scoring::*;
pub(crate) use fetch_runs::*;
pub(crate) use unified_build::*;

const DEFAULT_TRANSLATION_LANG: &str = "zh-Hans";

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, ValueEnum)]
#[serde(rename_all = "snake_case")]
pub enum MergeMode {
    #[default]
    Tracks,
    Inline,
}

impl From<MergeMode> for UnifiedLyricMode {
    fn from(value: MergeMode) -> Self {
        match value {
            MergeMode::Tracks => UnifiedLyricMode::Tracks,
            MergeMode::Inline => UnifiedLyricMode::Inline,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ValueEnum)]
#[serde(rename_all = "snake_case")]
pub enum SpecificFetchFormat {
    Raw,
    Json,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LyricNeed {
    Original,
    Timing,
    WordTiming,
    Translation,
    Ruby,
    Romanized,
}

impl LyricNeed {
    pub fn parse_list(value: Option<&str>) -> Vec<Self> {
        let Some(value) = value else {
            return vec![
                Self::Original,
                Self::Timing,
                Self::WordTiming,
                Self::Translation,
                Self::Ruby,
                Self::Romanized,
            ];
        };

        value
            .split(',')
            .filter_map(|part| match part.trim().to_ascii_lowercase().as_str() {
                "original" => Some(Self::Original),
                "timing" => Some(Self::Timing),
                "word-timing" | "word_timing" => Some(Self::WordTiming),
                "translation" => Some(Self::Translation),
                "ruby" => Some(Self::Ruby),
                "romanized" | "romaji" => Some(Self::Romanized),
                _ => None,
            })
            .collect()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AggregateFetchRequest {
    pub query: String,
    #[serde(default)]
    pub merge_mode: MergeMode,
    #[serde(default = "default_top")]
    pub top: usize,
    #[serde(default)]
    pub needs: Vec<LyricNeed>,
    #[serde(default = "default_translation_lang")]
    pub translation_lang: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sources: Option<Vec<Source>>,
    #[serde(default)]
    pub force: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ttl_seconds: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ai_scoring: Option<AiScoringConfig>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AiScoringConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FetchRunStructuredMetadata {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_count: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub candidate_count: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_warning_count: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cache_event: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AggregateFetchResponse {
    pub mode: MergeMode,
    pub results: Vec<UnifiedLyric>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ai_score: Option<AiScoringResult>,
    #[serde(default, skip_serializing_if = "is_default_fetch_metadata")]
    pub metadata: FetchRunStructuredMetadata,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiScoringResult {
    pub model: String,
    pub base_url: String,
    pub prompt_version: String,
    pub config_hash: String,
    pub candidate_summary_version: String,
    pub candidate_summary_hash: String,
    pub best_index: usize,
    pub scores: Vec<AiCandidateScore>,
    pub reason: String,
    pub created_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiCandidateScore {
    pub index: usize,
    pub source: String,
    pub title: String,
    pub artist: String,
    pub heuristic_score: f32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ai_score: Option<f32>,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceSearchRequest {
    pub query: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sources: Option<Vec<Source>>,
    #[serde(default = "default_search_limit")]
    pub limit: usize,
    #[serde(default)]
    pub force: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ttl_seconds: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct MultiSourceSearchResponse {
    pub query: String,
    pub results: Vec<SearchResult>,
    pub sources: Vec<SourceSearchResult>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<String>,
    #[serde(default, skip_serializing_if = "is_default_fetch_metadata")]
    pub metadata: FetchRunStructuredMetadata,
}

#[derive(Clone)]
pub struct ServiceContext {
    pub cache: Option<UpstreamCache>,
    pub provider_options: ProviderOptions,
    pub cookie: Option<String>,
    pub offline_db: Option<PathBuf>,
    pub default_ttl: Duration,
    pub force_refresh: bool,
    #[cfg(test)]
    pub(crate) provider_factory: Option<
        std::sync::Arc<
            dyn Fn(Source, Duration, bool) -> Result<Box<dyn LyricProvider>> + Send + Sync,
        >,
    >,
}

impl Default for ServiceContext {
    fn default() -> Self {
        Self {
            cache: None,
            provider_options: ProviderOptions::default(),
            cookie: None,
            offline_db: None,
            default_ttl: default_ttl(),
            force_refresh: false,
            #[cfg(test)]
            provider_factory: None,
        }
    }
}

const FETCH_STATUS_SUCCESS: &str = "success";
const FETCH_STATUS_ERROR: &str = "error";
const FETCH_STATUS_CACHE_HIT: &str = "cache_hit";
const FETCH_STATUS_CACHE_STORE: &str = "cache_store";
const FETCH_STATUS_PROVIDER_WARNING: &str = "provider_warning";
const FETCH_STATUS_AI_SKIPPED: &str = "ai_skipped";
const FETCH_STATUS_NO_LYRICS: &str = "no_lyrics_found";

impl ServiceContext {
    pub async fn aggregate_fetch(
        &self,
        request: AggregateFetchRequest,
    ) -> Result<AggregateFetchResponse> {
        let run_id = self
            .start_fetch_run(&request.query, None, "aggregate_fetch")
            .await?;
        let result = self.aggregate_fetch_inner(request).await;
        self.finish_fetch_run_from_result(run_id, &result).await?;
        result
    }

    async fn aggregate_fetch_inner(
        &self,
        mut request: AggregateFetchRequest,
    ) -> Result<AggregateFetchResponse> {
        if request.query.trim().is_empty() {
            return Err(Error::Service("query must not be empty".into()));
        }

        if request.needs.is_empty() {
            request.needs = LyricNeed::parse_list(None);
        }
        if request.translation_lang.trim().is_empty() {
            request.translation_lang = DEFAULT_TRANSLATION_LANG.to_string();
        }
        request.top = request.top.clamp(1, 5);

        let ttl = request
            .ttl_seconds
            .map(Duration::from_secs)
            .unwrap_or(self.default_ttl);
        let force = self.force_refresh || request.force;

        let cache_key = UpstreamCache::unified_key(&request)?;
        if !force {
            if let Some(cache) = &self.cache {
                if let Some(hit) = cache.get_unified_fresh(&cache_key).await? {
                    let mut response: AggregateFetchResponse = serde_json::from_slice(&hit.body)?;
                    response
                        .warnings
                        .push(format!("served_unified_cache: id={}", hit.id));
                    response.metadata.cache_event = Some("cache_hit".into());
                    return Ok(response);
                }
            }
        }

        let sources = request.sources.clone().unwrap_or_else(|| {
            default_sources_for_needs(&request.needs, self.provider_options.allow_experimental)
        });
        let mut handles = Vec::new();
        for source in sources {
            let context = self.clone();
            let query = request.query.clone();
            handles.push(tokio::spawn(async move {
                context
                    .fetch_first_candidate(source, &query, ttl, force)
                    .await
            }));
        }

        let mut candidates = Vec::new();
        let mut warnings = Vec::new();
        for handle in handles {
            match handle.await {
                Ok(Ok(candidate)) => candidates.push(candidate),
                Ok(Err(err)) => warnings.push(err.to_string()),
                Err(err) => warnings.push(format!("provider_warning: provider task failed: {err}")),
            }
        }

        if candidates.is_empty() {
            return Err(Error::Provider(
                "no_lyrics_found: all selected sources returned no usable lyric".into(),
            ));
        }
        warnings.retain(|warning| !is_low_signal_aggregate_warning(warning));

        candidates.sort_by(compare_candidate_quality);
        let mut ai_score = None;
        match self
            .select_best_candidate_with_ai(&candidates, request.ai_scoring.as_ref())
            .await
        {
            Ok(Some(selection)) => {
                warnings.push(format!(
                    "AI lyric selection chose {} with score {:.1}: {}",
                    candidates[selection.best_index].source.cli_name(),
                    selection.best_score().unwrap_or(0.0),
                    selection.reason
                ));
                let selected = candidates.remove(selection.best_index);
                candidates.insert(0, selected);
                ai_score = Some(selection);
            }
            Ok(None) => {}
            Err(err) => warnings.push(format!("ai_skipped: {err}")),
        }

        let results = candidates
            .iter()
            .take(request.top)
            .map(|candidate| build_unified(candidate, &candidates, request.merge_mode))
            .collect::<Result<Vec<_>>>()?;

        let provider_warning_count = provider_warning_count(&warnings);
        let mut response = AggregateFetchResponse {
            mode: request.merge_mode,
            results,
            warnings,
            ai_score: ai_score.clone(),
            metadata: FetchRunStructuredMetadata {
                provider_count: Some(
                    candidates
                        .iter()
                        .map(|candidate| candidate.source.cli_name())
                        .collect::<std::collections::BTreeSet<_>>()
                        .len(),
                ),
                candidate_count: Some(candidates.len()),
                provider_warning_count: Some(provider_warning_count),
                cache_event: None,
            },
        };

        if let Some(cache) = &self.cache {
            let body = serde_json::to_vec(&response)?;
            let dependency_keys = response
                .results
                .iter()
                .flat_map(|result| result.cache_refs.clone())
                .collect::<Vec<_>>();
            let id = cache
                .put_unified(&cache_key, &body, &dependency_keys, ttl)
                .await?;
            if let Some(ai_score) = &ai_score {
                cache
                    .put_ai_score(id, &serde_json::to_value(ai_score)?)
                    .await?;
            }
            response
                .warnings
                .push(format!("stored_unified_cache: id={id}"));
            response.metadata.cache_event = Some("cache_store".into());
        }

        Ok(response)
    }

    pub async fn search_sources(
        &self,
        request: SourceSearchRequest,
    ) -> Result<MultiSourceSearchResponse> {
        let run_id = self
            .start_fetch_run(&request.query, None, "search_sources")
            .await?;
        let result = self.search_sources_inner(request).await;
        self.finish_fetch_run_from_result(run_id, &result).await?;
        result
    }

    async fn search_sources_inner(
        &self,
        mut request: SourceSearchRequest,
    ) -> Result<MultiSourceSearchResponse> {
        request.query = request.query.trim().to_string();
        if request.query.is_empty() {
            return Err(Error::Service("query must not be empty".into()));
        }
        request.limit = request.limit.clamp(1, 100);
        let ttl = request
            .ttl_seconds
            .map(Duration::from_secs)
            .unwrap_or(self.default_ttl);
        let force = self.force_refresh || request.force;
        let sources = request
            .sources
            .clone()
            .unwrap_or_else(|| default_search_sources(self.provider_options.allow_experimental));

        let mut handles = Vec::new();
        for source in sources {
            let context = self.clone();
            let query = request.query.clone();
            handles.push(tokio::spawn(async move {
                context
                    .search_source_specific(source, &query, Some(ttl), force)
                    .await
            }));
        }

        let mut groups = Vec::new();
        let mut warnings = Vec::new();
        for handle in handles {
            match handle.await {
                Ok(Ok(mut group)) => {
                    group.results.truncate(request.limit);
                    groups.push(group);
                }
                Ok(Err(err)) => warnings.push(err.to_string()),
                Err(err) => warnings.push(format!("provider_warning: provider task failed: {err}")),
            }
        }

        let mut results = Vec::new();
        for group in &groups {
            results.extend(group.results.iter().cloned());
        }
        results.truncate(request.limit);

        let provider_warning_count = warnings.len();
        Ok(MultiSourceSearchResponse {
            query: request.query,
            metadata: FetchRunStructuredMetadata {
                provider_count: Some(groups.len() + provider_warning_count),
                candidate_count: Some(results.len()),
                provider_warning_count: Some(provider_warning_count),
                cache_event: None,
            },
            results,
            sources: groups,
            warnings,
        })
    }

    pub async fn fetch_source_specific(
        &self,
        source: Source,
        query: &str,
        format: SpecificFetchFormat,
        top: usize,
        ttl: Option<Duration>,
        force: bool,
        enrich: bool,
    ) -> Result<SpecificFetchResult> {
        let ttl = ttl.unwrap_or(self.default_ttl);
        let provider = self
            .provider(source, ttl, force || self.force_refresh)
            .await?;
        let results = provider.search(query).await?;
        let limit = top.clamp(1, 20);
        let selected = results.into_iter().take(limit).collect::<Vec<_>>();
        if selected.is_empty() {
            return Err(Error::Provider("no lyric candidates found".into()));
        }

        if limit == 1 {
            let result = selected.into_iter().next().expect("selected is not empty");
            let fetched = provider.fetch(&result).await?;
            return match format {
                SpecificFetchFormat::Raw => {
                    let raw = decode_source_raw_bytes(&fetched.raw, fetched.input_format)?;
                    Ok(SpecificFetchResult::Raw {
                        source,
                        result,
                        raw,
                    })
                }
                SpecificFetchFormat::Json => {
                    if enrich {
                        self.fetch_enriched_source_result(source, result, fetched, ttl, force)
                            .await
                    } else {
                        let input_format = fetched.input_format;
                        let annotations = fetched.annotations.clone();
                        let document = decode_fetched(fetched)?;
                        Ok(SpecificFetchResult::Json {
                            source,
                            result,
                            input_format,
                            document,
                            annotations,
                            unified: None,
                        })
                    }
                }
            };
        }

        match format {
            SpecificFetchFormat::Raw => {
                let mut items = Vec::new();
                let mut warnings = Vec::new();
                for result in selected {
                    match provider.fetch(&result).await {
                        Ok(fetched) => {
                            let raw = decode_source_raw_bytes(&fetched.raw, fetched.input_format)?;
                            items.push(SpecificRawItem { result, raw });
                        }
                        Err(err) => warnings.push(format!("{}: {err}", result.id)),
                    }
                }
                if items.is_empty() {
                    return Err(Error::Provider(format!(
                        "no lyric candidates could be fetched: {}",
                        warnings.join("; ")
                    )));
                }
                Ok(SpecificFetchResult::RawMany {
                    source,
                    results: items,
                    warnings,
                })
            }
            SpecificFetchFormat::Json => {
                let mut items = Vec::new();
                let mut warnings = Vec::new();
                for result in selected {
                    match provider.fetch(&result).await {
                        Ok(fetched) => {
                            let input_format = fetched.input_format;
                            let annotations = fetched.annotations.clone();
                            let document = decode_fetched(fetched)?;
                            items.push(SpecificJsonItem {
                                result,
                                input_format,
                                document,
                                annotations,
                            });
                        }
                        Err(err) => warnings.push(format!("{}: {err}", result.id)),
                    }
                }
                if items.is_empty() {
                    return Err(Error::Provider(format!(
                        "no lyric candidates could be fetched: {}",
                        warnings.join("; ")
                    )));
                }
                Ok(SpecificFetchResult::JsonMany {
                    source,
                    results: items,
                    warnings,
                })
            }
        }
    }

    pub async fn search_source_specific(
        &self,
        source: Source,
        query: &str,
        ttl: Option<Duration>,
        force: bool,
    ) -> Result<SourceSearchResult> {
        let ttl = ttl.unwrap_or(self.default_ttl);
        let provider = self
            .provider(source, ttl, force || self.force_refresh)
            .await?;
        Ok(SourceSearchResult {
            source,
            query: query.to_string(),
            results: provider.search(query).await?,
        })
    }

    pub async fn fetch_source_result(
        &self,
        result: SearchResult,
        format: SpecificFetchFormat,
        ttl: Option<Duration>,
        force: bool,
        enrich: bool,
    ) -> Result<SpecificFetchResult> {
        let query = fetch_run_query_for_result(&result);
        let run_id = self
            .start_fetch_run(&query, Some(result.source), "fetch_source_result")
            .await?;
        let fetch_result = self
            .fetch_source_result_inner(result, format, ttl, force, enrich)
            .await;
        self.finish_fetch_run_from_result(run_id, &fetch_result)
            .await?;
        fetch_result
    }

    async fn fetch_source_result_inner(
        &self,
        result: SearchResult,
        format: SpecificFetchFormat,
        ttl: Option<Duration>,
        force: bool,
        enrich: bool,
    ) -> Result<SpecificFetchResult> {
        let ttl = ttl.unwrap_or(self.default_ttl);
        let source = result.source;
        let provider = self
            .provider(source, ttl, force || self.force_refresh)
            .await?;
        let fetched = provider.fetch(&result).await?;
        match format {
            SpecificFetchFormat::Raw => {
                let raw = decode_source_raw_bytes(&fetched.raw, fetched.input_format)?;
                Ok(SpecificFetchResult::Raw {
                    source,
                    result,
                    raw,
                })
            }
            SpecificFetchFormat::Json => {
                if enrich {
                    self.fetch_enriched_source_result(source, result, fetched, ttl, force)
                        .await
                } else {
                    let input_format = fetched.input_format;
                    let annotations = fetched.annotations.clone();
                    let document = decode_fetched(fetched)?;
                    Ok(SpecificFetchResult::Json {
                        source,
                        result,
                        input_format,
                        document,
                        annotations,
                        unified: None,
                    })
                }
            }
        }
    }

    pub async fn fetch_aggregate_members(
        &self,
        members: Vec<SearchResult>,
        merge_mode: MergeMode,
        ttl: Option<Duration>,
        force: bool,
        ai_scoring: Option<&AiScoringConfig>,
    ) -> Result<UnifiedLyric> {
        let query = fetch_run_query_for_members(&members);
        let run_id = self
            .start_fetch_run(&query, None, "fetch_aggregate_members")
            .await?;
        let result = self
            .fetch_aggregate_members_inner(members, merge_mode, ttl, force, ai_scoring)
            .await;
        self.finish_fetch_run_from_result(run_id, &result).await?;
        result
    }

    async fn fetch_aggregate_members_inner(
        &self,
        members: Vec<SearchResult>,
        merge_mode: MergeMode,
        ttl: Option<Duration>,
        force: bool,
        ai_scoring: Option<&AiScoringConfig>,
    ) -> Result<UnifiedLyric> {
        if members.is_empty() {
            return Err(Error::Service(
                "aggregate result did not include source members".into(),
            ));
        }

        let ttl = ttl.unwrap_or(self.default_ttl);
        let mut candidates = Vec::new();
        let mut warnings = Vec::new();
        for member in members {
            match self.fetch_selected_candidate(member, ttl, force).await {
                Ok(candidate) => candidates.push(candidate),
                Err(err) => warnings.push(err.to_string()),
            }
        }

        if candidates.is_empty() {
            return Err(Error::Provider(format!(
                "no aggregate members could be fetched: {}",
                warnings.join("; ")
            )));
        }

        candidates.sort_by(compare_candidate_quality);
        match self
            .select_best_candidate_with_ai(&candidates, ai_scoring)
            .await
        {
            Ok(Some(selection)) => {
                warnings.push(format!(
                    "AI lyric selection chose {} with score {:.1}: {}",
                    candidates[selection.best_index].source.cli_name(),
                    selection.best_score().unwrap_or(0.0),
                    selection.reason
                ));
                let selected = candidates.remove(selection.best_index);
                candidates.insert(0, selected);
            }
            Ok(None) => {}
            Err(err) => warnings.push(format!("ai_skipped: {err}")),
        }

        let mut unified = build_unified(&candidates[0], &candidates, merge_mode)?;
        unified.meta.source = Some(format!(
            "aggregate({})",
            unique_source_names(&candidates).join("+")
        ));
        unified.warnings = warnings;
        Ok(unified)
    }

    async fn start_fetch_run(
        &self,
        query: &str,
        source: Option<Source>,
        mode: &str,
    ) -> Result<Option<i64>> {
        match self.cache.as_ref() {
            Some(cache) => Ok(Some(cache.start_fetch_run(query, source, mode).await?)),
            None => Ok(None),
        }
    }

    async fn finish_fetch_run_from_result<T: FetchRunOutcome>(
        &self,
        run_id: Option<i64>,
        result: &Result<T>,
    ) -> Result<()> {
        let Some(run_id) = run_id else {
            return Ok(());
        };
        let (status, message) = classify_fetch_run_result(result);
        let metadata = fetch_run_metadata_from_result(result);
        if let Some(cache) = &self.cache {
            cache
                .finish_fetch_run(run_id, status, message.as_deref(), metadata)
                .await?;
        }
        Ok(())
    }

    async fn select_best_candidate_with_ai(
        &self,
        candidates: &[SourceCandidate],
        config: Option<&AiScoringConfig>,
    ) -> Result<Option<AiScoringResult>> {
        let Some(config) = resolve_ai_scoring_config(config) else {
            return Ok(None);
        };
        if candidates.len() < 2 {
            return Ok(None);
        }

        let summaries = candidate_summaries(candidates);
        score_ai_candidate_summaries(&summaries, &config).await
    }

    pub async fn replay_ai_score_for_unified_cache(
        &self,
        unified_cache_id: i64,
        config: Option<&AiScoringConfig>,
    ) -> Result<AiScoringResult> {
        let Some(config) = resolve_ai_scoring_config(config) else {
            return Err(Error::Service("AI scoring is not configured".into()));
        };
        let cache = self
            .cache
            .as_ref()
            .ok_or_else(|| Error::Service("cache is not configured".into()))?;
        let body = cache
            .unified_body(unified_cache_id)
            .await?
            .ok_or_else(|| Error::Service("unified cache entry not found".into()))?;
        let response: AggregateFetchResponse = serde_json::from_slice(&body)?;
        let unified = response
            .results
            .first()
            .ok_or_else(|| Error::Service("unified cache entry has no candidates".into()))?;
        let summaries = candidate_summaries_from_unified(unified);
        if summaries.len() < 2 {
            return Err(Error::Service(
                "unified cache entry has fewer than two candidates".into(),
            ));
        }
        let score = score_ai_candidate_summaries(&summaries, &config)
            .await?
            .ok_or_else(|| Error::Service("AI scoring returned no selection".into()))?;
        cache
            .put_ai_score(unified_cache_id, &serde_json::to_value(&score)?)
            .await?;
        Ok(score)
    }

    pub async fn provider(
        &self,
        source: Source,
        ttl: Duration,
        force: bool,
    ) -> Result<Box<dyn LyricProvider>> {
        #[cfg(test)]
        if let Some(factory) = &self.provider_factory {
            return factory(source, ttl, force);
        }
        let credential =
            credential_for_source(source, self.cookie.as_deref(), self.offline_db.as_ref()).await?;
        let inner = provider_for_with_options(source, credential, self.provider_options)?;
        let runtime = Box::new(ProviderRuntime::new(
            source,
            inner,
            ProviderRequestPolicy::from(source.provider_config()),
        ));
        if let Some(cache) = &self.cache {
            Ok(Box::new(CachedProvider::new(
                source,
                runtime,
                cache.clone(),
                ttl,
                force,
            )))
        } else {
            Ok(runtime)
        }
    }

    async fn fetch_first_candidate(
        &self,
        source: Source,
        query: &str,
        ttl: Duration,
        force: bool,
    ) -> Result<SourceCandidate> {
        let provider = self.provider(source, ttl, force).await?;
        let results = provider.search(query).await?;
        let result = results.into_iter().next().ok_or_else(|| {
            Error::Provider(format!("{} returned no candidates", source.cli_name()))
        })?;
        let fetched = provider.fetch(&result).await?;
        let annotations = fetched.annotations.clone();
        let document = decode_fetched(fetched)?;
        if document.lines.is_empty() {
            return Err(Error::Provider(format!(
                "{} candidate {} contained no lyric lines",
                source.cli_name(),
                result.id
            )));
        }

        let quality = quality_for_document(&document, source);
        Ok(SourceCandidate {
            source,
            result,
            document,
            quality,
            annotations,
        })
    }

    async fn fetch_enriched_source_result(
        &self,
        source: Source,
        result: SearchResult,
        fetched: FetchedLyric,
        ttl: Duration,
        force: bool,
    ) -> Result<SpecificFetchResult> {
        let input_format = fetched.input_format;
        let annotations = fetched.annotations.clone();
        let document = decode_fetched(fetched)?;
        let mut candidates = Vec::new();
        let base_candidate = SourceCandidate {
            source,
            result: result.clone(),
            quality: quality_for_document(&document, source),
            document: document.clone(),
            annotations: annotations.clone(),
        };
        candidates.push(base_candidate);
        let query = [
            document.meta.title.as_deref(),
            document.meta.artist.as_deref(),
        ]
        .into_iter()
        .flatten()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>()
        .join(" ");
        let query = if query.is_empty() {
            format!("{} {}", result.title, result.artist)
        } else {
            query
        };
        let needs = LyricNeed::parse_list(None);
        let enrichment_sources =
            default_sources_for_needs(&needs, self.provider_options.allow_experimental)
                .into_iter()
                .filter(|candidate_source| *candidate_source != source)
                .collect::<Vec<_>>();
        let mut warnings = Vec::new();
        for enrichment_source in enrichment_sources {
            match self
                .fetch_first_candidate(enrichment_source, &query, ttl, force || self.force_refresh)
                .await
            {
                Ok(candidate) => candidates.push(candidate),
                Err(err) => warnings.push(format!(
                    "{} enrichment skipped: {err}",
                    enrichment_source.cli_name()
                )),
            }
        }
        let mut unified = build_unified(&candidates[0], &candidates, MergeMode::Inline)?;
        unified.meta.source = Some(format!(
            "enriched({})",
            unique_source_names(&candidates).join("+")
        ));
        unified.warnings = warnings;

        Ok(SpecificFetchResult::Json {
            source,
            result,
            input_format,
            document,
            annotations,
            unified: Some(unified),
        })
    }

    async fn fetch_selected_candidate(
        &self,
        result: SearchResult,
        ttl: Duration,
        force: bool,
    ) -> Result<SourceCandidate> {
        let source = result.source;
        let provider = self
            .provider(source, ttl, force || self.force_refresh)
            .await?;
        let fetched = provider.fetch(&result).await?;
        let annotations = fetched.annotations.clone();
        let document = decode_fetched(fetched)?;
        if document.lines.is_empty() {
            return Err(Error::Provider(format!(
                "{} candidate {} contained no lyric lines",
                source.cli_name(),
                result.id
            )));
        }

        let quality = quality_for_document(&document, source);
        Ok(SourceCandidate {
            source,
            result,
            document,
            quality,
            annotations,
        })
    }
}

pub enum SpecificFetchResult {
    Raw {
        source: Source,
        result: SearchResult,
        raw: Vec<u8>,
    },
    Json {
        source: Source,
        result: SearchResult,
        input_format: crate::decoder::InputFormat,
        document: LyricDocument,
        annotations: Vec<Annotation>,
        unified: Option<UnifiedLyric>,
    },
    RawMany {
        source: Source,
        results: Vec<SpecificRawItem>,
        warnings: Vec<String>,
    },
    JsonMany {
        source: Source,
        results: Vec<SpecificJsonItem>,
        warnings: Vec<String>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceSearchResult {
    pub source: Source,
    pub query: String,
    pub results: Vec<SearchResult>,
}

#[derive(Debug, Serialize)]
pub struct SpecificRawItem {
    pub result: SearchResult,
    #[serde(serialize_with = "serialize_raw_text")]
    pub raw: Vec<u8>,
}

#[derive(Debug, Serialize)]
pub struct SpecificJsonItem {
    pub result: SearchResult,
    pub input_format: crate::decoder::InputFormat,
    pub document: LyricDocument,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub annotations: Vec<Annotation>,
}

fn default_sources_for_needs(needs: &[LyricNeed], allow_experimental: bool) -> Vec<Source> {
    let mut sources = vec![
        Source::Netease,
        Source::Qq,
        Source::Kugou,
        Source::Lrclib,
        Source::Migu,
    ];
    if needs.contains(&LyricNeed::Ruby) || needs.contains(&LyricNeed::Translation) {
        sources.push(Source::Utaten);
    }
    sources.push(Source::Joysound);

    sources.dedup();
    if allow_experimental || env_allows_experimental() {
        sources.push(Source::SpotifyLyrics);
        sources.push(Source::OfflineDb);
    }
    sources
}

fn default_search_sources(allow_experimental: bool) -> Vec<Source> {
    let mut sources = vec![
        Source::Netease,
        Source::Qq,
        Source::Kugou,
        Source::Lrclib,
        Source::Migu,
        Source::Utaten,
        Source::Joysound,
    ];
    if allow_experimental || env_allows_experimental() {
        sources.push(Source::OfflineDb);
    }
    sources
}

fn is_low_signal_aggregate_warning(warning: &str) -> bool {
    warning.contains("supports direct URL or id")
        || warning.contains("returned no candidates")
        || warning.contains("contained no lyric lines")
}

pub async fn credential_for_source(
    source: Source,
    cli_cookie: Option<&str>,
    offline_db: Option<&PathBuf>,
) -> Result<Option<String>> {
    if source == Source::OfflineDb {
        if let Some(path) = offline_db {
            return Ok(Some(path.display().to_string()));
        }
        return Ok(env_var_any(&[
            "ROSETTRISM_OFFLINE_DB",
            "LRC_DECODE_OFFLINE_DB",
        ]));
    }

    if source == Source::SpotifyLyrics {
        if let Some(cookie) = cli_cookie {
            return Ok(Some(cookie.to_string()));
        }
        if let Some(token) = env_var_any(&["ROSETTRISM_SPOTIFY_BEARER_TOKEN"]) {
            return Ok(Some(token));
        }
        if let Some(path) = env_var_any(&["ROSETTRISM_SPOTIFY_COOKIE_FILE"]) {
            let cookie = tokio::fs::read_to_string(&path).await.map_err(|err| {
                Error::Io(std::io::Error::new(
                    err.kind(),
                    format!("failed to read Spotify cookie file {path}: {err}"),
                ))
            })?;
            let cookie = cookie.trim();
            if !cookie.is_empty() {
                return Ok(Some(cookie.to_string()));
            }
        }
    }

    Ok(cookie_for_source(source, cli_cookie))
}

pub fn cookie_for_source(source: Source, cli_cookie: Option<&str>) -> Option<String> {
    if let Some(cookie) = cli_cookie {
        return Some(cookie.to_string());
    }

    let source_keys = match source {
        Source::AppleMusic => &[
            "ROSETTRISM_APPLE_MUSIC_COOKIE",
            "LRC_DECODE_APPLE_MUSIC_COOKIE",
        ][..],
        Source::Animesongz => &[
            "ROSETTRISM_ANIMESONGZ_COOKIE",
            "LRC_DECODE_ANIMESONGZ_COOKIE",
        ],
        Source::Awa => &["ROSETTRISM_AWA_COOKIE", "LRC_DECODE_AWA_COOKIE"],
        Source::Azlyrics => &["ROSETTRISM_AZLYRICS_COOKIE", "LRC_DECODE_AZLYRICS_COOKIE"],
        Source::BrowserMxm => &[
            "ROSETTRISM_BROWSER_MXM_COOKIE",
            "LRC_DECODE_BROWSER_MXM_COOKIE",
        ],
        Source::Genius => &["ROSETTRISM_GENIUS_COOKIE", "LRC_DECODE_GENIUS_COOKIE"],
        Source::JLyric => &["ROSETTRISM_J_LYRIC_COOKIE", "LRC_DECODE_J_LYRIC_COOKIE"],
        Source::JTotal => &["ROSETTRISM_J_TOTAL_COOKIE", "LRC_DECODE_J_TOTAL_COOKIE"],
        Source::Joysound => &["ROSETTRISM_JOYSOUND_COOKIE", "LRC_DECODE_JOYSOUND_COOKIE"],
        Source::Kashinavi => &["ROSETTRISM_KASHINAVI_COOKIE", "LRC_DECODE_KASHINAVI_COOKIE"],
        Source::Kkbox => &["ROSETTRISM_KKBOX_COOKIE", "LRC_DECODE_KKBOX_COOKIE"],
        Source::Kugou => &["ROSETTRISM_KUGOU_COOKIE", "LRC_DECODE_KUGOU_COOKIE"],
        Source::LineMusic => &[
            "ROSETTRISM_LINE_MUSIC_COOKIE",
            "LRC_DECODE_LINE_MUSIC_COOKIE",
        ],
        Source::Lrclib => &[],
        Source::LyricalNonsense => &[
            "ROSETTRISM_LYRICAL_NONSENSE_COOKIE",
            "LRC_DECODE_LYRICAL_NONSENSE_COOKIE",
        ],
        Source::Migu => &["ROSETTRISM_MIGU_COOKIE", "LRC_DECODE_MIGU_COOKIE"],
        Source::Musixmatch => &[
            "ROSETTRISM_MUSIXMATCH_API_KEY",
            "LRC_DECODE_MUSIXMATCH_API_KEY",
        ],
        Source::Netease => &["ROSETTRISM_NETEASE_COOKIE", "LRC_DECODE_NETEASE_COOKIE"],
        Source::OfflineDb => &["ROSETTRISM_OFFLINE_DB", "LRC_DECODE_OFFLINE_DB"],
        Source::PetitLyrics => &[
            "ROSETTRISM_PETIT_LYRICS_COOKIE",
            "LRC_DECODE_PETIT_LYRICS_COOKIE",
        ],
        Source::Qq => &["ROSETTRISM_QQ_COOKIE", "LRC_DECODE_QQ_COOKIE"],
        Source::RockLyric => &["ROSETTRISM_ROCKLYRIC_COOKIE", "LRC_DECODE_ROCKLYRIC_COOKIE"],
        Source::Songtexte => &["ROSETTRISM_SONGTEXTE_COOKIE", "LRC_DECODE_SONGTEXTE_COOKIE"],
        Source::SpotifyLyrics => &[
            "ROSETTRISM_SPOTIFY_BEARER_TOKEN",
            "ROSETTRISM_SPOTIFY_COOKIE",
            "LRC_DECODE_SPOTIFY_COOKIE",
        ],
        Source::TuneCore => &["ROSETTRISM_TUNECORE_COOKIE", "LRC_DECODE_TUNECORE_COOKIE"],
        Source::UtaNet => &["ROSETTRISM_UTA_NET_COOKIE", "LRC_DECODE_UTA_NET_COOKIE"],
        Source::Utaten => &["ROSETTRISM_UTATEN_COOKIE", "LRC_DECODE_UTATEN_COOKIE"],
        Source::Utamap => &["ROSETTRISM_UTAMAP_COOKIE", "LRC_DECODE_UTAMAP_COOKIE"],
    };

    env_var_any(source_keys).or_else(|| env_var_any(&["ROSETTRISM_COOKIE", "LRC_DECODE_COOKIE"]))
}

pub fn env_var_any(names: &[&str]) -> Option<String> {
    names
        .iter()
        .find_map(|name| std::env::var(name).ok())
        .filter(|value| !value.trim().is_empty())
}

pub fn parse_ttl(value: &str) -> Result<Duration> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(Error::Service("ttl must not be empty".into()));
    }
    let (number, multiplier) = match trimmed.chars().last().unwrap_or_default() {
        'd' | 'D' => (&trimmed[..trimmed.len() - 1], 24 * 60 * 60),
        'h' | 'H' => (&trimmed[..trimmed.len() - 1], 60 * 60),
        'm' | 'M' => (&trimmed[..trimmed.len() - 1], 60),
        's' | 'S' => (&trimmed[..trimmed.len() - 1], 1),
        _ => (trimmed, 1),
    };
    let amount = number
        .parse::<u64>()
        .map_err(|err| Error::Service(format!("invalid ttl `{value}`: {err}")))?;
    Ok(Duration::from_secs(amount.saturating_mul(multiplier)))
}

pub fn source_from_cli_name(value: &str) -> Result<Source> {
    Source::from_str(value, true)
        .map_err(|err| Error::Service(format!("invalid source `{value}`: {err}")))
}

fn env_allows_experimental() -> bool {
    std::env::var("ROSETTRISM_ALLOW_EXPERIMENTAL")
        .or_else(|_| std::env::var("LRC_DECODE_ALLOW_EXPERIMENTAL"))
        .ok()
        .is_some_and(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
}

fn default_top() -> usize {
    1
}

fn default_search_limit() -> usize {
    100
}

fn default_translation_lang() -> String {
    DEFAULT_TRANSLATION_LANG.to_string()
}
