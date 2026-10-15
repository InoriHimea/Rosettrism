use super::*;

pub(super) const DEFAULT_SOURCE_SEARCH_LIMIT: usize = 100;
pub(super) const DEFAULT_AGGREGATE_SEARCH_LIMIT: usize = 10;

#[derive(Debug, Deserialize)]
pub(super) struct ApiFetchRequest {
    pub(super) query: String,
    #[serde(default)]
    pub(super) source: Option<String>,
    #[serde(default)]
    pub(super) format: Option<SpecificFetchFormat>,
    #[serde(default)]
    pub(super) merge_mode: Option<MergeMode>,
    #[serde(default)]
    pub(super) top: Option<usize>,
    #[serde(default)]
    pub(super) needs: Option<Vec<LyricNeed>>,
    #[serde(default)]
    pub(super) translation_lang: Option<String>,
    #[serde(default)]
    pub(super) sources: Option<Vec<String>>,
    #[serde(default)]
    pub(super) force: Option<bool>,
    #[serde(default)]
    pub(super) enrich: bool,
    #[serde(default)]
    pub(super) ttl_seconds: Option<u64>,
    #[serde(default)]
    pub(super) ai_scoring: Option<AiScoringConfig>,
}

#[derive(Debug, Deserialize)]
pub(super) struct ApiSearchRequest {
    #[serde(default)]
    pub(super) query: Option<String>,
    #[serde(default)]
    pub(super) title: Option<String>,
    #[serde(default)]
    pub(super) artist: Option<String>,
    #[serde(default)]
    pub(super) id: Option<String>,
    #[serde(default)]
    pub(super) source: Option<String>,
    #[serde(default)]
    pub(super) sources: Option<Vec<String>>,
    #[serde(default)]
    pub(super) top: Option<usize>,
    #[serde(default)]
    pub(super) limit: Option<usize>,
    #[serde(default)]
    pub(super) merge_mode: Option<MergeMode>,
    #[serde(default)]
    pub(super) force: Option<bool>,
    #[serde(default)]
    pub(super) ttl_seconds: Option<u64>,
}

impl ApiSearchRequest {
    pub(super) fn search_intent(&self) -> ApiResult<SearchIntent> {
        if let Some(id) = self
            .id
            .as_deref()
            .map(str::trim)
            .filter(|id| !id.is_empty())
        {
            return Ok(SearchIntent::Id(id.to_string()));
        }

        let title = self.title.as_deref().map(str::trim).unwrap_or_default();
        let artist = self.artist.as_deref().map(str::trim).unwrap_or_default();
        if !title.is_empty() || !artist.is_empty() {
            return Ok(SearchIntent::Fields {
                title: (!title.is_empty()).then(|| title.to_string()),
                artist: (!artist.is_empty()).then(|| artist.to_string()),
            });
        }

        let query = self.query.as_deref().map(str::trim).unwrap_or_default();
        if !query.is_empty() {
            return Ok(SearchIntent::Keyword(query.to_string()));
        }

        Err(ApiError::bad_request(
            "query, title, artist, or id must not be empty",
        ))
    }

    pub(super) fn search_limit(&self, default_limit: usize) -> usize {
        self.top
            .or(self.limit)
            .filter(|limit| *limit > 0)
            .unwrap_or(default_limit)
            .clamp(1, default_limit)
    }
}

#[derive(Debug, Clone)]
pub(super) enum SearchIntent {
    Id(String),
    Fields {
        title: Option<String>,
        artist: Option<String>,
    },
    Keyword(String),
}

impl SearchIntent {
    pub(super) fn provider_query(&self) -> String {
        match self {
            SearchIntent::Id(id) => id.clone(),
            SearchIntent::Fields { title, artist } => [title.as_deref(), artist.as_deref()]
                .into_iter()
                .flatten()
                .filter(|part| !part.trim().is_empty())
                .collect::<Vec<_>>()
                .join(" "),
            SearchIntent::Keyword(query) => query.clone(),
        }
    }
}

pub(super) fn filter_results(
    results: Vec<SearchResult>,
    intent: &SearchIntent,
) -> Vec<SearchResult> {
    results
        .into_iter()
        .filter(|result| result_matches_intent(result, intent))
        .collect()
}

