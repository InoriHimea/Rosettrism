use regex::Regex;

use crate::model::{Annotation, AnnotationType};

/// 将原始标注字符串解析为结构化标注列表
///
/// 格式示例：
/// ```text
/// [00:05.00]这`是一^首歌
/// [00:10.00]唱↑得很_好↓听
/// ```
///
/// 标注符号紧跟在对应字符之后，表示该字符的演唱技巧。
pub fn parse(raw: &str) -> Vec<Annotation> {
    if raw.trim().is_empty() {
        return Vec::new();
    }

    let timestamp_re = match Regex::new(r"^\[(\d{1,4}):(\d{2})(?:[.:](\d{1,3}))?\](.*)$") {
        Ok(re) => re,
        Err(_) => return Vec::new(),
    };

    let mut annotations = Vec::new();

    for line in raw.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        let caps = match timestamp_re.captures(line) {
            Some(caps) => caps,
            None => continue,
        };

        let minutes: u32 = match caps.get(1).and_then(|m| m.as_str().parse().ok()) {
            Some(v) => v,
            None => continue,
        };
        let seconds: u32 = match caps.get(2).and_then(|m| m.as_str().parse().ok()) {
            Some(v) => v,
            None => continue,
        };
        let fraction_str = caps.get(3).map(|m| m.as_str()).unwrap_or("0");
        let fraction_ms = fraction_to_ms(fraction_str);
        let start_ms = minutes * 60_000 + seconds * 1_000 + fraction_ms;

        let content = caps.get(4).map(|m| m.as_str()).unwrap_or("");
        if content.is_empty() {
            continue;
        }

        // Parse annotations from the content
        // Symbols appear AFTER the character they annotate
        let chars: Vec<char> = content.chars().collect();
        let mut i = 0;
        while i < chars.len() {
            let ch = chars[i];

            // Check if this character is an annotation symbol (without preceding text)
            if let Some(ann_type) = symbol_to_type(ch) {
                // Symbol without preceding text character - skip it
                // This handles cases where a symbol appears at the start
                // Look backward for the preceding character
                // Actually, if we encounter a symbol, the preceding non-symbol char is the text
                // But since we process left-to-right, we handle it differently:
                // We'll use a different approach - collect text char, then check if next is symbol
                let _ = ann_type;
                i += 1;
                continue;
            }

            // This is a text character - check if the next character is an annotation symbol
            let text_char = ch;
            i += 1;

            while i < chars.len() {
                let Some(ann_type) = symbol_to_type(chars[i]) else {
                    break;
                };
                annotations.push(Annotation {
                    annotation_type: ann_type,
                    start_ms,
                    duration_ms: 0,
                    text: text_char.to_string(),
                });
                i += 1; // consume the symbol
            }
        }
    }

    annotations
}

