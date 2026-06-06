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
