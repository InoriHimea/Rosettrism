use crate::model::LyricDocument;

pub fn to_string(document: &LyricDocument) -> String {
    let mut output = String::new();

    if let Some(title) = &document.meta.title {
        output.push_str(&format!("[ti:{title}]\n"));
    }
    if let Some(artist) = &document.meta.artist {
        output.push_str(&format!("[ar:{artist}]\n"));
    }
    if let Some(album) = &document.meta.album {
        output.push_str(&format!("[al:{album}]\n"));
    }
    if let Some(by) = &document.meta.by {
        output.push_str(&format!("[by:{by}]\n"));
    }
    if let Some(offset) = document.meta.offset_ms {
        output.push_str(&format!("[offset:{offset}]\n"));
    }

    if !output.is_empty() {
        output.push('\n');
    }

    for line in &document.lines {
        output.push_str(&format!(
            "[{}]{}\n",
            format_timestamp(line.start_ms),
            line.text
        ));
    }

    output
}

fn format_timestamp(ms: u32) -> String {
    let minutes = ms / 60_000;
    let seconds = (ms % 60_000) / 1_000;
    let centiseconds = (ms % 1_000) / 10;

    format!("{minutes:02}:{seconds:02}.{centiseconds:02}")
}

#[cfg(test)]
mod tests {
    use crate::model::{LyricDocument, LyricLine, LyricMeta};

    use super::*;

    #[test]
    fn exports_lrc() {
        let doc = LyricDocument {
            meta: LyricMeta {
                title: Some("Song".into()),
                ..Default::default()
            },
            lines: vec![LyricLine {
                start_ms: 61_230,
                duration_ms: Some(500),
                text: "line".into(),
                words: Vec::new(),
                ruby: Vec::new(),
                translation: None,
                reading: None,
                romanized: None,
            }],
        };

        let lrc = to_string(&doc);
        assert!(lrc.contains("[ti:Song]"));
        assert!(lrc.contains("[01:01.23]line"));
    }
}