/// 解析 QRC 格式的助唱标注内容
///
/// QRC 格式示例：
/// ```text
/// [16346,3408]^久(16346,349)未(16695,431)放(17126,463)`晴(17589,548)的(18137,346)
/// ```
///
/// 在 QRC 格式中：
/// - 行头 `[start_ms,duration_ms]` 是行级时间戳
/// - 每个字有 `(start_ms,duration_ms)` 的字级时间戳
/// - 标注符号出现在字符之前：`^久` 表示"久"字有换气标注
pub fn parse_qrc(raw: &str) -> Vec<Annotation> {
    if raw.trim().is_empty() {
        return Vec::new();
    }

    let line_re = match Regex::new(r"^\[(\d+),(\d+)\](.*)$") {
        Ok(re) => re,
        Err(_) => return Vec::new(),
    };
    // Match word timing: (start_ms,duration_ms) or (start_ms,duration_ms,extra)
    let word_re = match Regex::new(r"\((\d+),(\d+)(?:,[^)]*?)?\)") {
        Ok(re) => re,
        Err(_) => return Vec::new(),
    };

    let mut annotations = Vec::new();

    for line in raw.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        let caps = match line_re.captures(line) {
            Some(caps) => caps,
            None => continue,
        };

        let line_start_ms: u32 = match caps.get(1).and_then(|m| m.as_str().parse().ok()) {
            Some(v) => v,
            None => continue,
        };

        let body = caps.get(3).map(|m| m.as_str()).unwrap_or("");
        if body.is_empty() {
            continue;
        }

        // Collect word timings from the body
        let word_timings: Vec<(u32, u32, usize, usize)> = word_re
            .captures_iter(body)
            .filter_map(|caps| {
                let start: u32 = caps.get(1)?.as_str().parse().ok()?;
                let duration: u32 = caps.get(2)?.as_str().parse().ok()?;
                let match_start = caps.get(0)?.start();
                let match_end = caps.get(0)?.end();
                Some((start, duration, match_start, match_end))
            })
            .collect();

        // Parse the body looking for annotation symbols before characters
        // Strategy: scan through the body, tracking position relative to word timings
        let chars: Vec<char> = body.chars().collect();
        let mut char_idx = 0;
        let mut byte_pos = 0;
        let mut pending_annotations: Vec<AnnotationType> = Vec::new();

        while char_idx < chars.len() {
            let ch = chars[char_idx];
            let ch_byte_start = byte_pos;
            byte_pos += ch.len_utf8();

            // Skip content inside parentheses (word timing tags)
            if ch == '(' {
                // Find matching closing paren
                while char_idx + 1 < chars.len() {
                    char_idx += 1;
                    let next = chars[char_idx];
                    byte_pos += next.len_utf8();
                    if next == ')' {
                        break;
                    }
                }
                char_idx += 1;
                continue;
            }

            // Check if this is an annotation symbol
            if let Some(ann_type) = symbol_to_type(ch) {
                pending_annotations.push(ann_type);
                char_idx += 1;
                continue;
            }

            // This is a text character
            if !pending_annotations.is_empty() {
                // Find the timing for this character
                // Look for the word timing that follows this character
                let (start_ms, duration_ms) = qrc_character_timing(&word_timings, ch_byte_start)
                    .unwrap_or((line_start_ms, 0));

                for ann_type in pending_annotations.drain(..) {
                    annotations.push(Annotation {
                        annotation_type: ann_type,
                        start_ms,
                        duration_ms,
                        text: ch.to_string(),
                    });
                }
            }

            char_idx += 1;
        }
    }

    annotations
}

fn qrc_character_timing(
    word_timings: &[(u32, u32, usize, usize)],
    character_byte_start: usize,
) -> Option<(u32, u32)> {
    if let Some((start, duration, _, _)) = word_timings
        .iter()
        .find(|(_, _, match_start, _)| *match_start >= character_byte_start)
    {
        return Some((*start, *duration));
    }

    word_timings
        .iter()
        .min_by_key(|(_, _, match_start, match_end)| {
            let start_distance = match_start.abs_diff(character_byte_start);
            let end_distance = match_end.abs_diff(character_byte_start);
            start_distance.min(end_distance)
        })
        .map(|(start, duration, _, _)| (*start, *duration))
}

/// 将结构化标注列表格式化回原始字符串表示
///
/// 按时间排序后，将同一时间戳的标注合并到同一行。
/// 每个标注输出为 `{text}{symbol}` 格式。
pub fn format(annotations: &[Annotation]) -> String {
    if annotations.is_empty() {
        return String::new();
    }

    // Sort by start_ms
    let mut sorted: Vec<&Annotation> = annotations.iter().collect();
    sorted.sort_by_key(|a| a.start_ms);

    let mut result = String::new();
    let mut current_ms: Option<u32> = None;

    for ann in &sorted {
        if current_ms != Some(ann.start_ms) {
            // Start a new line
            if !result.is_empty() {
                result.push('\n');
            }
            result.push_str(&format_timestamp(ann.start_ms));
            current_ms = Some(ann.start_ms);
        }
        // Write text followed by symbol
        result.push_str(&ann.text);
        result.push(type_to_symbol(ann.annotation_type));
    }

    result
}

/// Convert a fraction string to milliseconds
fn fraction_to_ms(fraction: &str) -> u32 {
    match fraction.len() {
        0 => 0,
        1 => fraction.parse::<u32>().unwrap_or(0) * 100,
        2 => fraction.parse::<u32>().unwrap_or(0) * 10,
        _ => fraction[..3].parse::<u32>().unwrap_or(0),
    }
}

