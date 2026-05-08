use quick_xml::events::{BytesStart, Event};
use quick_xml::Reader;

use crate::model::{LyricDocument, LyricLine, LyricWord};
use crate::{Error, Result};

#[derive(Debug)]
struct RawWord {
    start_ms: u32,
    end_ms: u32,
    text: String,
}

pub fn parse(text: &str) -> Result<LyricDocument> {
    let mut reader = Reader::from_str(text.trim_start_matches('\u{feff}'));
    reader.config_mut().trim_text(false);

    let mut document = LyricDocument::default();
    document.meta.source = Some("apple_music".to_string());
    let mut agent_names = Vec::new();

    loop {
        match reader.read_event() {
            Ok(Event::Start(event)) => match local_name(event.name().as_ref()) {
                b"title" => {
                    let title = collect_element_text(&mut reader, b"title")?;
                    let title = normalize_ttml_text(&title);
                    if !title.is_empty() {
                        document.meta.title = Some(title);
                    }
                }
                b"agent" => {
                    for name in parse_agent(&mut reader)? {
                        if !agent_names.contains(&name) {
                            agent_names.push(name);
                        }
                    }
                }
                b"p" => {
                    if let Some(line) = parse_line(&mut reader, &event)? {
                        document.lines.push(line);
                    }
                }
                _ => {}
            },
            Ok(Event::Empty(event)) if local_name(event.name().as_ref()) == b"p" => {
                if let Some(line) = empty_line(&reader, &event)? {
                    document.lines.push(line);
                }
            }
            Ok(Event::Eof) => break,
            Err(err) => return Err(Error::Parse(err.to_string())),
            _ => {}
        }
    }

    if document.meta.artist.is_none() && !agent_names.is_empty() {
        document.meta.artist = Some(agent_names.join("/"));
    }

    document.sort_and_fill_durations();
    Ok(document)
}

fn parse_agent(reader: &mut Reader<&[u8]>) -> Result<Vec<String>> {
    let mut names = Vec::new();

    loop {
        match reader.read_event() {
            Ok(Event::Start(event)) if local_name(event.name().as_ref()) == b"name" => {
                let name = collect_element_text(reader, b"name")?;
                let name = normalize_ttml_text(&name);
                if !name.is_empty() && !names.contains(&name) {
                    names.push(name);
                }
            }
            Ok(Event::End(event)) if local_name(event.name().as_ref()) == b"agent" => break,
            Ok(Event::Eof) => {
                return Err(Error::Parse(
                    "unexpected end of TTML while reading agent".into(),
                ))
            }
            Err(err) => return Err(Error::Parse(err.to_string())),
            _ => {}
        }
    }

    Ok(names)
}

fn parse_line(reader: &mut Reader<&[u8]>, event: &BytesStart<'_>) -> Result<Option<LyricLine>> {
    let begin_ms = time_attr(reader, event, b"begin")?;
    let end_ms = time_attr(reader, event, b"end")?;
    let mut text = String::new();
    let mut raw_words = Vec::new();

    collect_mixed_content(reader, b"p", &mut text, &mut raw_words)?;
    build_line(begin_ms, end_ms, text, raw_words)
}

fn empty_line(reader: &Reader<&[u8]>, event: &BytesStart<'_>) -> Result<Option<LyricLine>> {
    build_line(
        time_attr(reader, event, b"begin")?,
        time_attr(reader, event, b"end")?,
        String::new(),
        Vec::new(),
    )
}

fn build_line(
    begin_ms: Option<u32>,
    end_ms: Option<u32>,
    text: String,
    raw_words: Vec<RawWord>,
) -> Result<Option<LyricLine>> {
    let Some(start_ms) = begin_ms.or_else(|| raw_words.iter().map(|word| word.start_ms).min())
    else {
        return Ok(None);
    };

    let end_ms = end_ms.or_else(|| raw_words.iter().map(|word| word.end_ms).max());
    let duration_ms = end_ms.and_then(|end| end.checked_sub(start_ms));
    let words = raw_words
        .into_iter()
        .filter_map(|word| {
            let text = normalize_ttml_text(&word.text);
            if text.is_empty() {
                return None;
            }

            Some(LyricWord {
                offset_ms: word.start_ms.saturating_sub(start_ms),
                duration_ms: word.end_ms.saturating_sub(word.start_ms),
                text,
            })
        })
        .collect::<Vec<_>>();
    let text = normalize_ttml_text(&text);

    if text.is_empty() && words.is_empty() {
        return Ok(None);
    }

    Ok(Some(LyricLine {
        start_ms,
        duration_ms,
        text,
        words,
        ruby: Vec::new(),
        reading: None,
        romanized: None,
    }))
}

