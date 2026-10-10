use super::*;
use crate::decoder::{decode_bytes, decode_raw_bytes, InputFormat};
use crate::model::{
    Annotation, InlineLyricLine, LyricDocument, LyricLine, LyricMeta, LyricTrack, LyricTrackKind,
    LyricTrackQuality, UnifiedLyric, UnifiedLyricScore,
};
use crate::provider::{FetchedLyric, Source};
use crate::Result;

pub(crate) fn serialize_raw_text<S>(
    raw: &[u8],
    serializer: S,
) -> std::result::Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    serializer.serialize_str(&String::from_utf8_lossy(raw))
}

#[derive(Debug, Clone)]
pub(crate) struct SourceCandidate {
    pub(crate) source: Source,
    pub(crate) result: SearchResult,
    pub(crate) document: LyricDocument,
    pub(crate) quality: LyricTrackQuality,
    pub(crate) annotations: Vec<Annotation>,
}
pub(crate) fn compare_candidate_quality(
    left: &SourceCandidate,
    right: &SourceCandidate,
) -> std::cmp::Ordering {
    right
        .quality
        .score
        .partial_cmp(&left.quality.score)
        .unwrap_or(std::cmp::Ordering::Equal)
}
pub(crate) fn build_unified(
    base: &SourceCandidate,
    candidates: &[SourceCandidate],
    merge_mode: MergeMode,
) -> Result<UnifiedLyric> {
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
        .map(|candidate| fetch_cache_key(candidate.source, &candidate.result))
        .collect::<Result<Vec<_>>>()?;
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

    Ok(UnifiedLyric {
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
    })
}

pub(crate) fn unique_source_names(candidates: &[SourceCandidate]) -> Vec<String> {
    let mut names = Vec::new();
    for candidate in candidates {
        let name = candidate.source.cli_name().to_string();
        if !names.contains(&name) {
            names.push(name);
        }
    }
    names
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

pub(crate) fn quality_for_document(document: &LyricDocument, source: Source) -> LyricTrackQuality {
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

pub(crate) fn decode_fetched(fetched: FetchedLyric) -> Result<LyricDocument> {
    match fetched.document {
        Some(document) => Ok(document),
        None => decode_bytes(&fetched.raw, fetched.input_format),
    }
}

pub(crate) fn decode_source_raw_bytes(bytes: &[u8], input_format: InputFormat) -> Result<Vec<u8>> {
    match input_format {
        InputFormat::Qrc => Ok(crate::decoder::qrc::decode_raw_lyric_content(bytes)?.into_bytes()),
        format => decode_raw_bytes(bytes, format),
    }
}