/// Format milliseconds as `[mm:ss.xx]` timestamp
fn format_timestamp(ms: u32) -> String {
    let minutes = ms / 60_000;
    let seconds = (ms % 60_000) / 1_000;
    let centiseconds = (ms % 1_000) / 10;
    format!("[{:02}:{:02}.{:02}]", minutes, seconds, centiseconds)
}

/// Map annotation symbol character to AnnotationType
fn symbol_to_type(ch: char) -> Option<AnnotationType> {
    match ch {
        '`' => Some(AnnotationType::Stress),
        '^' => Some(AnnotationType::Breath),
        '_' => Some(AnnotationType::LongTone),
        '↑' => Some(AnnotationType::PortamentoUp),
        '↓' => Some(AnnotationType::PortamentoDown),
        _ => None,
    }
}

/// Map AnnotationType to its symbol character
fn type_to_symbol(ann_type: AnnotationType) -> char {
    match ann_type {
        AnnotationType::Stress => '`',
        AnnotationType::Breath => '^',
        AnnotationType::LongTone => '_',
        AnnotationType::PortamentoUp => '↑',
        AnnotationType::PortamentoDown => '↓',
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_input_returns_empty_vec() {
        assert_eq!(parse(""), Vec::new());
        assert_eq!(parse("   "), Vec::new());
        assert_eq!(parse("\n\n"), Vec::new());
    }

    #[test]
    fn parse_stress_annotation() {
        let input = "[00:05.00]这`是";
        let result = parse(input);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].annotation_type, AnnotationType::Stress);
        assert_eq!(result[0].start_ms, 5000);
        assert_eq!(result[0].text, "这");
    }

    #[test]
    fn parse_breath_annotation() {
        let input = "[00:05.00]一^首";
        let result = parse(input);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].annotation_type, AnnotationType::Breath);
        assert_eq!(result[0].start_ms, 5000);
        assert_eq!(result[0].text, "一");
    }

    #[test]
    fn parse_all_annotation_types() {
        let input = "[00:10.00]唱↑得很_好↓听";
        let result = parse(input);
        assert_eq!(result.len(), 3);

        assert_eq!(result[0].annotation_type, AnnotationType::PortamentoUp);
        assert_eq!(result[0].text, "唱");

        assert_eq!(result[1].annotation_type, AnnotationType::LongTone);
        assert_eq!(result[1].text, "很");

        assert_eq!(result[2].annotation_type, AnnotationType::PortamentoDown);
        assert_eq!(result[2].text, "好");
    }

    #[test]
    fn parse_multiple_lines() {
        let input = "[00:05.00]这`是一^首歌\n[00:10.00]唱↑得很_好↓听";
        let result = parse(input);
        assert_eq!(result.len(), 5);

        // Line 1
        assert_eq!(result[0].start_ms, 5000);
        assert_eq!(result[0].annotation_type, AnnotationType::Stress);
        assert_eq!(result[0].text, "这");

        assert_eq!(result[1].start_ms, 5000);
        assert_eq!(result[1].annotation_type, AnnotationType::Breath);
        assert_eq!(result[1].text, "一");

        // Line 2
        assert_eq!(result[2].start_ms, 10000);
        assert_eq!(result[2].annotation_type, AnnotationType::PortamentoUp);
        assert_eq!(result[2].text, "唱");
    }

    #[test]
    fn parse_with_three_digit_fraction() {
        let input = "[01:23.456]字`符";
        let result = parse(input);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].start_ms, 83_456);
    }

    #[test]
    fn skip_invalid_lines() {
        let input = "这不是有效行\n[00:05.00]这`是\n无效内容\n[00:10.00]好_的";
        let result = parse(input);
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].text, "这");
        assert_eq!(result[1].text, "好");
    }

    #[test]
    fn format_empty_annotations() {
        assert_eq!(format(&[]), "");
    }

    #[test]
    fn parse_qrc_prefix_annotations_use_word_timing() {
        let input = "[16346,3408]^久(16346,349)`晴(17589,548)_天(18137,346)";
        let result = parse_qrc(input);

        assert_eq!(result.len(), 3);
        assert_eq!(result[0].annotation_type, AnnotationType::Breath);
        assert_eq!(result[0].start_ms, 16346);
        assert_eq!(result[0].duration_ms, 349);
        assert_eq!(result[0].text, "久");
        assert_eq!(result[1].annotation_type, AnnotationType::Stress);
        assert_eq!(result[1].start_ms, 17589);
        assert_eq!(result[1].duration_ms, 548);
        assert_eq!(result[1].text, "晴");
        assert_eq!(result[2].annotation_type, AnnotationType::LongTone);
        assert_eq!(result[2].start_ms, 18137);
        assert_eq!(result[2].duration_ms, 346);
        assert_eq!(result[2].text, "天");
    }

    #[test]
    fn parse_adjacent_suffix_annotations_keep_same_character_markers() {
        let input = "[00:01.00]横^`刀";
        let result = parse(input);

        assert_eq!(result.len(), 2);
        assert_eq!(result[0].annotation_type, AnnotationType::Breath);
        assert_eq!(result[0].start_ms, 1000);
        assert_eq!(result[0].text, "横");
        assert_eq!(result[1].annotation_type, AnnotationType::Stress);
        assert_eq!(result[1].start_ms, 1000);
        assert_eq!(result[1].text, "横");
    }

    #[test]
    fn parse_qrc_adjacent_prefix_annotations_keep_same_character_markers() {
        let input = "[1000,1000]^`横(1000,300)";
        let result = parse_qrc(input);

        assert_eq!(result.len(), 2);
        assert_eq!(result[0].annotation_type, AnnotationType::Breath);
        assert_eq!(result[0].start_ms, 1000);
        assert_eq!(result[0].duration_ms, 300);
        assert_eq!(result[0].text, "横");
        assert_eq!(result[1].annotation_type, AnnotationType::Stress);
        assert_eq!(result[1].start_ms, 1000);
        assert_eq!(result[1].duration_ms, 300);
        assert_eq!(result[1].text, "横");
    }

    #[test]
    fn format_single_annotation() {
        let annotations = vec![Annotation {
            annotation_type: AnnotationType::Stress,
            start_ms: 5000,
            duration_ms: 0,
            text: "这".to_string(),
        }];
        let result = format(&annotations);
        assert_eq!(result, "[00:05.00]这`");
    }

    #[test]
    fn format_multiple_same_timestamp() {
        let annotations = vec![
            Annotation {
                annotation_type: AnnotationType::Stress,
                start_ms: 5000,
                duration_ms: 0,
                text: "这".to_string(),
            },
            Annotation {
                annotation_type: AnnotationType::Breath,
                start_ms: 5000,
                duration_ms: 0,
                text: "一".to_string(),
            },
        ];
        let result = format(&annotations);
        assert_eq!(result, "[00:05.00]这`一^");
    }

    #[test]
    fn format_multiple_timestamps() {
        let annotations = vec![
            Annotation {
                annotation_type: AnnotationType::Stress,
                start_ms: 5000,
                duration_ms: 0,
                text: "这".to_string(),
            },
            Annotation {
                annotation_type: AnnotationType::PortamentoUp,
                start_ms: 10000,
                duration_ms: 0,
                text: "唱".to_string(),
            },
        ];
        let result = format(&annotations);
        assert_eq!(result, "[00:05.00]这`\n[00:10.00]唱↑");
    }

    #[test]
    fn round_trip_basic() {
        let annotations = vec![
            Annotation {
                annotation_type: AnnotationType::Stress,
                start_ms: 5000,
                duration_ms: 0,
                text: "这".to_string(),
            },
            Annotation {
                annotation_type: AnnotationType::Breath,
                start_ms: 5000,
                duration_ms: 0,
                text: "一".to_string(),
            },
            Annotation {
                annotation_type: AnnotationType::PortamentoUp,
                start_ms: 10000,
                duration_ms: 0,
                text: "唱".to_string(),
            },
        ];
        let formatted = format(&annotations);
        let parsed = parse(&formatted);

        assert_eq!(parsed.len(), annotations.len());
        for (original, reparsed) in annotations.iter().zip(parsed.iter()) {
            assert_eq!(original.annotation_type, reparsed.annotation_type);
            assert_eq!(original.start_ms, reparsed.start_ms);
            assert_eq!(original.text, reparsed.text);
        }
    }
}