fn collect_mixed_content(
    reader: &mut Reader<&[u8]>,
    end_name: &[u8],
    text: &mut String,
    raw_words: &mut Vec<RawWord>,
) -> Result<()> {
    loop {
        match reader.read_event() {
            Ok(Event::Text(event)) => {
                text.push_str(
                    &event
                        .unescape()
                        .map_err(|err| Error::Parse(err.to_string()))?,
                );
            }
            Ok(Event::CData(event)) => text.push_str(&String::from_utf8_lossy(event.as_ref())),
            Ok(Event::Start(event)) if local_name(event.name().as_ref()) == b"span" => {
                collect_span(reader, &event, text, raw_words)?;
            }
            Ok(Event::Empty(event)) if local_name(event.name().as_ref()) == b"br" => {
                text.push('\n');
            }
            Ok(Event::Start(event)) => {
                let end = local_name(event.name().as_ref()).to_vec();
                collect_mixed_content(reader, &end, text, raw_words)?;
            }
            Ok(Event::End(event)) if local_name(event.name().as_ref()) == end_name => break,
            Ok(Event::Eof) => {
                return Err(Error::Parse(
                    "unexpected end of TTML while reading lyric line".into(),
                ))
            }
            Err(err) => return Err(Error::Parse(err.to_string())),
            _ => {}
        }
    }

    Ok(())
}

fn collect_span(
    reader: &mut Reader<&[u8]>,
    event: &BytesStart<'_>,
    line_text: &mut String,
    raw_words: &mut Vec<RawWord>,
) -> Result<String> {
    let begin_ms = time_attr(reader, event, b"begin")?;
    let end_ms = time_attr(reader, event, b"end")?;
    let mut span_text = String::new();
    let word_count_before = raw_words.len();

    collect_span_content(reader, &mut span_text, line_text, raw_words)?;

    if raw_words.len() == word_count_before {
        if let (Some(start_ms), Some(end_ms)) = (begin_ms, end_ms) {
            if end_ms > start_ms {
                let text = normalize_ttml_text(&span_text);
                if !text.is_empty() {
                    raw_words.push(RawWord {
                        start_ms,
                        end_ms,
                        text,
                    });
                }
            }
        }
    }

    Ok(span_text)
}

fn collect_span_content(
    reader: &mut Reader<&[u8]>,
    span_text: &mut String,
    line_text: &mut String,
    raw_words: &mut Vec<RawWord>,
) -> Result<()> {
    loop {
        match reader.read_event() {
            Ok(Event::Text(event)) => {
                let text = event
                    .unescape()
                    .map_err(|err| Error::Parse(err.to_string()))?;
                span_text.push_str(&text);
                line_text.push_str(&text);
            }
            Ok(Event::CData(event)) => {
                let text = String::from_utf8_lossy(event.as_ref());
                span_text.push_str(&text);
                line_text.push_str(&text);
            }
            Ok(Event::Start(event)) if local_name(event.name().as_ref()) == b"span" => {
                let child_text = collect_span(reader, &event, line_text, raw_words)?;
                span_text.push_str(&child_text);
            }
            Ok(Event::Empty(event)) if local_name(event.name().as_ref()) == b"br" => {
                span_text.push('\n');
                line_text.push('\n');
            }
            Ok(Event::Start(event)) => {
                let end = local_name(event.name().as_ref()).to_vec();
                collect_mixed_content(reader, &end, line_text, raw_words)?;
            }
            Ok(Event::End(event)) if local_name(event.name().as_ref()) == b"span" => break,
            Ok(Event::Eof) => {
                return Err(Error::Parse(
                    "unexpected end of TTML while reading span".into(),
                ))
            }
            Err(err) => return Err(Error::Parse(err.to_string())),
            _ => {}
        }
    }

    Ok(())
}

fn collect_element_text(reader: &mut Reader<&[u8]>, end_name: &[u8]) -> Result<String> {
    let mut text = String::new();
    let mut depth = 0_u32;

    loop {
        match reader.read_event() {
            Ok(Event::Text(event)) => {
                text.push_str(
                    &event
                        .unescape()
                        .map_err(|err| Error::Parse(err.to_string()))?,
                );
            }
            Ok(Event::CData(event)) => text.push_str(&String::from_utf8_lossy(event.as_ref())),
            Ok(Event::Start(event)) if local_name(event.name().as_ref()) == end_name => {
                depth = depth.saturating_add(1);
            }
            Ok(Event::End(event)) if local_name(event.name().as_ref()) == end_name => {
                if depth == 0 {
                    break;
                }
                depth -= 1;
            }
            Ok(Event::Eof) => {
                return Err(Error::Parse(
                    "unexpected end of TTML while reading text".into(),
                ))
            }
            Err(err) => return Err(Error::Parse(err.to_string())),
            _ => {}
        }
    }

    Ok(text)
}

fn time_attr(reader: &Reader<&[u8]>, event: &BytesStart<'_>, name: &[u8]) -> Result<Option<u32>> {
    for attr in event.attributes() {
        let attr = attr.map_err(|err| Error::Parse(err.to_string()))?;
        if local_name(attr.key.as_ref()) == name {
            let value = attr
                .decode_and_unescape_value(reader.decoder())
                .map_err(|err| Error::Parse(err.to_string()))?;
            return parse_clock_value(value.trim()).map(Some);
        }
    }

    Ok(None)
}