pub(super) fn result_matches_intent(result: &SearchResult, intent: &SearchIntent) -> bool {
    match intent {
        SearchIntent::Id(id) => result_matches_id(result, id),
        SearchIntent::Fields { title, artist } => {
            title
                .as_deref()
                .is_none_or(|title| title_field_matches(&result.title, title))
                && artist
                    .as_deref()
                    .is_none_or(|artist| field_matches(&result.artist, artist))
        }
        SearchIntent::Keyword(query) => keyword_matches(result, query),
    }
}

pub(super) fn add_direct_id_fallback(result: &mut SourceSearchResult, intent: &SearchIntent) {
    let SearchIntent::Id(id) = intent else {
        return;
    };
    if result.results.is_empty() {
        result.results.push(direct_id_result(result.source, id));
    }
}

pub(super) fn direct_id_result(source: Source, id: &str) -> SearchResult {
    let id = id.trim().to_string();
    SearchResult {
        source,
        id: id.clone(),
        title: id.clone(),
        artist: String::new(),
        album: None,
        duration_ms: None,
        extra: direct_id_extra(source, &id),
    }
}

pub(super) fn direct_id_extra(source: Source, id: &str) -> Value {
    match source {
        Source::Qq => match id.parse::<u64>() {
            Ok(song_id) => json!({
                "direct_id": true,
                "songid": song_id
            }),
            Err(_) => json!({
                "direct_id": true,
                "songmid": id
            }),
        },
        Source::Kugou => json!({
            "direct_id": true,
            "provider_result": "song",
            "fmt": "krc",
            "album_audio_id": id.parse::<u64>().ok(),
            "hash": looks_like_kugou_hash(id).then_some(id)
        }),
        _ => json!({
            "direct_id": true
        }),
    }
}

pub(super) fn looks_like_kugou_hash(id: &str) -> bool {
    id.len() == 32 && id.chars().all(|ch| ch.is_ascii_hexdigit())
}

pub(super) fn result_matches_id(result: &SearchResult, id: &str) -> bool {
    let needle = normalized_text(id);
    if needle.is_empty() {
        return false;
    }

    normalized_text(&result.id) == needle || value_contains_exact_id(&result.extra, &needle)
}

pub(super) fn value_contains_exact_id(value: &Value, needle: &str) -> bool {
    match value {
        Value::String(value) => normalized_text(value) == needle,
        Value::Number(value) => normalized_text(&value.to_string()) == needle,
        Value::Array(values) => values
            .iter()
            .any(|value| value_contains_exact_id(value, needle)),
        Value::Object(values) => values
            .values()
            .any(|value| value_contains_exact_id(value, needle)),
        _ => false,
    }
}

pub(super) fn field_matches(value: &str, filter: &str) -> bool {
    let value = normalized_text(value);
    let filter = normalized_text(filter);
    if value.is_empty() || filter.is_empty() {
        return false;
    }
    if value == filter || value.contains(&filter) {
        return true;
    }

    let reversed = filter.chars().rev().collect::<String>();
    if filter.chars().count() <= 3 && value == reversed {
        return true;
    }

    common_char_ratio(&value, &filter) >= 0.9
}

pub(super) fn title_field_matches(value: &str, filter: &str) -> bool {
    let value = normalized_text(value);
    let filter = normalized_text(filter);
    if value.is_empty() || filter.is_empty() {
        return false;
    }
    if value == filter || value.starts_with(&filter) {
        return true;
    }

    let reversed = filter.chars().rev().collect::<String>();
    filter.chars().count() <= 3 && value == reversed
}

pub(super) fn keyword_matches(result: &SearchResult, query: &str) -> bool {
    let haystack = normalized_text(
        &keyword_haystack_values(result)
            .into_iter()
            .filter(|value| !value.is_empty())
            .collect::<Vec<_>>()
            .join(" "),
    );
    let term_groups = query
        .split_whitespace()
        .map(keyword_term_alternatives)
        .filter(|terms| !terms.is_empty())
        .collect::<Vec<_>>();

    !term_groups.is_empty()
        && term_groups.iter().all(|terms| {
            terms.iter().any(|term| {
                haystack.contains(term)
                    || (term.chars().count() <= 3
                        && haystack.contains(&term.chars().rev().collect::<String>()))
            })
        })
}

pub(super) fn keyword_haystack_values(result: &SearchResult) -> Vec<String> {
    let mut values = vec![
        result.title.clone(),
        result.artist.clone(),
        result.album.clone().unwrap_or_default(),
        result.id.clone(),
    ];
    collect_extra_keyword_values(&result.extra, &mut values);
    values
}

