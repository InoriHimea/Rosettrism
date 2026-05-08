use regex::Regex;

use crate::model::{LyricDocument, LyricLine};
use crate::{Error, Result};

pub fn parse(text: &str) -> Result<LyricDocument> {
    let timestamp_re = Regex::new(r"\[(\d{1,4}):(\d{2})(?:[.:](\d{1,3}))?\]")
        .map_err(|err| Error::Parse(err.to_string()))?;
    let meta_re =
        Regex::new(r"^\[([a-zA-Z]+):(.*)\]$").map_err(|err| Error::Parse(err.to_string()))?;

    let mut doc = LyricDocument::default();

    for raw_line in text.trim_start_matches('\u{feff}').lines() {
        let line = raw_line.trim_end();
        if line.trim().is_empty() {
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

        let times: Vec<u32> = timestamp_re
            .captures_iter(line)
            .filter_map(|caps| {
                let minutes = caps.get(1)?.as_str().parse::<u32>().ok()?;
                let seconds = caps.get(2)?.as_str().parse::<u32>().ok()?;
                let fraction = caps.get(3).map(|m| m.as_str()).unwrap_or("0");
                Some(minutes * 60_000 + seconds * 1_000 + fraction_to_ms(fraction))
            })
            .collect();

        if times.is_empty() {
            continue;
        }

        let lyric_text = timestamp_re.replace_all(line, "").trim().to_string();
        for start_ms in times {
            doc.lines.push(LyricLine {
                start_ms,
                duration_ms: None,
                text: lyric_text.clone(),
                words: Vec::new(),
                ruby: Vec::new(),
                reading: None,
                romanized: None,
            });
        }
    }

    doc.sort_and_fill_durations();
    Ok(doc)
}

pub(crate) fn set_meta(doc: &mut LyricDocument, key: &str, value: String) {
    match key.to_ascii_lowercase().as_str() {
        "ti" | "title" => doc.meta.title = Some(value),
        "ar" | "artist" => doc.meta.artist = Some(value),
        "al" | "album" => doc.meta.album = Some(value),
        "by" => doc.meta.by = Some(value),
        "offset" => doc.meta.offset_ms = value.parse::<i64>().ok(),
        _ => {}
    }
}

pub(crate) fn fraction_to_ms(fraction: &str) -> u32 {
    match fraction.len() {
        0 => 0,
        1 => fraction.parse::<u32>().unwrap_or(0) * 100,
        2 => fraction.parse::<u32>().unwrap_or(0) * 10,
        _ => fraction[..3].parse::<u32>().unwrap_or(0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_lrc_and_fills_duration() {
        let doc = parse("[ti:Song]\n[00:01.20]one\n[00:03.000]two\n").unwrap();

        assert_eq!(doc.meta.title.as_deref(), Some("Song"));
        assert_eq!(doc.lines[0].start_ms, 1200);
        assert_eq!(doc.lines[0].duration_ms, Some(1800));
        assert_eq!(doc.lines[1].text, "two");
    }
}
