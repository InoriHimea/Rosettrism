use crate::model::{LyricDocument, LyricLine};
use crate::Result;

pub fn parse(text: &str) -> Result<LyricDocument> {
    let mut doc = LyricDocument::default();

    for raw_line in text.trim_start_matches('\u{feff}').lines() {
        let line = raw_line.trim_end();
        if line.trim().is_empty() {
            continue;
        }

        doc.lines.push(LyricLine {
            start_ms: 0,
            duration_ms: None,
            text: line.to_string(),
            words: Vec::new(),
            ruby: Vec::new(),
            reading: None,
            romanized: None,
        });
    }

    Ok(doc)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_untimed_text_without_durations() {
        let doc = parse("one\n\ntwo\n").unwrap();

        assert_eq!(doc.lines.len(), 2);
        assert_eq!(doc.lines[0].start_ms, 0);
        assert_eq!(doc.lines[0].duration_ms, None);
        assert_eq!(doc.lines[1].text, "two");
    }
}