pub(super) fn collect_extra_keyword_values(extra: &Value, values: &mut Vec<String>) {
    const DIRECT_KEYS: &[&str] = &[
        "url",
        "songmid",
        "songid",
        "song_id",
        "lyrics_id",
        "lrclib_id",
        "spotify_track_id",
        "album_audio_id",
        "hash",
    ];
    let Value::Object(object) = extra else {
        return;
    };
    for key in DIRECT_KEYS {
        if let Some(value) = object.get(*key) {
            match value {
                Value::String(value) => values.push(value.clone()),
                Value::Number(value) => values.push(value.to_string()),
                _ => {}
            }
        }
    }
}

pub(super) fn keyword_term_alternatives(term: &str) -> Vec<String> {
    let trimmed = term.trim();
    let mut alternatives = Vec::new();
    push_normalized_keyword(&mut alternatives, trimmed);
    if let Some(id) = trimmed.strip_prefix("id:") {
        push_normalized_keyword(&mut alternatives, id);
    }
    if let Some(id) = direct_query_tail_id(trimmed) {
        push_normalized_keyword(&mut alternatives, &id);
    }
    alternatives
}

pub(super) fn push_normalized_keyword(alternatives: &mut Vec<String>, value: &str) {
    let normalized = normalized_text(value);
    if !normalized.is_empty() && !alternatives.contains(&normalized) {
        alternatives.push(normalized);
    }
}

pub(super) fn direct_query_tail_id(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if !trimmed.contains("://") && !trimmed.starts_with("spotify:") {
        return None;
    }

    if let Some((_, query)) = trimmed.split_once('?') {
        for part in query.split('&') {
            let (key, value) = part.split_once('=').unwrap_or((part, ""));
            if matches!(key, "i" | "id" | "track" | "track_id" | "surl") && !value.is_empty() {
                return Some(value.to_string());
            }
        }
    }

    let without_fragment = trimmed.split('#').next().unwrap_or(trimmed);
    let without_query = without_fragment
        .split('?')
        .next()
        .unwrap_or(without_fragment);
    without_query
        .trim_end_matches('/')
        .rsplit(['/', ':'])
        .find(|part| !part.trim().is_empty())
        .map(ToOwned::to_owned)
}

pub(super) fn common_char_ratio(value: &str, filter: &str) -> f32 {
    let mut chars = value.chars().collect::<Vec<_>>();
    let mut matched = 0usize;
    for ch in filter.chars() {
        if let Some(index) = chars.iter().position(|candidate| *candidate == ch) {
            chars.remove(index);
            matched += 1;
        }
    }

    let total = filter.chars().count();
    if total == 0 {
        0.0
    } else {
        matched as f32 / total as f32
    }
}

pub(super) fn normalized_text(value: &str) -> String {
    value
        .chars()
        .flat_map(char::to_lowercase)
        .filter(|ch| !ch.is_whitespace() && !ch.is_ascii_punctuation() && !is_cjk_punctuation(*ch))
        .collect()
}

pub(super) fn is_cjk_punctuation(ch: char) -> bool {
    matches!(
        ch,
        '，' | '。'
            | '、'
            | '：'
            | '；'
            | '！'
            | '？'
            | '（'
            | '）'
            | '《'
            | '》'
            | '「'
            | '」'
            | '『'
            | '』'
            | '【'
            | '】'
            | '－'
            | '—'
            | '～'
    )
}

pub(super) fn aggregate_search_results(
    groups: &[SourceSearchResult],
    merge_mode: MergeMode,
    limit: usize,
) -> Vec<SearchResult> {
    let mut aggregate_groups: Vec<AggregateSearchGroup> = Vec::new();
    for result in groups
        .iter()
        .flat_map(|group| group.results.iter().cloned())
    {
        if let Some(group) = aggregate_groups
            .iter_mut()
            .find(|group| group.can_include(&result))
        {
            group.push(result);
        } else {
            aggregate_groups.push(AggregateSearchGroup::new(result));
        }
    }

    aggregate_groups.sort_by(|left, right| {
        right
            .members
            .len()
            .cmp(&left.members.len())
            .then_with(|| left.order.cmp(&right.order))
    });

    aggregate_groups
        .into_iter()
        .take(limit)
        .enumerate()
        .map(|(index, group)| group.into_result(index, merge_mode))
        .collect()
}

pub(super) struct AggregateSearchGroup {
    pub(super) title_key: String,
    pub(super) artist_key: String,
    pub(super) duration_ms: Option<u32>,
    pub(super) order: usize,
    pub(super) members: Vec<SearchResult>,
}

