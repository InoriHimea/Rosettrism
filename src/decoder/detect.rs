use super::InputFormat;

pub fn detect(bytes: &[u8]) -> InputFormat {
    if bytes.starts_with(b"krc1") {
        return InputFormat::Krc;
    }

    if bytes.starts_with(&[
        0x98, 0x25, 0xb0, 0xac, 0xe3, 0x02, 0x83, 0x68, 0xe8, 0xfc, 0x6c,
    ]) {
        return InputFormat::Qrc;
    }

    let sample_len = bytes.len().min(2048);
    let sample = String::from_utf8_lossy(&bytes[..sample_len]);
    let trimmed = sample.trim_start_matches('\u{feff}').trim_start();

    if looks_like_apple_music_ttml(trimmed) {
        return InputFormat::AppleMusic;
    }

    if looks_like_yrc(trimmed) {
        return InputFormat::Yrc;
    }

    if looks_like_document_json(trimmed) {
        return InputFormat::Json;
    }

    if trimmed.starts_with("<?xml")
        || trimmed.starts_with("<QrcInfos")
        || trimmed.contains("<LyricInfo")
        || trimmed.contains("LyricContent=")
    {
        return InputFormat::Qrc;
    }

    if trimmed.starts_with("[ti:")
        || trimmed.starts_with("[ar:")
        || trimmed
            .lines()
            .any(|line| line.starts_with('[') && line.contains(']'))
    {
        if trimmed.contains('(') && trimmed.contains(',') && trimmed.contains(')') {
            InputFormat::Qrc
        } else {
            InputFormat::Lrc
        }
    } else if looks_like_hex(trimmed) {
        InputFormat::Qrc
    } else {
        InputFormat::Auto
    }
}

fn looks_like_hex(value: &str) -> bool {
    let compact: String = value.chars().filter(|ch| !ch.is_whitespace()).collect();
    compact.len() >= 16
        && compact.len().is_multiple_of(2)
        && compact.chars().all(|ch| ch.is_ascii_hexdigit())
}

fn looks_like_apple_music_ttml(value: &str) -> bool {
    (value.starts_with("<?xml") || value.starts_with("<tt"))
        && value.contains("<tt")
        && value.contains("<body")
        && (value.contains("http://www.w3.org/ns/ttml")
            || value.contains("itunes.apple.com/lyric-ttml-extensions")
            || value.contains("<p begin=")
            || value.contains("<span begin="))
}

fn looks_like_yrc(value: &str) -> bool {
    value.lines().any(|line| {
        let trimmed = line.trim_start();
        trimmed.starts_with(r#"{"t":"#) && trimmed.contains(r#""c":"#)
    })
}

fn looks_like_document_json(value: &str) -> bool {
    value.starts_with('{')
        && value.contains(r#""lines""#)
        && (value.contains(r#""meta""#) || value.contains(r#""start_ms""#))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_apple_music_ttml_before_generic_xml() {
        let input = br#"<?xml version="1.0"?><tt xmlns="http://www.w3.org/ns/ttml"><body><div><p begin="00:01.000" end="00:02.000">Hi</p></div></body></tt>"#;

        assert_eq!(detect(input), InputFormat::AppleMusic);
    }

    #[test]
    fn detects_yrc_json_metadata() {
        let input = r#"{"t":0,"c":[{"tx":"Lyric: "},{"tx":"DECO*27"}]}"#.as_bytes();

        assert_eq!(detect(input), InputFormat::Yrc);
    }

    #[test]
    fn detects_rosettrism_document_json() {
        let input =
            br#"{"meta":{},"lines":[{"start_ms":0,"duration_ms":null,"text":"Hi","words":[]}]}"#;

        assert_eq!(detect(input), InputFormat::Json);
    }
}
