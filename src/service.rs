use std::path::PathBuf;
use std::time::Duration;

use clap::ValueEnum;
use reqwest::header::{AUTHORIZATION, CONTENT_TYPE};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};

use crate::cache::{default_ttl, now_unix, FetchRunMetadata, UpstreamCache};
use crate::cached_provider::CachedProvider;
use crate::decoder::{decode_bytes, decode_raw_bytes, InputFormat};
use crate::model::{
    Annotation, InlineLyricLine, LyricDocument, LyricLine, LyricMeta, LyricTrack, LyricTrackKind,
    LyricTrackQuality, UnifiedLyric, UnifiedLyricMode, UnifiedLyricScore,
};
use crate::provider::{
    provider_for_with_options, FetchedLyric, LyricProvider, ProviderOptions, ProviderRequestPolicy,
    ProviderRuntime, SearchResult, Source,
};
use crate::{Error, Result};

const DEFAULT_TRANSLATION_LANG: &str = "zh-Hans";
const DEFAULT_OPENAI_MODEL: &str = "gpt-4o-mini";
const AI_PROMPT_VERSION: &str = "ai-score-prompt-v1";
const AI_CANDIDATE_SUMMARY_VERSION: &str = "candidate-summary-v1";

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
        let run_id = self.start_fetch_run(&request.query, None, "aggregate_fetch")?;
        let result = self.aggregate_fetch_inner(request).await;
        self.finish_fetch_run_from_result(run_id, &result)?;
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
                if let Some(hit) = cache.get_unified_fresh(&cache_key)? {
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
            .collect::<Vec<_>>();

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
            let id = cache.put_unified(&cache_key, &body, &dependency_keys, ttl)?;
            if let Some(ai_score) = &ai_score {
                cache.put_ai_score(id, &serde_json::to_value(ai_score)?)?;
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
        let run_id = self.start_fetch_run(&request.query, None, "search_sources")?;
        let result = self.search_sources_inner(request).await;
        self.finish_fetch_run_from_result(run_id, &result)?;
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
        let run_id = self.start_fetch_run(&query, Some(result.source), "fetch_source_result")?;
        let fetch_result = self
            .fetch_source_result_inner(result, format, ttl, force, enrich)
            .await;
        self.finish_fetch_run_from_result(run_id, &fetch_result)?;
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
        let run_id = self.start_fetch_run(&query, None, "fetch_aggregate_members")?;
        let result = self
            .fetch_aggregate_members_inner(members, merge_mode, ttl, force, ai_scoring)
            .await;
        self.finish_fetch_run_from_result(run_id, &result)?;
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

        let mut unified = build_unified(&candidates[0], &candidates, merge_mode);
        unified.meta.source = Some(format!(
            "aggregate({})",
            unique_source_names(&candidates).join("+")
        ));
        unified.warnings = warnings;
        Ok(unified)
    }

    fn start_fetch_run(
        &self,
        query: &str,
        source: Option<Source>,
        mode: &str,
    ) -> Result<Option<i64>> {
        self.cache
            .as_ref()
            .map(|cache| cache.start_fetch_run(query, source, mode))
            .transpose()
    }

    fn finish_fetch_run_from_result<T: FetchRunOutcome>(
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
            cache.finish_fetch_run(run_id, status, message.as_deref(), metadata)?;
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
            .unified_body(unified_cache_id)?
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
        cache.put_ai_score(unified_cache_id, &serde_json::to_value(&score)?)?;
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
        let input_format = fetched.input_format;
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
            input_format,
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
            input_format,
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
        let mut unified = build_unified(&candidates[0], &candidates, MergeMode::Inline);
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
        let input_format = fetched.input_format;
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
            input_format,
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

fn serialize_raw_text<S>(raw: &[u8], serializer: S) -> std::result::Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    serializer.serialize_str(&String::from_utf8_lossy(raw))
}

#[derive(Debug, Clone)]
struct SourceCandidate {
    source: Source,
    result: SearchResult,
    input_format: crate::decoder::InputFormat,
    document: LyricDocument,
    quality: LyricTrackQuality,
    annotations: Vec<Annotation>,
}

#[derive(Debug, Clone)]
struct ResolvedAiScoringConfig {
    base_url: String,
    api_key: String,
    model: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct AiCandidateSummary {
    index: usize,
    source: String,
    title: String,
    artist: String,
    line_count: usize,
    timed_line_count: usize,
    word_timing_count: usize,
    heuristic_score: f32,
    sample_lines: Vec<String>,
}

fn compare_candidate_quality(
    left: &SourceCandidate,
    right: &SourceCandidate,
) -> std::cmp::Ordering {
    right
        .quality
        .score
        .partial_cmp(&left.quality.score)
        .unwrap_or(std::cmp::Ordering::Equal)
}

fn resolve_ai_scoring_config(config: Option<&AiScoringConfig>) -> Option<ResolvedAiScoringConfig> {
    let enabled = config.is_some_and(|config| config.enabled)
        || std::env::var("ROSETTRISM_OPENAI_API_KEY").is_ok();
    if !enabled {
        return None;
    }
    let base_url = config
        .and_then(|config| config.base_url.as_deref())
        .filter(|value| !value.trim().is_empty())
        .map(str::trim)
        .map(ToOwned::to_owned)
        .or_else(|| std::env::var("ROSETTRISM_OPENAI_BASE_URL").ok())
        .unwrap_or_else(|| "https://api.openai.com/v1".into());
    let api_key = config
        .and_then(|config| config.api_key.as_deref())
        .filter(|value| !value.trim().is_empty())
        .map(str::trim)
        .map(ToOwned::to_owned)
        .or_else(|| std::env::var("ROSETTRISM_OPENAI_API_KEY").ok())?;
    let model = config
        .and_then(|config| config.model.as_deref())
        .filter(|value| !value.trim().is_empty())
        .map(str::trim)
        .map(ToOwned::to_owned)
        .or_else(|| std::env::var("ROSETTRISM_OPENAI_MODEL").ok())
        .unwrap_or_else(|| DEFAULT_OPENAI_MODEL.into());

    Some(ResolvedAiScoringConfig {
        base_url,
        api_key,
        model,
    })
}

fn openai_chat_completions_url(base_url: &str) -> Result<String> {
    let base = base_url.trim().trim_end_matches('/');
    if base.is_empty() {
        return Err(Error::Service("OpenAI compatible base URL is empty".into()));
    }
    if base.ends_with("/chat/completions") {
        return Ok(base.to_string());
    }
    Ok(format!("{base}/chat/completions"))
}

async fn score_ai_candidate_summaries(
    summaries: &[AiCandidateSummary],
    config: &ResolvedAiScoringConfig,
) -> Result<Option<AiScoringResult>> {
    let summary_json = serde_json::to_string(summaries)?;
    let summary_hash = hash_json(summary_json.as_bytes());
    let request = json!({
        "model": config.model,
        "response_format": { "type": "json_object" },
        "messages": [
            {
                "role": "system",
                "content": ai_scoring_system_prompt()
            },
            {
                "role": "user",
                "content": summary_json
            }
        ]
    });
    let response = reqwest::Client::new()
        .post(openai_chat_completions_url(&config.base_url)?)
        .header(AUTHORIZATION, format!("Bearer {}", config.api_key))
        .header(CONTENT_TYPE, "application/json")
        .json(&request)
        .send()
        .await?;
    let status = response.status();
    let body = response.text().await?;
    if !status.is_success() {
        return Err(Error::Provider(format!(
            "OpenAI compatible endpoint returned {status}: {body}"
        )));
    }
    let value: serde_json::Value = serde_json::from_str(&body)?;
    let content = value
        .get("choices")
        .and_then(|choices| choices.get(0))
        .and_then(|choice| choice.get("message"))
        .and_then(|message| message.get("content"))
        .and_then(|content| content.as_str())
        .ok_or_else(|| {
            Error::Provider("OpenAI compatible response missing message content".into())
        })?;
    parse_ai_candidate_selection(content, config, summaries, summary_hash)
}

fn ai_scoring_system_prompt() -> &'static str {
    "You score synced lyric candidates. Return strict JSON only: {\"best_index\":number,\"scores\":[{\"index\":number,\"score\":number,\"reason\":string}]}"
}

fn candidate_summaries(candidates: &[SourceCandidate]) -> Vec<AiCandidateSummary> {
    candidates
        .iter()
        .enumerate()
        .map(|(index, candidate)| AiCandidateSummary {
            index,
            source: candidate.source.cli_name().to_string(),
            title: candidate.result.title.clone(),
            artist: candidate.result.artist.clone(),
            line_count: candidate.quality.line_count,
            timed_line_count: candidate.quality.timed_line_count,
            word_timing_count: candidate.quality.word_timing_count,
            heuristic_score: candidate.quality.score,
            sample_lines: candidate
                .document
                .lines
                .iter()
                .filter_map(|line| {
                    let text = line.text.trim();
                    (!text.is_empty()).then(|| text.chars().take(120).collect::<String>())
                })
                .take(18)
                .collect(),
        })
        .collect()
}

fn candidate_summaries_from_unified(unified: &UnifiedLyric) -> Vec<AiCandidateSummary> {
    unified
        .tracks
        .iter()
        .enumerate()
        .map(|(index, track)| AiCandidateSummary {
            index,
            source: track.source.clone(),
            title: track
                .document
                .meta
                .title
                .clone()
                .unwrap_or_else(|| unified.meta.title.clone().unwrap_or_default()),
            artist: track
                .document
                .meta
                .artist
                .clone()
                .unwrap_or_else(|| unified.meta.artist.clone().unwrap_or_default()),
            line_count: track.quality.line_count,
            timed_line_count: track.quality.timed_line_count,
            word_timing_count: track.quality.word_timing_count,
            heuristic_score: track.quality.score,
            sample_lines: track
                .document
                .lines
                .iter()
                .filter_map(|line| {
                    let text = line.text.trim();
                    (!text.is_empty()).then(|| text.chars().take(120).collect::<String>())
                })
                .take(18)
                .collect(),
        })
        .collect()
}

fn ai_config_hash(config: &ResolvedAiScoringConfig) -> String {
    let value = json!({
        "base_url": config.base_url,
        "model": config.model,
        "prompt_version": AI_PROMPT_VERSION,
        "candidate_summary_version": AI_CANDIDATE_SUMMARY_VERSION,
    });
    hash_json(serde_json::to_string(&value).unwrap_or_default().as_bytes())
}

fn parse_ai_candidate_selection(
    content: &str,
    config: &ResolvedAiScoringConfig,
    summaries: &[AiCandidateSummary],
    summary_hash: String,
) -> Result<Option<AiScoringResult>> {
    let value: serde_json::Value = serde_json::from_str(content.trim())?;
    let best_index = value
        .get("best_index")
        .and_then(|value| value.as_u64())
        .ok_or_else(|| Error::Provider("AI response missing best_index".into()))?
        as usize;
    if best_index >= summaries.len() {
        return Err(Error::Provider(format!(
            "AI response best_index {best_index} is out of range"
        )));
    }
    let response_scores = value
        .get("scores")
        .and_then(|value| value.as_array())
        .cloned()
        .unwrap_or_default();
    let scores = summaries
        .iter()
        .map(|summary| {
            let score_item = response_scores.iter().find(|score| {
                score
                    .get("index")
                    .and_then(|index| index.as_u64())
                    .is_some_and(|index| index as usize == summary.index)
            });
            AiCandidateScore {
                index: summary.index,
                source: summary.source.clone(),
                title: summary.title.clone(),
                artist: summary.artist.clone(),
                heuristic_score: summary.heuristic_score,
                ai_score: score_item
                    .and_then(|item| item.get("score"))
                    .and_then(|score| score.as_f64())
                    .map(|score| score as f32),
                reason: score_item
                    .and_then(|item| item.get("reason"))
                    .and_then(|reason| reason.as_str())
                    .unwrap_or("")
                    .chars()
                    .take(240)
                    .collect(),
            }
        })
        .collect::<Vec<_>>();
    let reason = scores
        .iter()
        .find(|score| score.index == best_index)
        .map(|score| score.reason.as_str())
        .filter(|reason| !reason.is_empty())
        .unwrap_or("selected by AI")
        .to_string();
    Ok(Some(AiScoringResult {
        model: config.model.clone(),
        base_url: config.base_url.clone(),
        prompt_version: AI_PROMPT_VERSION.to_string(),
        config_hash: ai_config_hash(config),
        candidate_summary_version: AI_CANDIDATE_SUMMARY_VERSION.to_string(),
        candidate_summary_hash: summary_hash,
        best_index,
        scores,
        reason,
        created_at: now_unix(),
    }))
}

trait FetchRunOutcome {
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

fn fetch_run_metadata_from_result<T: FetchRunOutcome>(result: &Result<T>) -> FetchRunMetadata {
    result
        .as_ref()
        .map(FetchRunOutcome::fetch_run_metadata)
        .unwrap_or_default()
}

fn provider_warning_count(warnings: &[String]) -> usize {
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

fn is_default_fetch_metadata(metadata: &FetchRunStructuredMetadata) -> bool {
    metadata.provider_count.is_none()
        && metadata.candidate_count.is_none()
        && metadata.provider_warning_count.is_none()
        && metadata.cache_event.is_none()
}

fn classify_fetch_run_result<T: FetchRunOutcome>(
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

fn fetch_run_query_for_result(result: &SearchResult) -> String {
    [result.title.as_str(), result.artist.as_str()]
        .into_iter()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
        .if_empty_then(|| result.id.clone())
}

fn fetch_run_query_for_members(members: &[SearchResult]) -> String {
    members
        .first()
        .map(fetch_run_query_for_result)
        .unwrap_or_else(|| "aggregate members".into())
}

trait EmptyStringExt {
    fn if_empty_then(self, fallback: impl FnOnce() -> String) -> String;
}

impl EmptyStringExt for String {
    fn if_empty_then(self, fallback: impl FnOnce() -> String) -> String {
        if self.is_empty() {
            fallback()
        } else {
            self
        }
    }
}

impl AiScoringResult {
    fn best_score(&self) -> Option<f32> {
        self.scores
            .iter()
            .find(|score| score.index == self.best_index)
            .and_then(|score| score.ai_score)
    }
}

fn hash_json(body: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(body);
    format!("{:x}", hasher.finalize())
}

fn build_unified(
    base: &SourceCandidate,
    candidates: &[SourceCandidate],
    merge_mode: MergeMode,
) -> UnifiedLyric {
    let mut tracks = Vec::new();
    tracks.push(track_from_candidate(
        base,
        LyricTrackKind::Original,
        None,
        &base.document,
    ));

    if let Some(track) = best_translation_track(candidates) {
        tracks.push(track_from_candidate(
            track,
            LyricTrackKind::Translation,
            Some(DEFAULT_TRANSLATION_LANG.into()),
            &translation_document(&track.document),
        ));
    }
    if let Some(track) = best_ruby_track(candidates) {
        if !same_source_result(base, track) {
            tracks.push(track_from_candidate(
                track,
                LyricTrackKind::Ruby,
                None,
                &track.document,
            ));
        }
    }
    if let Some(track) = best_reading_track(candidates) {
        tracks.push(track_from_candidate(
            track,
            LyricTrackKind::Reading,
            Some("ja-Kana".into()),
            &reading_document(&track.document),
        ));
    }
    if let Some(track) = best_romanized_track(candidates) {
        tracks.push(track_from_candidate(
            track,
            LyricTrackKind::Romanized,
            Some("und-Latn".into()),
            &romanized_document(&track.document),
        ));
    }

    let source_refs = tracks
        .iter()
        .map(|track| track.source.clone())
        .collect::<Vec<_>>();
    let cache_refs = candidates
        .iter()
        .map(|candidate| {
            format!(
                "{}:{}:{}",
                candidate.source.cli_name(),
                candidate.result.id,
                candidate.input_format_name()
            )
        })
        .collect::<Vec<_>>();
    let inline_lines = if merge_mode == MergeMode::Inline {
        build_inline_lines(base, &tracks)
    } else {
        Vec::new()
    };

    let enrichment_score = tracks
        .iter()
        .filter(|track| track.kind != LyricTrackKind::Original)
        .count() as f32
        / 3.0;
    let score = UnifiedLyricScore {
        final_score: (base.quality.score + enrichment_score.min(1.0) * 20.0).min(100.0),
        timing_score: base.quality.score.min(100.0),
        completeness_score: completeness_score(&base.document),
        enrichment_score: enrichment_score.min(1.0) * 100.0,
    };

    let annotations = if base.annotations.is_empty() {
        candidates
            .iter()
            .find(|candidate| !candidate.annotations.is_empty())
            .map(|candidate| candidate.annotations.clone())
            .unwrap_or_default()
    } else {
        base.annotations.clone()
    };

    UnifiedLyric {
        schema_version: crate::model::UNIFIED_LYRIC_SCHEMA_VERSION.to_string(),
        meta: merged_meta(base),
        mode: merge_mode.into(),
        tracks,
        inline_lines,
        source_refs,
        score,
        cache_refs,
        warnings: Vec::new(),
        annotations,
    }
}

fn unique_source_names(candidates: &[SourceCandidate]) -> Vec<String> {
    let mut names = Vec::new();
    for candidate in candidates {
        let name = candidate.source.cli_name().to_string();
        if !names.contains(&name) {
            names.push(name);
        }
    }
    names
}

impl SourceCandidate {
    fn input_format_name(&self) -> &'static str {
        match self.input_format {
            crate::decoder::InputFormat::Auto => "auto",
            crate::decoder::InputFormat::AppleMusic => "apple-music",
            crate::decoder::InputFormat::Json => "json",
            crate::decoder::InputFormat::Krc => "krc",
            crate::decoder::InputFormat::Qrc => "qrc",
            crate::decoder::InputFormat::Text => "text",
            crate::decoder::InputFormat::Yrc => "yrc",
            crate::decoder::InputFormat::Lrc => "lrc",
        }
    }
}

fn track_from_candidate(
    candidate: &SourceCandidate,
    kind: LyricTrackKind,
    language: Option<String>,
    document: &LyricDocument,
) -> LyricTrack {
    LyricTrack {
        kind,
        language,
        source: candidate.source.cli_name().to_string(),
        document: document.clone(),
        quality: quality_for_document(document, candidate.source),
    }
}

fn same_source_result(left: &SourceCandidate, right: &SourceCandidate) -> bool {
    left.source == right.source && left.result.id == right.result.id
}

fn best_translation_track(candidates: &[SourceCandidate]) -> Option<&SourceCandidate> {
    candidates
        .iter()
        .filter(|candidate| {
            candidate
                .document
                .lines
                .iter()
                .any(|line| line.translation.is_some())
        })
        .max_by(|left, right| {
            left.quality
                .score
                .partial_cmp(&right.quality.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
}

fn best_ruby_track(candidates: &[SourceCandidate]) -> Option<&SourceCandidate> {
    candidates
        .iter()
        .filter(|candidate| {
            candidate
                .document
                .lines
                .iter()
                .any(|line| !line.ruby.is_empty())
        })
        .max_by(|left, right| {
            left.quality
                .score
                .partial_cmp(&right.quality.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
}

fn best_reading_track(candidates: &[SourceCandidate]) -> Option<&SourceCandidate> {
    candidates
        .iter()
        .filter(|candidate| {
            candidate
                .document
                .lines
                .iter()
                .any(|line| line.reading.is_some())
        })
        .max_by(|left, right| {
            left.quality
                .score
                .partial_cmp(&right.quality.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
}

fn best_romanized_track(candidates: &[SourceCandidate]) -> Option<&SourceCandidate> {
    candidates
        .iter()
        .filter(|candidate| {
            candidate
                .document
                .lines
                .iter()
                .any(|line| line.romanized.is_some())
        })
        .max_by(|left, right| {
            left.quality
                .score
                .partial_cmp(&right.quality.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
}

fn translation_document(document: &LyricDocument) -> LyricDocument {
    field_document(document, |line| line.translation.clone())
}

fn reading_document(document: &LyricDocument) -> LyricDocument {
    field_document(document, |line| line.reading.clone())
}

fn romanized_document(document: &LyricDocument) -> LyricDocument {
    field_document(document, |line| line.romanized.clone())
}

fn field_document(
    document: &LyricDocument,
    field: impl Fn(&LyricLine) -> Option<String>,
) -> LyricDocument {
    let mut out = document.clone();
    out.lines = document
        .lines
        .iter()
        .filter_map(|line| {
            let text = field(line)?;
            if text.trim().is_empty() {
                return None;
            }
            Some(LyricLine {
                start_ms: line.start_ms,
                duration_ms: line.duration_ms,
                text,
                words: Vec::new(),
                ruby: Vec::new(),
                translation: None,
                reading: None,
                romanized: None,
            })
        })
        .collect();
    out
}

fn build_inline_lines(base: &SourceCandidate, tracks: &[LyricTrack]) -> Vec<InlineLyricLine> {
    base.document
        .lines
        .iter()
        .map(|line| {
            let mut inline = InlineLyricLine {
                start_ms: line.start_ms,
                duration_ms: line.duration_ms,
                text: line.text.clone(),
                translation: line.translation.clone(),
                reading: line.reading.clone(),
                romanized: line.romanized.clone(),
                ruby: line.ruby.clone(),
                source_refs: vec![base.source.cli_name().to_string()],
            };

            for track in tracks {
                match track.kind {
                    LyricTrackKind::Reading if inline.reading.is_none() => {
                        inline.reading = nearest_line_text(&track.document, line.start_ms);
                    }
                    LyricTrackKind::Romanized if inline.romanized.is_none() => {
                        inline.romanized = nearest_line_text(&track.document, line.start_ms);
                    }
                    LyricTrackKind::Ruby if inline.ruby.is_empty() => {
                        if let Some(ruby_line) = nearest_line(&track.document, line.start_ms) {
                            inline.ruby = ruby_line.ruby.clone();
                        }
                    }
                    LyricTrackKind::Translation if inline.translation.is_none() => {
                        inline.translation = nearest_line_text(&track.document, line.start_ms);
                    }
                    _ => {}
                }
                if !inline.source_refs.contains(&track.source) {
                    inline.source_refs.push(track.source.clone());
                }
            }

            inline
        })
        .collect()
}

fn nearest_line_text(document: &LyricDocument, start_ms: u32) -> Option<String> {
    nearest_line(document, start_ms).map(|line| line.text.clone())
}

fn nearest_line(document: &LyricDocument, start_ms: u32) -> Option<&LyricLine> {
    document
        .lines
        .iter()
        .min_by_key(|line| line.start_ms.abs_diff(start_ms))
        .filter(|line| line.start_ms.abs_diff(start_ms) <= 1_500)
}

fn merged_meta(base: &SourceCandidate) -> LyricMeta {
    let mut meta = base.document.meta.clone();
    if meta.title.is_none() && !base.result.title.trim().is_empty() {
        meta.title = Some(base.result.title.clone());
    }
    if meta.artist.is_none() && !base.result.artist.trim().is_empty() {
        meta.artist = Some(base.result.artist.clone());
    }
    if meta.album.is_none() {
        meta.album = base.result.album.clone();
    }
    meta.source = Some(base.source.cli_name().to_string());
    meta
}

fn quality_for_document(document: &LyricDocument, source: Source) -> LyricTrackQuality {
    let line_count = document.lines.len();
    let timed_line_count = document
        .lines
        .iter()
        .filter(|line| line.start_ms > 0 || line.duration_ms.is_some())
        .count();
    let word_timing_count = document
        .lines
        .iter()
        .map(|line| line.words.len())
        .sum::<usize>();
    let timed_ratio = ratio(timed_line_count, line_count);
    let word_ratio = ratio(
        word_timing_count,
        document
            .lines
            .iter()
            .map(|line| line.text.chars().count())
            .sum(),
    );
    let source_weight = source_weight(source);
    let score = (source_weight
        + timed_ratio * 25.0
        + word_ratio.min(1.0) * 30.0
        + completeness_score(document) * 0.2)
        .min(100.0);

    LyricTrackQuality {
        score,
        line_count,
        timed_line_count,
        word_timing_count,
    }
}

fn ratio(part: usize, total: usize) -> f32 {
    if total == 0 {
        0.0
    } else {
        part as f32 / total as f32
    }
}

fn completeness_score(document: &LyricDocument) -> f32 {
    let line_count = document
        .lines
        .iter()
        .filter(|line| !line.text.trim().is_empty())
        .count();
    (line_count as f32 * 2.0).min(100.0)
}

fn source_weight(source: Source) -> f32 {
    match source {
        Source::SpotifyLyrics
        | Source::AppleMusic
        | Source::Netease
        | Source::Qq
        | Source::Kugou => 45.0,
        Source::Lrclib | Source::Migu | Source::Musixmatch => 38.0,
        Source::Utaten | Source::UtaNet | Source::LyricalNonsense => 32.0,
        Source::Joysound | Source::PetitLyrics | Source::Kkbox | Source::LineMusic => 28.0,
        _ => 22.0,
    }
}

fn decode_fetched(fetched: FetchedLyric) -> Result<LyricDocument> {
    match fetched.document {
        Some(document) => Ok(document),
        None => decode_bytes(&fetched.raw, fetched.input_format),
    }
}

fn decode_source_raw_bytes(bytes: &[u8], input_format: InputFormat) -> Result<Vec<u8>> {
    match input_format {
        InputFormat::Qrc => Ok(crate::decoder::qrc::decode_raw_lyric_content(bytes)?.into_bytes()),
        format => decode_raw_bytes(bytes, format),
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::LyricRubySpan;
    use serde_json::json;

    #[test]
    fn golden_ai_score_fixture_tracks_quality_changes() {
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../tests/fixtures/ai_score_history/candidate_regression.json"
        ))
        .unwrap();
        let summaries: Vec<AiCandidateSummary> =
            serde_json::from_value(fixture["candidates"].clone()).unwrap();
        let expected = &fixture["expected"];
        let heuristic_best_index = summaries
            .iter()
            .max_by(|left, right| {
                left.heuristic_score
                    .partial_cmp(&right.heuristic_score)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|summary| summary.index)
            .unwrap();
        assert_eq!(
            heuristic_best_index,
            expected["heuristic_best_index"].as_u64().unwrap() as usize
        );

        let config = ResolvedAiScoringConfig {
            base_url: "https://example.invalid/v1".into(),
            api_key: "fixture-secret".into(),
            model: "fixture-model".into(),
        };
        let summary_hash = hash_json(serde_json::to_string(&summaries).unwrap().as_bytes());
        let v1 = parse_ai_candidate_selection(
            &fixture["ai_response_v1"].to_string(),
            &config,
            &summaries,
            summary_hash.clone(),
        )
        .unwrap()
        .unwrap();
        let v2 = parse_ai_candidate_selection(
            &fixture["ai_response_v2"].to_string(),
            &config,
            &summaries,
            summary_hash,
        )
        .unwrap()
        .unwrap();

        assert_eq!(
            v1.best_index,
            expected["ai_best_index_v1"].as_u64().unwrap() as usize
        );
        assert_eq!(
            v2.best_index,
            expected["ai_best_index_v2"].as_u64().unwrap() as usize
        );
        assert_eq!(v1.prompt_version, AI_PROMPT_VERSION);
        assert_eq!(v1.candidate_summary_version, AI_CANDIDATE_SUMMARY_VERSION);
        assert_ne!(v1.best_index, v2.best_index);
    }

    #[test]
    fn parses_ttl_units() {
        assert_eq!(parse_ttl("7d").unwrap(), Duration::from_secs(604_800));
        assert_eq!(parse_ttl("2h").unwrap(), Duration::from_secs(7_200));
        assert_eq!(parse_ttl("30").unwrap(), Duration::from_secs(30));
    }

    #[test]
    fn inline_merge_keeps_base_text_and_adds_enrichment() {
        let base = SourceCandidate {
            source: Source::Lrclib,
            result: SearchResult {
                source: Source::Lrclib,
                id: "1".into(),
                title: "Song".into(),
                artist: "Artist".into(),
                album: None,
                duration_ms: None,
                extra: json!({}),
            },
            input_format: crate::decoder::InputFormat::Lrc,
            document: LyricDocument {
                meta: LyricMeta::default(),
                lines: vec![LyricLine {
                    start_ms: 1_000,
                    duration_ms: Some(1_000),
                    text: "歌".into(),
                    words: Vec::new(),
                    ruby: Vec::new(),
                    translation: None,
                    reading: None,
                    romanized: None,
                }],
            },
            quality: LyricTrackQuality::default(),
            annotations: Vec::new(),
        };
        let ruby = SourceCandidate {
            source: Source::Utaten,
            result: SearchResult {
                source: Source::Utaten,
                id: "ruby".into(),
                title: "Song".into(),
                artist: "Artist".into(),
                album: None,
                duration_ms: None,
                extra: json!({}),
            },
            document: LyricDocument {
                lines: vec![LyricLine {
                    start_ms: 1_020,
                    text: "歌".into(),
                    ruby: vec![LyricRubySpan {
                        start_char: 0,
                        end_char: 1,
                        text: "歌".into(),
                        reading: "うた".into(),
                    }],
                    ..Default::default()
                }],
                ..Default::default()
            },
            ..base.clone()
        };

        let unified = build_unified(&base, &[base.clone(), ruby], MergeMode::Inline);
        assert_eq!(unified.inline_lines[0].text, "歌");
        assert_eq!(unified.inline_lines[0].ruby[0].reading, "うた");
    }

    #[test]
    fn source_specific_qrc_raw_returns_decoded_lyric_content() {
        use base64::Engine;

        let wrapped =
            base64::engine::general_purpose::STANDARD.encode("[1000,500](1000,500,0)Hi\n");
        let xml = format!(
            r#"<?xml version="1.0"?><QrcInfos><LyricInfo><Lyric_1 LyricContent="{wrapped}"/></LyricInfo></QrcInfos>"#
        );
        let raw =
            decode_source_raw_bytes(xml.as_bytes(), crate::decoder::InputFormat::Qrc).unwrap();

        assert_eq!(
            String::from_utf8(raw).unwrap(),
            "[1000,500](1000,500,0)Hi\n"
        );
    }

    #[tokio::test]
    async fn source_specific_search_returns_all_candidates_when_top_is_omitted() {
        let path = service_offline_db_fixture();
        let context = offline_service_context(path.clone());

        let response = context
            .search_source_specific(
                Source::OfflineDb,
                "Artist",
                Some(Duration::from_secs(60)),
                false,
            )
            .await
            .unwrap();

        assert_eq!(response.results.len(), 2);
        assert_eq!(response.results[0].title, "First Song");
        assert_eq!(response.results[1].title, "Second Song");

        let _ = std::fs::remove_file(path);
    }

    #[tokio::test]
    async fn source_specific_top_fetches_multiple_candidates() {
        let path = service_offline_db_fixture();
        let context = offline_service_context(path.clone());

        let result = context
            .fetch_source_specific(
                Source::OfflineDb,
                "Artist",
                SpecificFetchFormat::Raw,
                2,
                Some(Duration::from_secs(60)),
                false,
                false,
            )
            .await
            .unwrap();

        match result {
            SpecificFetchResult::RawMany {
                results, warnings, ..
            } => {
                assert!(warnings.is_empty());
                assert_eq!(results.len(), 2);
                assert!(String::from_utf8(results[0].raw.clone())
                    .unwrap()
                    .contains("First"));
                assert!(String::from_utf8(results[1].raw.clone())
                    .unwrap()
                    .contains("Second"));
            }
            _ => panic!("expected RawMany"),
        }

        let _ = std::fs::remove_file(path);
    }

    #[tokio::test]
    async fn multi_source_search_and_fetch_result_use_selected_candidate() {
        let path = service_offline_db_fixture();
        let context = offline_service_context(path.clone());

        let response = context
            .search_sources(SourceSearchRequest {
                query: "Artist".into(),
                sources: Some(vec![Source::OfflineDb]),
                limit: 10,
                force: false,
                ttl_seconds: Some(60),
            })
            .await
            .unwrap();
        assert_eq!(response.results.len(), 2);

        let second = response.results[1].clone();
        let fetched = context
            .fetch_source_result(
                second,
                SpecificFetchFormat::Raw,
                Some(Duration::from_secs(60)),
                false,
                false,
            )
            .await
            .unwrap();
        match fetched {
            SpecificFetchResult::Raw { raw, result, .. } => {
                assert_eq!(result.id, "two");
                assert!(String::from_utf8(raw).unwrap().contains("Second"));
            }
            _ => panic!("expected Raw"),
        }

        let _ = std::fs::remove_file(path);
    }

    fn offline_service_context(path: std::path::PathBuf) -> ServiceContext {
        ServiceContext {
            offline_db: Some(path),
            provider_options: ProviderOptions {
                allow_experimental: true,
            },
            ..Default::default()
        }
    }

    fn service_offline_db_fixture() -> std::path::PathBuf {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "rosettrism-service-offline-{unique}-{}.sqlite",
            std::process::id()
        ));
        let connection = rusqlite::Connection::open(&path).unwrap();
        connection
            .execute_batch(
                r#"
                CREATE TABLE lyrics (
                    id TEXT PRIMARY KEY,
                    title TEXT NOT NULL,
                    artist TEXT NOT NULL,
                    album TEXT,
                    duration_ms INTEGER,
                    source TEXT,
                    format TEXT,
                    text TEXT NOT NULL,
                    reading TEXT,
                    romanized TEXT,
                    metadata_json TEXT
                );
                "#,
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO lyrics VALUES (?1, ?2, ?3, NULL, ?4, ?5, ?6, ?7, NULL, NULL, NULL)",
                rusqlite::params![
                    "one",
                    "First Song",
                    "Artist",
                    120_000_i64,
                    "fixture",
                    "lrc",
                    "[00:01.00]First\n"
                ],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO lyrics VALUES (?1, ?2, ?3, NULL, ?4, ?5, ?6, ?7, NULL, NULL, NULL)",
                rusqlite::params![
                    "two",
                    "Second Song",
                    "Artist",
                    130_000_i64,
                    "fixture",
                    "lrc",
                    "[00:01.00]Second\n"
                ],
            )
            .unwrap();
        drop(connection);
        path
    }
}
