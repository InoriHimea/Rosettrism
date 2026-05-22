use regex::Regex;
use serde::Deserialize;

use crate::decoder::lrc::set_meta;
use crate::model::{LyricDocument, LyricLine, LyricWord};
use crate::{Error, Result};

pub fn parse(text: &str) -> Result<LyricDocument> {
    let line_re =
        Regex::new(r"^\[(\d+),(\d+)\](.*)$").map_err(|err| Error::Parse(err.to_string()))?;
    let meta_re =
        Regex::new(r"^\[([a-zA-Z]+):(.*)\]$").map_err(|err| Error::Parse(err.to_string()))?;
    let word_re = Regex::new(r"\((\d+),(\d+)(?:,[^)]*)?\)([^()]*)")
        .map_err(|err| Error::Parse(err.to_string()))?;

    let mut doc = LyricDocument::default();
    doc.meta.source = Some("netease".to_string());

    for raw_line in text.trim_start_matches('\u{feff}').lines() {
        let line = raw_line.trim();
        if line.is_empty() {
            continue;
        }

        if line.starts_with('{') {
            if let Some(line) = parse_json_line(line)? {
                doc.lines.push(line);
            }
            continue;
        }

        if let Some(caps) = meta_re.captures(line) {
            let key = caps.get(1).map(|m| m.as_str()).unwrap_or_default();
            let value = caps
                .get(2)
                .map(|m| m.as_str().trim().to_string())
                .unwrap_or_default();
            set_meta(&mut doc, key, value);
            continue;
        }

        let Some(caps) = line_re.captures(line) else {
            continue;
        };

        let start_ms = parse_u32(&caps, 1)?;
        let duration_ms = parse_u32(&caps, 2)?;
        let body = caps.get(3).map(|m| m.as_str()).unwrap_or_default();
        let words = parse_words(body, start_ms, &word_re)?;
        let text = if words.is_empty() {
            strip_word_tags(body)
        } else {
            words.iter().map(|word| word.text.as_str()).collect()
        };

        if text.trim().is_empty() {
            continue;
        }

        doc.lines.push(LyricLine {
            start_ms,
            duration_ms: Some(duration_ms),
            text,
            words,
            ruby: Vec::new(),
            translation: None,
            reading: None,
            romanized: None,
        });
    }

    doc.sort_and_fill_durations();
    Ok(doc)
}

fn parse_json_line(line: &str) -> Result<Option<LyricLine>> {
    let json =
        serde_json::from_str::<YrcJsonLine>(line).map_err(|err| Error::Parse(err.to_string()))?;
    let text = json
        .content
        .into_iter()
        .filter_map(|item| item.text)
        .collect::<String>();

    if text.trim().is_empty() {
        return Ok(None);
    }

    Ok(Some(LyricLine {
        start_ms: json.time,
        duration_ms: None,
        text,
        words: Vec::new(),
        ruby: Vec::new(),
        translation: None,
        reading: None,
        romanized: None,
    }))
}

fn parse_words(body: &str, line_start_ms: u32, word_re: &Regex) -> Result<Vec<LyricWord>> {
    let mut words = Vec::new();
    for caps in word_re.captures_iter(body) {
        let text = caps
            .get(3)
            .map(|m| m.as_str())
            .unwrap_or_default()
            .to_string();
        if text.is_empty() {
            continue;
        }

        let word_start = parse_u32(&caps, 1)?;
        words.push(LyricWord {
            offset_ms: word_start_to_offset(word_start, line_start_ms),
            duration_ms: parse_u32(&caps, 2)?,
            text,
        });
    }

    Ok(words)
}

fn word_start_to_offset(word_start_ms: u32, line_start_ms: u32) -> u32 {
    if word_start_ms >= line_start_ms {
        word_start_ms - line_start_ms
    } else {
        word_start_ms
    }
}

fn parse_u32(caps: &regex::Captures<'_>, index: usize) -> Result<u32> {
    caps.get(index)
        .ok_or_else(|| Error::Parse(format!("missing capture {index}")))?
        .as_str()
        .parse::<u32>()
        .map_err(|err| Error::Parse(err.to_string()))
}

fn strip_word_tags(body: &str) -> String {
    let mut out = String::new();
    let mut in_tag = false;

    for ch in body.chars() {
        match ch {
            '(' => in_tag = true,
            ')' => in_tag = false,
            _ if !in_tag => out.push(ch),
            _ => {}
        }
    }

    out.trim().to_string()
}

#[derive(Debug, Deserialize)]
struct YrcJsonLine {
    #[serde(rename = "t")]
    time: u32,
    #[serde(rename = "c", default)]
    content: Vec<YrcJsonSegment>,
}

#[derive(Debug, Deserialize)]
struct YrcJsonSegment {
    #[serde(rename = "tx", default)]
    text: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_yrc_word_lines() {
        let doc = parse(
            "[ti:Song]\n[54260,3090](54260,900,0)Stop (55160,480,0)and (55640,1710,0)stare\n",
        )
        .unwrap();

        assert_eq!(doc.meta.title.as_deref(), Some("Song"));
        assert_eq!(doc.meta.source.as_deref(), Some("netease"));
        assert_eq!(doc.lines[0].start_ms, 54_260);
        assert_eq!(doc.lines[0].duration_ms, Some(3_090));
        assert_eq!(doc.lines[0].text, "Stop and stare");
        assert_eq!(doc.lines[0].words[1].offset_ms, 900);
        assert_eq!(doc.lines[0].words[1].duration_ms, 480);
    }

    #[test]
    fn parses_json_metadata_lines() {
        let doc = parse(
            r#"{"t":0,"c":[{"tx":"作词: "},{"tx":"DECO*27"}]}
{"t":1000,"c":[{"tx":"作曲: "},{"tx":"DECO*27"}]}"#,
        )
        .unwrap();

        assert_eq!(doc.lines[0].start_ms, 0);
        assert_eq!(doc.lines[0].duration_ms, Some(1_000));
        assert_eq!(doc.lines[0].text, "作词: DECO*27");
        assert_eq!(doc.lines[1].text, "作曲: DECO*27");
    }

    #[test]
    fn accepts_relative_word_offsets() {
        let doc = parse("[1000,900](0,300,0)你(300,600,0)好\n").unwrap();

        assert_eq!(doc.lines[0].text, "你好");
        assert_eq!(doc.lines[0].words[1].offset_ms, 300);
    }
}
