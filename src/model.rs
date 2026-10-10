use serde::{Deserialize, Serialize};

pub const UNIFIED_LYRIC_SCHEMA_VERSION: &str = "1.0";

fn default_unified_lyric_schema_version() -> String {
    UNIFIED_LYRIC_SCHEMA_VERSION.to_string()
}

/// 标注类型枚举
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AnnotationType {
    Stress,         // 重音，符号: `
    Breath,         // 换气，符号: ^
    LongTone,       // 长音，符号: _
    PortamentoUp,   // 上滑音，符号: ↑
    PortamentoDown, // 下滑音，符号: ↓
}

/// 单个助唱标注
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Annotation {
    pub annotation_type: AnnotationType,
    pub start_ms: u32,
    pub duration_ms: u32,
    pub text: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct LyricMeta {
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub by: Option<String>,
    pub offset_ms: Option<i64>,
    pub source: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct LyricWord {
    pub offset_ms: u32,
    pub duration_ms: u32,
    pub text: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct LyricRubySpan {
    pub start_char: u32,
    pub end_char: u32,
    pub text: String,
    pub reading: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct LyricLine {
    pub start_ms: u32,
    pub duration_ms: Option<u32>,
    pub text: String,
    pub words: Vec<LyricWord>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ruby: Vec<LyricRubySpan>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub translation: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reading: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub romanized: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct LyricDocument {
    pub meta: LyricMeta,
    pub lines: Vec<LyricLine>,
}

impl LyricDocument {
    pub fn sort_and_fill_durations(&mut self) {
        self.lines.sort_by_key(|line| line.start_ms);

        for index in 0..self.lines.len() {
            if self.lines[index].duration_ms.is_none() {
                self.lines[index].duration_ms = self
                    .lines
                    .get(index + 1)
                    .and_then(|next| next.start_ms.checked_sub(self.lines[index].start_ms));
            }
        }
    }
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LyricTrackKind {
    #[default]
    Original,
    Translation,
    Reading,
    Romanized,
    Ruby,
    PlainFallback,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct LyricTrackQuality {
    pub score: f32,
    pub line_count: usize,
    pub timed_line_count: usize,
    pub word_timing_count: usize,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct LyricTrack {
    pub kind: LyricTrackKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    pub source: String,
    pub document: LyricDocument,
    pub quality: LyricTrackQuality,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct InlineLyricLine {
    pub start_ms: u32,
    pub duration_ms: Option<u32>,
    pub text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub translation: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reading: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub romanized: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ruby: Vec<LyricRubySpan>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub source_refs: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct UnifiedLyricScore {
    pub final_score: f32,
    pub timing_score: f32,
    pub completeness_score: f32,
    pub enrichment_score: f32,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum UnifiedLyricMode {
    #[default]
    Tracks,
    Inline,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct UnifiedLyric {
    #[serde(default = "default_unified_lyric_schema_version")]
    pub schema_version: String,
    pub meta: LyricMeta,
    pub mode: UnifiedLyricMode,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tracks: Vec<LyricTrack>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub inline_lines: Vec<InlineLyricLine>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub source_refs: Vec<String>,
    pub score: UnifiedLyricScore,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cache_refs: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub annotations: Vec<Annotation>,
}

impl Default for UnifiedLyric {
    fn default() -> Self {
        Self {
            schema_version: default_unified_lyric_schema_version(),
            meta: LyricMeta::default(),
            mode: UnifiedLyricMode::default(),
            tracks: Vec::new(),
            inline_lines: Vec::new(),
            source_refs: Vec::new(),
            score: UnifiedLyricScore::default(),
            cache_refs: Vec::new(),
            warnings: Vec::new(),
            annotations: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(start_ms: u32, duration_ms: Option<u32>, text: &str) -> LyricLine {
        LyricLine {
            start_ms,
            duration_ms,
            text: text.into(),
            ..LyricLine::default()
        }
    }

    #[test]
    fn sort_and_fill_durations_sorts_and_fills_missing() {
        let mut document = LyricDocument {
            meta: LyricMeta::default(),
            lines: vec![
                line(2_000, None, "second"),
                line(1_000, None, "first"),
                line(3_500, None, "third"),
            ],
        };
        document.sort_and_fill_durations();
        let starts: Vec<u32> = document.lines.iter().map(|l| l.start_ms).collect();
        assert_eq!(starts, vec![1_000, 2_000, 3_500]);
        assert_eq!(document.lines[0].duration_ms, Some(1_000));
        assert_eq!(document.lines[1].duration_ms, Some(1_500));
        assert_eq!(document.lines[2].duration_ms, None);
    }

    #[test]
    fn sort_and_fill_durations_keeps_existing_duration() {
        let mut document = LyricDocument {
            meta: LyricMeta::default(),
            lines: vec![line(1_000, Some(250), "a"), line(2_000, None, "b")],
        };
        document.sort_and_fill_durations();
        assert_eq!(document.lines[0].duration_ms, Some(250));
        assert_eq!(document.lines[1].duration_ms, None);
    }

    #[test]
    fn sort_and_fill_durations_handles_empty_document() {
        let mut document = LyricDocument::default();
        document.sort_and_fill_durations();
        assert!(document.lines.is_empty());
    }

    #[test]
    fn serde_roundtrip_unified_lyric_is_equivalent() {
        let unified = UnifiedLyric {
            tracks: vec![LyricTrack {
                kind: LyricTrackKind::Original,
                language: Some("zh-Hans".into()),
                source: "kugou".into(),
                document: LyricDocument {
                    meta: LyricMeta {
                        title: Some("Song".into()),
                        artist: Some("Artist".into()),
                        ..LyricMeta::default()
                    },
                    lines: vec![LyricLine {
                        start_ms: 1_000,
                        duration_ms: Some(900),
                        text: "hello".into(),
                        words: vec![LyricWord {
                            offset_ms: 1_000,
                            duration_ms: 500,
                            text: "he".into(),
                        }],
                        ruby: vec![LyricRubySpan {
                            start_char: 0,
                            end_char: 2,
                            text: "he".into(),
                            reading: "ヒ".into(),
                        }],
                        ..LyricLine::default()
                    }],
                },
                quality: LyricTrackQuality {
                    score: 0.5,
                    line_count: 1,
                    timed_line_count: 1,
                    word_timing_count: 1,
                },
                ..LyricTrack::default()
            }],
            annotations: vec![Annotation {
                annotation_type: AnnotationType::Breath,
                start_ms: 1_200,
                duration_ms: 80,
                text: "^".into(),
            }],
            ..UnifiedLyric::default()
        };
        let json = serde_json::to_string(&unified).unwrap();
        let parsed: UnifiedLyric = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, unified);
    }

    #[test]
    fn enums_serialize_as_snake_case() {
        assert_eq!(
            serde_json::to_string(&AnnotationType::LongTone).unwrap(),
            r#""long_tone""#
        );
        assert_eq!(
            serde_json::to_string(&AnnotationType::PortamentoDown).unwrap(),
            r#""portamento_down""#
        );
        assert_eq!(
            serde_json::to_string(&LyricTrackKind::PlainFallback).unwrap(),
            r#""plain_fallback""#
        );
        assert_eq!(
            serde_json::to_string(&UnifiedLyricMode::Inline).unwrap(),
            r#""inline""#
        );
        let kind: LyricTrackKind = serde_json::from_str(r#""translation""#).unwrap();
        assert_eq!(kind, LyricTrackKind::Translation);
    }

    #[test]
    fn default_serialization_injects_schema_version_and_omits_empty_fields() {
        let json = serde_json::to_value(UnifiedLyric::default()).unwrap();
        assert_eq!(json["schema_version"], "1.0");
        assert_eq!(json["mode"], "tracks");
        for key in [
            "tracks",
            "inline_lines",
            "source_refs",
            "cache_refs",
            "warnings",
            "annotations",
        ] {
            assert!(
                json.get(key).is_none(),
                "expected key `{key}` to be omitted when empty"
            );
        }
    }

    #[test]
    fn missing_schema_version_parses_with_default_backfill() {
        let mut json = serde_json::to_value(UnifiedLyric::default()).unwrap();
        json.as_object_mut().unwrap().remove("schema_version");
        let parsed: UnifiedLyric = serde_json::from_value(json).unwrap();
        assert_eq!(parsed.schema_version, UNIFIED_LYRIC_SCHEMA_VERSION);
    }
}