fn parse_clock_value(value: &str) -> Result<u32> {
    if let Some(value) = value.strip_suffix("ms") {
        return value
            .trim()
            .parse::<u32>()
            .map_err(|err| Error::Parse(format!("invalid TTML millisecond time: {err}")));
    }

    if let Some(value) = value.strip_suffix('s') {
        let seconds = value
            .trim()
            .parse::<f64>()
            .map_err(|err| Error::Parse(format!("invalid TTML second time: {err}")))?;
        return Ok((seconds * 1_000.0).round() as u32);
    }

    let parts = value.split(':').collect::<Vec<_>>();
    let (hours, minutes, seconds) = match parts.as_slice() {
        [minutes, seconds] => (0, parse_u32(minutes)?, *seconds),
        [hours, minutes, seconds] => (parse_u32(hours)?, parse_u32(minutes)?, *seconds),
        _ => return Err(Error::Parse(format!("invalid TTML clock value `{value}`"))),
    };

    let (seconds, milliseconds) = parse_seconds(seconds)?;
    Ok(hours
        .saturating_mul(3_600_000)
        .saturating_add(minutes.saturating_mul(60_000))
        .saturating_add(seconds.saturating_mul(1_000))
        .saturating_add(milliseconds))
}

fn parse_seconds(value: &str) -> Result<(u32, u32)> {
    let mut parts = value.splitn(2, '.');
    let seconds = parse_u32(parts.next().unwrap_or_default())?;
    let milliseconds = parts.next().map(fraction_to_ms).unwrap_or(0);
    Ok((seconds, milliseconds))
}

fn parse_u32(value: &str) -> Result<u32> {
    value
        .parse::<u32>()
        .map_err(|err| Error::Parse(err.to_string()))
}

fn fraction_to_ms(fraction: &str) -> u32 {
    let mut digits = fraction
        .chars()
        .take(3)
        .filter(|ch| ch.is_ascii_digit())
        .collect::<String>();
    while digits.len() < 3 {
        digits.push('0');
    }

    digits.parse::<u32>().unwrap_or(0)
}

fn normalize_ttml_text(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn local_name(name: &[u8]) -> &[u8] {
    name.rsplit(|byte| *byte == b':').next().unwrap_or(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_line_by_line_ttml() {
        let doc = parse(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<tt xmlns="http://www.w3.org/ns/ttml" xmlns:ttm="http://www.w3.org/ns/ttml#metadata" xmlns:itunes="http://itunes.apple.com/lyric-ttml-extensions">
  <head>
    <metadata>
      <ttm:title>City of Stars</ttm:title>
      <ttm:agent type="person" xml:id="v1"><ttm:name type="full">Ryan Gosling</ttm:name></ttm:agent>
    </metadata>
  </head>
  <body dur="02:29.720">
    <div begin="00:09.327" end="00:15.906" itunes:song-part="Verse">
      <p begin="00:09.327" end="00:12.109" ttm:agent="v1">City of stars</p>
    </div>
  </body>
</tt>"#,
        )
        .unwrap();

        assert_eq!(doc.meta.title.as_deref(), Some("City of Stars"));
        assert_eq!(doc.meta.artist.as_deref(), Some("Ryan Gosling"));
        assert_eq!(doc.meta.source.as_deref(), Some("apple_music"));
        assert_eq!(doc.lines[0].start_ms, 9_327);
        assert_eq!(doc.lines[0].duration_ms, Some(2_782));
        assert_eq!(doc.lines[0].text, "City of stars");
    }

    #[test]
    fn parses_beat_by_beat_spans() {
        let doc = parse(
            r#"<tt xmlns="http://www.w3.org/ns/ttml">
  <body>
    <div>
      <p begin="00:07.621" end="00:10.267">
        <span begin="00:07.621" end="00:07.920">I</span> <span begin="00:07.920" end="00:08.253">don't</span>
        <span begin="00:08.253" end="00:08.739">wanna</span>
      </p>
    </div>
  </body>
</tt>"#,
        )
        .unwrap();

        assert_eq!(doc.lines[0].text, "I don't wanna");
        assert_eq!(doc.lines[0].words.len(), 3);
        assert_eq!(doc.lines[0].words[1].offset_ms, 299);
        assert_eq!(doc.lines[0].words[1].duration_ms, 333);
        assert_eq!(doc.lines[0].words[1].text, "don't");
    }

    #[test]
    fn parses_word_timed_line_without_line_timing() {
        let doc = parse(
            r#"<tt xmlns="http://www.w3.org/ns/ttml"><body><div><p><span begin="00:01.000" end="00:01.500">Hi</span></p></div></body></tt>"#,
        )
        .unwrap();

        assert_eq!(doc.lines[0].start_ms, 1_000);
        assert_eq!(doc.lines[0].duration_ms, Some(500));
        assert_eq!(doc.lines[0].words[0].offset_ms, 0);
    }
}
