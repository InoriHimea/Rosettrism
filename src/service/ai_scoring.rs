use std::time::Duration;

use reqwest::header::{AUTHORIZATION, CONTENT_TYPE};
use serde_json::json;
use sha2::{Digest, Sha256};

use super::*;
use crate::cache::now_unix;
use crate::{Error, Result};

const DEFAULT_OPENAI_MODEL: &str = "gpt-4o-mini";
const AI_SCORING_TIMEOUT: Duration = Duration::from_secs(30);
pub(crate) const AI_PROMPT_VERSION: &str = "ai-score-prompt-v1";
pub(crate) const AI_CANDIDATE_SUMMARY_VERSION: &str = "candidate-summary-v1";

#[derive(Debug, Clone)]
pub(crate) struct ResolvedAiScoringConfig {
    pub(crate) base_url: String,
    pub(crate) api_key: String,
    pub(crate) model: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct AiCandidateSummary {
    pub(crate) index: usize,
    pub(crate) source: String,
    pub(crate) title: String,
    pub(crate) artist: String,
    pub(crate) line_count: usize,
    pub(crate) timed_line_count: usize,
    pub(crate) word_timing_count: usize,
    pub(crate) heuristic_score: f32,
    pub(crate) sample_lines: Vec<String>,
}

pub(crate) fn resolve_ai_scoring_config(
    config: Option<&AiScoringConfig>,
) -> Option<ResolvedAiScoringConfig> {
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

fn text_preview(value: &str, max_chars: usize) -> String {
    if value.chars().count() <= max_chars {
        return value.to_string();
    }
    let mut preview = value.chars().take(max_chars).collect::<String>();
    preview.push_str("...");
    preview
}

pub(crate) async fn score_ai_candidate_summaries(
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
    let client = reqwest::Client::builder()
        .timeout(AI_SCORING_TIMEOUT)
        .build()?;
    let response = client
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
            "OpenAI compatible endpoint returned {status}: {}",
            text_preview(&body, 1_000)
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

pub(crate) fn candidate_summaries(candidates: &[SourceCandidate]) -> Vec<AiCandidateSummary> {
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

pub(crate) fn candidate_summaries_from_unified(unified: &UnifiedLyric) -> Vec<AiCandidateSummary> {
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

pub(crate) fn parse_ai_candidate_selection(
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

pub(crate) trait EmptyStringExt {
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
    pub(crate) fn best_score(&self) -> Option<f32> {
        self.scores
            .iter()
            .find(|score| score.index == self.best_index)
            .and_then(|score| score.ai_score)
    }
}

pub(crate) fn hash_json(body: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(body);
    format!("{:x}", hasher.finalize())
}