impl AggregateSearchGroup {
    pub(super) fn new(result: SearchResult) -> Self {
        static NEXT_ORDER: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        Self {
            title_key: normalized_text(&result.title),
            artist_key: normalized_text(&result.artist),
            duration_ms: result.duration_ms,
            order: NEXT_ORDER.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            members: vec![result],
        }
    }

    pub(super) fn can_include(&self, result: &SearchResult) -> bool {
        self.title_key == normalized_text(&result.title)
            && self.artist_key == normalized_text(&result.artist)
            && duration_close(self.duration_ms, result.duration_ms)
    }

    pub(super) fn push(&mut self, result: SearchResult) {
        if self
            .members
            .iter()
            .any(|member| member.source == result.source)
        {
            return;
        }
        self.members.push(result);
    }

    pub(super) fn into_result(self, index: usize, merge_mode: MergeMode) -> SearchResult {
        let base = self
            .members
            .iter()
            .max_by_key(|member| source_rank(member.source))
            .cloned()
            .unwrap_or_else(|| self.members[0].clone());
        let sources = unique_result_sources(&self.members);
        let display_source = format!("聚合({})", sources.join("+"));
        SearchResult {
            source: base.source,
            id: format!("aggregate:{}", index + 1),
            title: base.title,
            artist: base.artist,
            album: base.album,
            duration_ms: base.duration_ms,
            extra: json!({
                "result_kind": "aggregate",
                "display_source": display_source,
                "aggregate_sources": sources,
                "aggregate_members": self.members,
                "merge_mode": merge_mode,
            }),
        }
    }
}

pub(super) fn duration_close(left: Option<u32>, right: Option<u32>) -> bool {
    match (left, right) {
        (Some(left), Some(right)) => left.abs_diff(right) <= 15_000,
        _ => true,
    }
}

pub(super) fn source_rank(source: Source) -> u8 {
    match source {
        Source::Kugou => 7,
        Source::Qq => 6,
        Source::Netease => 5,
        Source::Lrclib => 4,
        Source::Migu => 3,
        _ => 1,
    }
}

pub(super) fn unique_result_sources(results: &[SearchResult]) -> Vec<String> {
    let mut sources = Vec::new();
    for result in results {
        if !sources.contains(&result.source) {
            sources.push(result.source);
        }
    }
    sources.sort_by(|left, right| {
        source_rank(*right)
            .cmp(&source_rank(*left))
            .then_with(|| left.cli_name().cmp(right.cli_name()))
    });
    sources
        .into_iter()
        .map(|source| source.cli_name().to_string())
        .collect()
}

pub(super) fn aggregate_members(result: &SearchResult) -> ApiResult<Option<Vec<SearchResult>>> {
    if result.extra.get("result_kind").and_then(Value::as_str) != Some("aggregate") {
        return Ok(None);
    }

    let members = result
        .extra
        .get("aggregate_members")
        .cloned()
        .ok_or_else(|| ApiError::bad_request("aggregate result is missing members"))?;
    serde_json::from_value::<Vec<SearchResult>>(members)
        .map(Some)
        .map_err(|err| ApiError::bad_request(format!("invalid aggregate members: {err}")))
}

pub(super) fn aggregate_merge_mode(result: &SearchResult) -> Option<MergeMode> {
    serde_json::from_value(result.extra.get("merge_mode")?.clone()).ok()
}

pub(super) fn search_response_body<T: Serialize>(value: &T) -> ApiResult<Vec<u8>> {
    let mut value = serde_json::to_value(value)?;
    inject_display_source(&mut value);
    Ok(serde_json::to_vec_pretty(&value)?)
}

pub(super) fn inject_display_source(value: &mut Value) {
    match value {
        Value::Object(object) => {
            if object.contains_key("title") && object.contains_key("source") {
                let display_source = object
                    .get("extra")
                    .and_then(|extra| extra.get("display_source"))
                    .and_then(Value::as_str)
                    .map(ToOwned::to_owned)
                    .or_else(|| {
                        object
                            .get("source")
                            .and_then(Value::as_str)
                            .map(ToOwned::to_owned)
                    });
                if let Some(display_source) = display_source {
                    object.insert("display_source".into(), Value::String(display_source));
                }
            }
            for value in object.values_mut() {
                inject_display_source(value);
            }
        }
        Value::Array(values) => {
            for value in values {
                inject_display_source(value);
            }
        }
        _ => {}
    }
}
