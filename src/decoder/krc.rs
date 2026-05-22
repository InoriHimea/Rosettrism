use std::io::Read;

use flate2::read::{DeflateDecoder, ZlibDecoder};
use regex::Regex;

use crate::decoder::lrc::set_meta;
use crate::model::{LyricDocument, LyricLine, LyricWord};
use crate::{Error, Result};

const KRC_MAGIC: &[u8; 4] = b"krc1";
const KRC_XOR_KEY: &[u8] = b"@Gaw^2tGQ61-\xce\xd2ni";

pub fn decode(bytes: &[u8]) -> Result<LyricDocument> {
    let text = decode_raw(bytes)?;
    parse(&text)
}

pub fn decode_raw(bytes: &[u8]) -> Result<String> {
    if let Ok(text) = std::str::from_utf8(bytes) {
        if !bytes.starts_with(KRC_MAGIC) && text.lines().any(|line| line.starts_with('[')) {
            return Ok(text.to_string());
        }
    }

    if !bytes.starts_with(KRC_MAGIC) {
        return Err(Error::Decode("KRC payload does not start with krc1".into()));
    }

    let xored: Vec<u8> = bytes[4..]
        .iter()
        .enumerate()
        .map(|(idx, byte)| byte ^ KRC_XOR_KEY[idx % KRC_XOR_KEY.len()])
        .collect();

    let inflated = inflate_any(&xored)?;
    Ok(String::from_utf8_lossy(&inflated).to_string())
}

pub fn parse(text: &str) -> Result<LyricDocument> {
    let line_re =
        Regex::new(r"^\[(\d+),(\d+)\](.*)$").map_err(|err| Error::Parse(err.to_string()))?;
    let word_re = Regex::new(r"<(\d+),(\d+)(?:,[^>]*)?>([^<]*)")
        .map_err(|err| Error::Parse(err.to_string()))?;
    let meta_re =
        Regex::new(r"^\[([a-zA-Z]+):(.*)\]$").map_err(|err| Error::Parse(err.to_string()))?;

    let mut doc = LyricDocument::default();

    for raw_line in text.trim_start_matches('\u{feff}').lines() {
        let line = raw_line.trim();
        if line.is_empty() {
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
        let words = parse_words(body, &word_re)?;
        let text = if words.is_empty() {
            strip_word_tags(body)
        } else {
            words.iter().map(|word| word.text.as_str()).collect()
        };

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

fn inflate_any(bytes: &[u8]) -> Result<Vec<u8>> {
    let mut output = Vec::new();
    if ZlibDecoder::new(bytes).read_to_end(&mut output).is_ok() {
        return Ok(output);
    }

    output.clear();
    if DeflateDecoder::new(bytes).read_to_end(&mut output).is_ok() {
        return Ok(output);
    }

    Err(Error::Decode("KRC zlib/deflate inflate failed".into()))
}

fn parse_words(body: &str, word_re: &Regex) -> Result<Vec<LyricWord>> {
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

        words.push(LyricWord {
            offset_ms: parse_u32(&caps, 1)?,
            duration_ms: parse_u32(&caps, 2)?,
            text,
        });
    }

    Ok(words)
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
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(ch),
            _ => {}
        }
    }

    out.trim().to_string()
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use flate2::write::ZlibEncoder;
    use flate2::Compression;

    use super::*;

    #[test]
    fn parses_plain_krc_text() {
        let doc = parse("[ti:Song]\n[1000,900]<0,300,0>你<300,600,0>好\n").unwrap();

        assert_eq!(doc.meta.title.as_deref(), Some("Song"));
        assert_eq!(doc.lines[0].start_ms, 1000);
        assert_eq!(doc.lines[0].duration_ms, Some(900));
        assert_eq!(doc.lines[0].text, "你好");
        assert_eq!(doc.lines[0].words[1].offset_ms, 300);
    }

    #[test]
    fn decodes_krc1_payload() {
        let plain = b"[1000,500]<0,500,0>Hi\n";
        let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(plain).unwrap();
        let compressed = encoder.finish().unwrap();

        let mut payload = Vec::from(KRC_MAGIC.as_slice());
        payload.extend(
            compressed
                .iter()
                .enumerate()
                .map(|(idx, byte)| byte ^ KRC_XOR_KEY[idx % KRC_XOR_KEY.len()]),
        );

        let doc = decode(&payload).unwrap();
        assert_eq!(doc.lines[0].text, "Hi");
    }
}
