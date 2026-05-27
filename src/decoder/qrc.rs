use std::io::Read;

use base64::Engine;
use des::cipher::generic_array::GenericArray;
use des::cipher::{BlockDecrypt, BlockEncrypt, KeyInit};
use des::{Des, TdesEde3};
use flate2::read::{DeflateDecoder, GzDecoder, ZlibDecoder};
use quick_xml::events::Event;
use quick_xml::name::QName;
use quick_xml::Reader;
use regex::Regex;

use crate::decoder::lrc::{parse as parse_lrc, set_meta};
use crate::model::{LyricDocument, LyricLine, LyricWord};
use crate::{Error, Result};

const KEY1: &[u8; 16] = b"!@#)(NHLiuy*$%^&";
const KEY2: &[u8; 16] = b"123ZXC!@#)(*$%^&";
const KEY3: &[u8; 16] = b"!@#)(*$%^&abcDEF";
const CLIENT_DES_KEY1: &[u8; 8] = b"!@#)(NHL";
const CLIENT_DES_KEY2: &[u8; 8] = b"123ZXC!@";
const CLIENT_DES_KEY3: &[u8; 8] = b"!@#)(*$%";
const QQ_LYRIC_3DES_KEY: &[u8; 24] = b"!@#)(*$%123ZXC!@!@#)(NHL";
const QRC_QMC_MAGIC: &[u8; 11] = &[
    0x98, 0x25, 0xb0, 0xac, 0xe3, 0x02, 0x83, 0x68, 0xe8, 0xfc, 0x6c,
];
const QMC1_KEY: [u8; 128] = [
    0xc3, 0x4a, 0xd6, 0xca, 0x90, 0x67, 0xf7, 0x52, 0xd8, 0xa1, 0x66, 0x62, 0x9f, 0x5b, 0x09, 0x00,
    0xc3, 0x5e, 0x95, 0x23, 0x9f, 0x13, 0x11, 0x7e, 0xd8, 0x92, 0x3f, 0xbc, 0x90, 0xbb, 0x74, 0x0e,
    0xc3, 0x47, 0x74, 0x3d, 0x90, 0xaa, 0x3f, 0x51, 0xd8, 0xf4, 0x11, 0x84, 0x9f, 0xde, 0x95, 0x1d,
    0xc3, 0xc6, 0x09, 0xd5, 0x9f, 0xfa, 0x66, 0xf9, 0xd8, 0xf0, 0xf7, 0xa0, 0x90, 0xa1, 0xd6, 0xf3,
    0xc3, 0xf3, 0xd6, 0xa1, 0x90, 0xa0, 0xf7, 0xf0, 0xd8, 0xf9, 0x66, 0xfa, 0x9f, 0xd5, 0x09, 0xc6,
    0xc3, 0x1d, 0x95, 0xde, 0x9f, 0x84, 0x11, 0xf4, 0xd8, 0x51, 0x3f, 0xaa, 0x90, 0x3d, 0x74, 0x47,
    0xc3, 0x0e, 0x74, 0xbb, 0x90, 0xbc, 0x3f, 0x92, 0xd8, 0x7e, 0x11, 0x13, 0x9f, 0x23, 0x95, 0x5e,
    0xc3, 0x00, 0x09, 0x5b, 0x9f, 0x62, 0x66, 0xa1, 0xd8, 0x52, 0xf7, 0x67, 0x90, 0xca, 0xd6, 0x4a,
];

pub fn decode(bytes: &[u8]) -> Result<LyricDocument> {
    let raw = decode_raw(bytes)?;
    let content = extract_lyric_content(&raw)?;
    parse_lyric_content(&content)
}

pub fn decode_raw_lyric_content(bytes: &[u8]) -> Result<String> {
    let raw = decode_raw(bytes)?;
    extract_lyric_content(&raw)
}

pub fn decode_raw(bytes: &[u8]) -> Result<String> {
    if bytes.starts_with(QRC_QMC_MAGIC) {
        return decrypt_payload(bytes);
    }

    let text = String::from_utf8_lossy(bytes);
    let normalized = text.trim_start_matches('\u{feff}').trim();

    if looks_like_qrc_xml(normalized) {
        return Ok(normalized.to_string());
    }

    if normalized
        .lines()
        .any(|line| is_qrc_line(line) || is_lrc_line(line))
    {
        return Ok(normalized.to_string());
    }

    if let Ok(decoded) = decrypt_payload(normalized.as_bytes()) {
        return Ok(decoded);
    }

    decrypt_payload(bytes)
}

pub fn parse_lyric_content(text: &str) -> Result<LyricDocument> {
    if text.lines().any(is_qrc_line) {
        parse_qrc_lines(text)
    } else {
        parse_lrc(text)
    }
}

pub fn decrypt_payload(payload: &[u8]) -> Result<String> {
    if payload.starts_with(QRC_QMC_MAGIC) {
        return decrypt_client_qrc_payload(payload);
    }

    let candidates = encrypted_candidates(payload);
    let mut errors = Vec::new();

    for candidate in candidates {
        if let Ok(text) = decrypt_client_qrc_payload(&candidate) {
            return Ok(text);
        }

        for decrypted in des_candidates(&candidate) {
            match inflate_any(&decrypted) {
                Ok(inflated) => {
                    let text = String::from_utf8_lossy(&inflated)
                        .trim_start_matches('\u{feff}')
                        .trim_matches(char::from(0))
                        .to_string();
                    if text.contains("<?xml") || text.contains("[") {
                        return Ok(text);
                    }
                    errors.push("inflated payload did not look like lyric text".to_string());
                }
                Err(err) => errors.push(err.to_string()),
            }
        }
    }

    Err(Error::Decode(format!(
        "QRC decrypt failed: {}",
        errors.join("; ")
    )))
}

fn extract_lyric_content(xml: &str) -> Result<String> {
    if let Some(content) = extract_cdata_content(xml)? {
        return Ok(content);
    }

    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(false);

    loop {
        match reader.read_event() {
            Ok(Event::Start(event)) | Ok(Event::Empty(event)) => {
                for attr in event.attributes().flatten() {
                    if attr.key == QName(b"LyricContent") {
                        let decoded = attr
                            .decode_and_unescape_value(reader.decoder())
                            .map_err(|err| Error::Parse(err.to_string()))?;
                        return decode_wrapped_lyric_payload(&decoded);
                    }
                }
            }
            Ok(Event::Text(event)) => {
                let text = event
                    .unescape()
                    .map_err(|err| Error::Parse(err.to_string()))?
                    .into_owned();
                if looks_like_hex(&text) {
                    if let Ok(decoded) = decrypt_payload(text.as_bytes()) {
                        return match extract_lyric_content(&decoded) {
                            Ok(content) => Ok(content),
                            Err(_) => Ok(decoded),
                        };
                    }
                }
            }
            Ok(Event::Eof) => break,
            Err(err) => return Err(Error::Parse(err.to_string())),
            _ => {}
        }
    }

    let attr_re =
        Regex::new(r#"LyricContent="([^"]*)""#).map_err(|err| Error::Parse(err.to_string()))?;
    if let Some(caps) = attr_re.captures(xml) {
        return decode_wrapped_lyric_payload(&xml_unescape(
            caps.get(1).map(|m| m.as_str()).unwrap_or_default(),
        ));
    }

    Ok(xml.to_string())
}

fn extract_cdata_content(xml: &str) -> Result<Option<String>> {
    let content_re = Regex::new(r#"(?s)<content\b[^>]*>\s*<!\[CDATA\[(.*?)\]\]>\s*</content>"#)
        .map_err(|err| Error::Parse(err.to_string()))?;
    let Some(caps) = content_re.captures(xml) else {
        return Ok(None);
    };

    let content = caps
        .get(1)
        .map(|match_| match_.as_str().trim())
        .unwrap_or_default();
    if content.is_empty() {
        return Ok(Some(String::new()));
    }

    Ok(Some(decode_wrapped_lyric_payload(content)?))
}

fn decode_wrapped_lyric_payload(content: &str) -> Result<String> {
    let content = content.trim();
    if content.is_empty() {
        return Ok(String::new());
    }

    if looks_like_hex(content) {
        let decoded = decrypt_payload(content.as_bytes())?;
        return Ok(match extract_lyric_content(&decoded) {
            Ok(content) => content,
            Err(_) => decoded,
        });
    }

    let compact = content
        .chars()
        .filter(|ch| !ch.is_whitespace())
        .collect::<String>();
    if looks_like_base64(&compact) {
        if let Ok(decoded_bytes) = base64::engine::general_purpose::STANDARD.decode(&compact) {
            if let Ok(decoded) = decrypt_payload(&decoded_bytes) {
                return Ok(match extract_lyric_content(&decoded) {
                    Ok(content) => content,
                    Err(_) => decoded,
                });
            }

            let decoded_text = String::from_utf8_lossy(&decoded_bytes)
                .trim_start_matches('\u{feff}')
                .trim_matches(char::from(0))
                .to_string();
            if looks_like_qrc_xml(&decoded_text) {
                return extract_lyric_content(&decoded_text);
            }
            if decoded_text
                .lines()
                .any(|line| is_qrc_line(line) || is_lrc_line(line))
            {
                return Ok(decoded_text);
            }
        }
    }

    Ok(content.to_string())
}

fn parse_qrc_lines(text: &str) -> Result<LyricDocument> {
    let line_re =
        Regex::new(r"^\[(\d+),(\d+)\](.*)$").map_err(|err| Error::Parse(err.to_string()))?;
    let meta_re =
        Regex::new(r"^\[([a-zA-Z]+):(.*)\]$").map_err(|err| Error::Parse(err.to_string()))?;
    let prefix_word_re = Regex::new(r"\((\d+),(\d+)(?:,[^)]*)?\)([^()]+)")
        .map_err(|err| Error::Parse(err.to_string()))?;
    let mut doc = LyricDocument::default();
    let mut non_empty_index = 0usize;

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
            non_empty_index += 1;
            continue;
        }

        if doc.meta.title.is_none() && non_empty_index < 5 {
            if let Some(title) = qrc_inline_title(line) {
                doc.meta.title = Some(title);
                non_empty_index += 1;
                continue;
            }
        }

        let Some(caps) = line_re.captures(line) else {
            non_empty_index += 1;
            continue;
        };

        let start_ms = parse_u32(&caps, 1)?;
        let duration_ms = parse_u32(&caps, 2)?;
        let body = caps.get(3).map(|m| m.as_str()).unwrap_or_default();
        let words = parse_qrc_words(body, start_ms, &prefix_word_re)?;
        let text = if words.is_empty() {
            strip_qrc_word_tags(body)
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
        non_empty_index += 1;
    }

    doc.sort_and_fill_durations();
    Ok(doc)
}

fn qrc_inline_title(line: &str) -> Option<String> {
    let inner = line.strip_prefix('[')?.strip_suffix(']')?.trim();
    if inner.is_empty()
        || inner.contains(':')
        || inner.contains(',')
        || inner.chars().all(|ch| ch.is_ascii_digit())
    {
        return None;
    }
    Some(inner.to_string())
}

fn parse_qrc_words(
    body: &str,
    line_start_ms: u32,
    prefix_word_re: &Regex,
) -> Result<Vec<LyricWord>> {
    if body.trim_start().starts_with('(') {
        let prefix_words = collect_prefix_words(body, line_start_ms, prefix_word_re)?;
        if !prefix_words.is_empty() {
            return Ok(prefix_words);
        }
    }

    collect_postfix_words(body, line_start_ms)
}

fn collect_prefix_words(body: &str, line_start_ms: u32, word_re: &Regex) -> Result<Vec<LyricWord>> {
    let mut words = Vec::new();
    for caps in word_re.captures_iter(body) {
        let absolute_start = parse_u32(&caps, 1)?;
        let text = caps
            .get(3)
            .map(|m| m.as_str())
            .unwrap_or_default()
            .to_string();
        if text.is_empty() {
            continue;
        }

        words.push(LyricWord {
            offset_ms: absolute_start.saturating_sub(line_start_ms),
            duration_ms: parse_u32(&caps, 2)?,
            text,
        });
    }
    Ok(words)
}

fn collect_postfix_words(body: &str, line_start_ms: u32) -> Result<Vec<LyricWord>> {
    let mut words = Vec::new();
    let bytes = body.as_bytes();
    let mut cursor = 0usize;
    let mut text_start = 0usize;

    while cursor < bytes.len() {
        if bytes[cursor] != b'(' {
            cursor += 1;
            continue;
        }

        let Some((absolute_start, duration_ms, tag_end)) = parse_qrc_word_tag_at(body, cursor)
        else {
            cursor += 1;
            continue;
        };

        let text = body[text_start..cursor].to_string();
        if !text.is_empty() {
            words.push(LyricWord {
                offset_ms: absolute_start.saturating_sub(line_start_ms),
                duration_ms,
                text,
            });
        }

        cursor = tag_end;
        text_start = tag_end;
    }

    Ok(words)
}

fn parse_qrc_word_tag_at(body: &str, start: usize) -> Option<(u32, u32, usize)> {
    let tail = body.get(start..)?;
    let end_offset = tail.find(')')?;
    let inner = tail.get(1..end_offset)?;
    let mut parts = inner.split(',');
    let start_ms = parts.next()?.parse::<u32>().ok()?;
    let duration_ms = parts.next()?.parse::<u32>().ok()?;
    if !parts.all(|part| !part.is_empty() && part.chars().all(|ch| ch.is_ascii_digit())) {
        return None;
    }
    Some((start_ms, duration_ms, start + end_offset + 1))
}

fn encrypted_candidates(payload: &[u8]) -> Vec<Vec<u8>> {
    let text = String::from_utf8_lossy(payload);
    let compact: String = text.chars().filter(|ch| !ch.is_whitespace()).collect();

    if looks_like_hex(&compact) {
        if let Ok(decoded) = hex::decode(compact) {
            return vec![decoded];
        }
    }

    vec![payload.to_vec()]
}

fn decrypt_client_qrc_payload(payload: &[u8]) -> Result<String> {
    if !payload.starts_with(QRC_QMC_MAGIC) {
        return Err(Error::Decode("not a QQMusic client QRC payload".into()));
    }

    let mut data = qmc_decode(payload);
    data.drain(..QRC_QMC_MAGIC.len());

    if data.len() < 8 || !data.len().is_multiple_of(8) {
        return Err(Error::Decode(format!(
            "QQMusic client QRC payload has invalid DES length {}",
            data.len()
        )));
    }

    qrc_des_transform_bytes(&mut data, CLIENT_DES_KEY1, false)?;
    qrc_des_transform_bytes(&mut data, CLIENT_DES_KEY2, true)?;
    qrc_des_transform_bytes(&mut data, CLIENT_DES_KEY3, false)?;

    let inflated = inflate_any(&data)?;
    let text = String::from_utf8_lossy(&inflated)
        .trim_start_matches('\u{feff}')
        .trim_matches(char::from(0))
        .to_string();
    if text.contains("<?xml") || text.contains('[') {
        Ok(text)
    } else {
        Err(Error::Decode(
            "QQMusic client QRC decoded payload did not look like lyric text".into(),
        ))
    }
}

fn qmc_decode(payload: &[u8]) -> Vec<u8> {
    payload
        .iter()
        .enumerate()
        .map(|(offset, byte)| byte ^ qmc_mask(offset))
        .collect()
}

fn qmc_mask(offset: usize) -> u8 {
    if offset > 0x7fff {
        QMC1_KEY[(offset % 0x7fff) & 0x7f]
    } else {
        QMC1_KEY[offset & 0x7f]
    }
}

fn qrc_des_transform_bytes(data: &mut [u8], key: &[u8; 8], mode_encrypt: bool) -> Result<()> {
    if !data.len().is_multiple_of(8) {
        return Err(Error::Decode(format!(
            "QQMusic client QRC DES input length {} is not a multiple of 8",
            data.len()
        )));
    }

    let subkeys = qrc_des_subkeys(key, mode_encrypt);
    for block in data.chunks_exact_mut(8) {
        let value = u64::from_le_bytes(block.try_into().expect("DES block is exactly 8 bytes"));
        let value = qrc_des_transform_block(value, &subkeys);
        block.copy_from_slice(&value.to_le_bytes());
    }
    Ok(())
}

fn qrc_des_subkeys(key_bytes: &[u8; 8], mode_encrypt: bool) -> [u64; 16] {
    let key = u64::from_le_bytes(*key_bytes);
    let param = qrc_map_u64(key, KEY_PERMUTATION_TABLE);
    let mut param_c = qrc_u64_lo32(param);
    let mut param_d = qrc_u64_hi32(param);
    let mut subkeys = [0_u64; 16];

    for (index, shift_left) in KEY_RND_SHIFTS.iter().copied().enumerate() {
        let subkey_index = if mode_encrypt { index } else { 15 - index };
        qrc_update_param(&mut param_c, shift_left);
        qrc_update_param(&mut param_d, shift_left);
        subkeys[subkey_index] = qrc_map_u64(qrc_make_u64(param_d, param_c), KEY_COMPRESSION);
    }

    subkeys
}

fn qrc_des_transform_block(data: u64, subkeys: &[u64; 16]) -> u64 {
    let mut state = qrc_map_u64(data, IP);
    for key in subkeys {
        state = qrc_des_crypt_proc(state, *key);
    }
    state = qrc_swap_u64_side(state);
    qrc_map_u64(state, IP_INV)
}

fn qrc_des_crypt_proc(state: u64, key: u64) -> u64 {
    let state_hi32 = qrc_u64_hi32(state);
    let state_lo32 = qrc_u64_lo32(state);

    let expanded = qrc_map_u64(qrc_make_u64(state_hi32, state_hi32), KEY_EXPANSION) ^ key;
    let mut next_lo32 = qrc_sbox_transform(expanded);
    next_lo32 = qrc_map_u32_bits(next_lo32, P_BOX);
    next_lo32 ^= state_lo32;

    qrc_make_u64(next_lo32, state_hi32)
}

fn qrc_sbox_transform(state: u64) -> u32 {
    let mut result = 0_u32;
    for (index, shift) in LARGE_STATE_SHIFTS.iter().copied().enumerate() {
        let sbox_index = ((state >> u64::from(shift)) & 0b111111) as usize;
        result = (result << 4) | u32::from(SBOXES[index][sbox_index]);
    }
    result
}

fn qrc_update_param(param: &mut u32, shift_left: u8) {
    let shift_right = 28 - shift_left;
    *param = (*param << shift_left) | ((*param >> shift_right) & 0xfffffff0);
}

fn qrc_make_u64(hi32: u32, lo32: u32) -> u64 {
    (u64::from(hi32) << 32) | u64::from(lo32)
}

fn qrc_swap_u64_side(value: u64) -> u64 {
    value.rotate_left(32)
}

fn qrc_u64_lo32(value: u64) -> u32 {
    value as u32
}

fn qrc_u64_hi32(value: u64) -> u32 {
    (value >> 32) as u32
}

fn qrc_map_u32_bits(src_value: u32, table: &[u8]) -> u32 {
    let mut result = 0_u64;
    for (index, value) in table.iter().copied().enumerate() {
        qrc_map_bit(&mut result, u64::from(src_value), value, index as u8);
    }
    result as u32
}

fn qrc_map_u64(src_value: u64, table: &[u8]) -> u64 {
    let mid_index = table.len() / 2;
    let mut lo32 = 0_u64;
    let mut hi32 = 0_u64;

    for (index, value) in table[..mid_index].iter().copied().enumerate() {
        qrc_map_bit(&mut lo32, src_value, value, index as u8);
    }
    for (index, value) in table[mid_index..].iter().copied().enumerate() {
        qrc_map_bit(&mut hi32, src_value, value, index as u8);
    }

    qrc_make_u64(hi32 as u32, lo32 as u32)
}

fn qrc_map_bit(result: &mut u64, src: u64, check: u8, set: u8) {
    if (qrc_shift_mask(check) & src) != 0 {
        *result |= qrc_shift_mask(set);
    }
}

fn qrc_shift_mask(value: u8) -> u64 {
    let index = value & 0x3f;
    if index < 32 {
        1_u64 << (31 - index)
    } else {
        1_u64 << (95 - index)
    }
}

const KEY_RND_SHIFTS: &[u8] = &[1, 1, 2, 2, 2, 2, 2, 2, 1, 2, 2, 2, 2, 2, 2, 1];
const LARGE_STATE_SHIFTS: &[u8] = &[0x1a, 0x14, 0x0e, 0x08, 0x3a, 0x34, 0x2e, 0x28];
const SBOXES: [[u8; 64]; 8] = [
    [
        14, 0, 4, 15, 13, 7, 1, 4, 2, 14, 15, 2, 11, 13, 8, 1, 3, 10, 10, 6, 6, 12, 12, 11, 5, 9,
        9, 5, 0, 3, 7, 8, 4, 15, 1, 12, 14, 8, 8, 2, 13, 4, 6, 9, 2, 1, 11, 7, 15, 5, 12, 11, 9, 3,
        7, 14, 3, 10, 10, 0, 5, 6, 0, 13,
    ],
    [
        15, 3, 1, 13, 8, 4, 14, 7, 6, 15, 11, 2, 3, 8, 4, 15, 9, 12, 7, 0, 2, 1, 13, 10, 12, 6, 0,
        9, 5, 11, 10, 5, 0, 13, 14, 8, 7, 10, 11, 1, 10, 3, 4, 15, 13, 4, 1, 2, 5, 11, 8, 6, 12, 7,
        6, 12, 9, 0, 3, 5, 2, 14, 15, 9,
    ],
    [
        10, 13, 0, 7, 9, 0, 14, 9, 6, 3, 3, 4, 15, 6, 5, 10, 1, 2, 13, 8, 12, 5, 7, 14, 11, 12, 4,
        11, 2, 15, 8, 1, 13, 1, 6, 10, 4, 13, 9, 0, 8, 6, 15, 9, 3, 8, 0, 7, 11, 4, 1, 15, 2, 14,
        12, 3, 5, 11, 10, 5, 14, 2, 7, 12,
    ],
    [
        7, 13, 13, 8, 14, 11, 3, 5, 0, 6, 6, 15, 9, 0, 10, 3, 1, 4, 2, 7, 8, 2, 5, 12, 11, 1, 12,
        10, 4, 14, 15, 9, 10, 3, 6, 15, 9, 0, 0, 6, 12, 10, 11, 10, 7, 13, 13, 8, 15, 9, 1, 4, 3,
        5, 14, 11, 5, 12, 2, 7, 8, 2, 4, 14,
    ],
    [
        2, 14, 12, 11, 4, 2, 1, 12, 7, 4, 10, 7, 11, 13, 6, 1, 8, 5, 5, 0, 3, 15, 15, 10, 13, 3, 0,
        9, 14, 8, 9, 6, 4, 11, 2, 8, 1, 12, 11, 7, 10, 1, 13, 14, 7, 2, 8, 13, 15, 6, 9, 15, 12, 0,
        5, 9, 6, 10, 3, 4, 0, 5, 14, 3,
    ],
    [
        12, 10, 1, 15, 10, 4, 15, 2, 9, 7, 2, 12, 6, 9, 8, 5, 0, 6, 13, 1, 3, 13, 4, 14, 14, 0, 7,
        11, 5, 3, 11, 8, 9, 4, 14, 3, 15, 2, 5, 12, 2, 9, 8, 5, 12, 15, 3, 10, 7, 11, 0, 14, 4, 1,
        10, 7, 1, 6, 13, 0, 11, 8, 6, 13,
    ],
    [
        4, 13, 11, 0, 2, 11, 14, 7, 15, 4, 0, 9, 8, 1, 13, 10, 3, 14, 12, 3, 9, 5, 7, 12, 5, 2, 10,
        15, 6, 8, 1, 6, 1, 6, 4, 11, 11, 13, 13, 8, 12, 1, 3, 4, 7, 10, 14, 7, 10, 9, 15, 5, 6, 0,
        8, 15, 0, 14, 5, 2, 9, 3, 2, 12,
    ],
    [
        13, 1, 2, 15, 8, 13, 4, 8, 6, 10, 15, 3, 11, 7, 1, 4, 10, 12, 9, 5, 3, 6, 14, 11, 5, 0, 0,
        14, 12, 9, 7, 2, 7, 2, 11, 1, 4, 14, 1, 7, 9, 4, 12, 10, 14, 8, 2, 13, 0, 15, 6, 12, 10, 9,
        13, 0, 15, 3, 3, 5, 5, 6, 8, 11,
    ],
];
const P_BOX: &[u8] = &[
    0x0f, 0x06, 0x13, 0x14, 0x1c, 0x0b, 0x1b, 0x10, 0x00, 0x0e, 0x16, 0x19, 0x04, 0x11, 0x1e, 0x09,
    0x01, 0x07, 0x17, 0x0d, 0x1f, 0x1a, 0x02, 0x08, 0x12, 0x0c, 0x1d, 0x05, 0x15, 0x0a, 0x03, 0x18,
];
const IP: &[u8] = &[
    0x39, 0x31, 0x29, 0x21, 0x19, 0x11, 0x09, 0x01, 0x3b, 0x33, 0x2b, 0x23, 0x1b, 0x13, 0x0b, 0x03,
    0x3d, 0x35, 0x2d, 0x25, 0x1d, 0x15, 0x0d, 0x05, 0x3f, 0x37, 0x2f, 0x27, 0x1f, 0x17, 0x0f, 0x07,
    0x38, 0x30, 0x28, 0x20, 0x18, 0x10, 0x08, 0x00, 0x3a, 0x32, 0x2a, 0x22, 0x1a, 0x12, 0x0a, 0x02,
    0x3c, 0x34, 0x2c, 0x24, 0x1c, 0x14, 0x0c, 0x04, 0x3e, 0x36, 0x2e, 0x26, 0x1e, 0x16, 0x0e, 0x06,
];
const IP_INV: &[u8] = &[
    0x27, 0x07, 0x2f, 0x0f, 0x37, 0x17, 0x3f, 0x1f, 0x26, 0x06, 0x2e, 0x0e, 0x36, 0x16, 0x3e, 0x1e,
    0x25, 0x05, 0x2d, 0x0d, 0x35, 0x15, 0x3d, 0x1d, 0x24, 0x04, 0x2c, 0x0c, 0x34, 0x14, 0x3c, 0x1c,
    0x23, 0x03, 0x2b, 0x0b, 0x33, 0x13, 0x3b, 0x1b, 0x22, 0x02, 0x2a, 0x0a, 0x32, 0x12, 0x3a, 0x1a,
    0x21, 0x01, 0x29, 0x09, 0x31, 0x11, 0x39, 0x19, 0x20, 0x00, 0x28, 0x08, 0x30, 0x10, 0x38, 0x18,
];
const KEY_PERMUTATION_TABLE: &[u8] = &[
    0x38, 0x30, 0x28, 0x20, 0x18, 0x10, 0x08, 0x00, 0x39, 0x31, 0x29, 0x21, 0x19, 0x11, 0x09, 0x01,
    0x3a, 0x32, 0x2a, 0x22, 0x1a, 0x12, 0x0a, 0x02, 0x3b, 0x33, 0x2b, 0x23, 0x3e, 0x36, 0x2e, 0x26,
    0x1e, 0x16, 0x0e, 0x06, 0x3d, 0x35, 0x2d, 0x25, 0x1d, 0x15, 0x0d, 0x05, 0x3c, 0x34, 0x2c, 0x24,
    0x1c, 0x14, 0x0c, 0x04, 0x1b, 0x13, 0x0b, 0x03,
];
const KEY_COMPRESSION: &[u8] = &[
    0x0d, 0x10, 0x0a, 0x17, 0x00, 0x04, 0x02, 0x1b, 0x0e, 0x05, 0x14, 0x09, 0x16, 0x12, 0x0b, 0x03,
    0x19, 0x07, 0x0f, 0x06, 0x1a, 0x13, 0x0c, 0x01, 0x2d, 0x38, 0x23, 0x29, 0x33, 0x3b, 0x22, 0x2c,
    0x37, 0x31, 0x25, 0x34, 0x30, 0x35, 0x2b, 0x3c, 0x26, 0x39, 0x32, 0x2e, 0x36, 0x28, 0x21, 0x24,
];
const KEY_EXPANSION: &[u8] = &[
    0x1f, 0x00, 0x01, 0x02, 0x03, 0x04, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x07, 0x08, 0x09, 0x0a,
    0x0b, 0x0c, 0x0b, 0x0c, 0x0d, 0x0e, 0x0f, 0x10, 0x0f, 0x10, 0x11, 0x12, 0x13, 0x14, 0x13, 0x14,
    0x15, 0x16, 0x17, 0x18, 0x17, 0x18, 0x19, 0x1a, 0x1b, 0x1c, 0x1b, 0x1c, 0x1d, 0x1e, 0x1f, 0x00,
];

fn des_candidates(bytes: &[u8]) -> Vec<Vec<u8>> {
    let mut attempts = Vec::new();

    if let Some(data) = try_qrc_des_decrypt(bytes) {
        attempts.push(data);
    }
    if let Some(data) = try_tdes_3key_decrypt(bytes, QQ_LYRIC_3DES_KEY) {
        attempts.push(data);
    }
    if let Some(data) = try_three_stage(bytes, StageMode::DdesDesDdes) {
        attempts.push(data);
    }
    if let Some(data) = try_three_stage(bytes, StageMode::TdesTdesTdes) {
        attempts.push(data);
    }
    if let Some(data) = try_three_stage(bytes, StageMode::DdesDesEncryptDdes) {
        attempts.push(data);
    }

    attempts
}

fn try_qrc_des_decrypt(bytes: &[u8]) -> Option<Vec<u8>> {
    if bytes.len() < 8 || !bytes.len().is_multiple_of(8) {
        return None;
    }

    let mut out = bytes.to_vec();
    qrc_des_transform_bytes(&mut out, CLIENT_DES_KEY1, false).ok()?;
    qrc_des_transform_bytes(&mut out, CLIENT_DES_KEY2, true).ok()?;
    qrc_des_transform_bytes(&mut out, CLIENT_DES_KEY3, false).ok()?;
    Some(out)
}

fn try_tdes_3key_decrypt(bytes: &[u8], key24: &[u8; 24]) -> Option<Vec<u8>> {
    if bytes.len() < 8 || !bytes.len().is_multiple_of(8) {
        return None;
    }

    let mut out = bytes.to_vec();
    let cipher = TdesEde3::new_from_slice(key24).ok()?;
    for block in out.chunks_exact_mut(8) {
        cipher.decrypt_block(GenericArray::from_mut_slice(block));
    }
    Some(out)
}

#[derive(Debug, Clone, Copy)]
enum StageMode {
    DdesDesDdes,
    TdesTdesTdes,
    DdesDesEncryptDdes,
}

fn try_three_stage(bytes: &[u8], mode: StageMode) -> Option<Vec<u8>> {
    if bytes.len() < 8 || !bytes.len().is_multiple_of(8) {
        return None;
    }

    let mut out = bytes.to_vec();
    match mode {
        StageMode::DdesDesDdes => {
            tdes_2key_decrypt_in_place(&mut out, KEY1).ok()?;
            des_decrypt_in_place(&mut out, &KEY2[..8]).ok()?;
            tdes_2key_decrypt_in_place(&mut out, KEY3).ok()?;
        }
        StageMode::TdesTdesTdes => {
            tdes_2key_decrypt_in_place(&mut out, KEY1).ok()?;
            tdes_2key_decrypt_in_place(&mut out, KEY2).ok()?;
            tdes_2key_decrypt_in_place(&mut out, KEY3).ok()?;
        }
        StageMode::DdesDesEncryptDdes => {
            tdes_2key_decrypt_in_place(&mut out, KEY1).ok()?;
            des_encrypt_in_place(&mut out, &KEY2[..8]).ok()?;
            tdes_2key_decrypt_in_place(&mut out, KEY3).ok()?;
        }
    }

    Some(out)
}

fn tdes_2key_decrypt_in_place(data: &mut [u8], key16: &[u8; 16]) -> Result<()> {
    let mut key24 = [0_u8; 24];
    key24[..16].copy_from_slice(key16);
    key24[16..].copy_from_slice(&key16[..8]);

    let cipher = TdesEde3::new_from_slice(&key24)
        .map_err(|err| Error::Decode(format!("invalid 3DES key: {err}")))?;
    for block in data.chunks_exact_mut(8) {
        cipher.decrypt_block(GenericArray::from_mut_slice(block));
    }
    Ok(())
}

fn des_decrypt_in_place(data: &mut [u8], key8: &[u8]) -> Result<()> {
    let cipher = Des::new_from_slice(key8)
        .map_err(|err| Error::Decode(format!("invalid DES key: {err}")))?;
    for block in data.chunks_exact_mut(8) {
        cipher.decrypt_block(GenericArray::from_mut_slice(block));
    }
    Ok(())
}

fn des_encrypt_in_place(data: &mut [u8], key8: &[u8]) -> Result<()> {
    let cipher = Des::new_from_slice(key8)
        .map_err(|err| Error::Decode(format!("invalid DES key: {err}")))?;
    for block in data.chunks_exact_mut(8) {
        cipher.encrypt_block(GenericArray::from_mut_slice(block));
    }
    Ok(())
}

fn inflate_any(bytes: &[u8]) -> Result<Vec<u8>> {
    let mut output = Vec::new();

    if ZlibDecoder::new(bytes).read_to_end(&mut output).is_ok() {
        return Ok(trim_padding(output));
    }

    output.clear();
    if DeflateDecoder::new(bytes).read_to_end(&mut output).is_ok() {
        return Ok(trim_padding(output));
    }

    output.clear();
    if GzDecoder::new(bytes).read_to_end(&mut output).is_ok() {
        return Ok(trim_padding(output));
    }

    Err(Error::Decode("QRC inflate failed".into()))
}

fn trim_padding(mut data: Vec<u8>) -> Vec<u8> {
    while data.last().is_some_and(|byte| *byte == 0) {
        data.pop();
    }
    data
}

fn parse_u32(caps: &regex::Captures<'_>, index: usize) -> Result<u32> {
    caps.get(index)
        .ok_or_else(|| Error::Parse(format!("missing capture {index}")))?
        .as_str()
        .parse::<u32>()
        .map_err(|err| Error::Parse(err.to_string()))
}

fn strip_qrc_word_tags(body: &str) -> String {
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

fn is_qrc_line(line: &str) -> bool {
    let trimmed = line.trim();
    trimmed.starts_with('[')
        && trimmed.contains(',')
        && trimmed.contains(']')
        && trimmed.contains('(')
        && trimmed.contains(')')
}

fn is_lrc_line(line: &str) -> bool {
    let trimmed = line.trim();
    trimmed.starts_with('[') && trimmed.contains(':') && trimmed.contains(']')
}

fn looks_like_qrc_xml(text: &str) -> bool {
    text.starts_with("<?xml")
        || text.starts_with("<QrcInfos")
        || text.contains("<LyricInfo")
        || text.contains("LyricContent=")
        || (text.contains("<content") && text.contains("<![CDATA["))
        || (text.starts_with("<!--")
            && text.contains("<command-lable")
            && text.contains("<content"))
}

fn looks_like_hex(value: &str) -> bool {
    let compact: String = value.chars().filter(|ch| !ch.is_whitespace()).collect();
    compact.len() >= 16
        && compact.len().is_multiple_of(2)
        && compact.chars().all(|ch| ch.is_ascii_hexdigit())
}

fn looks_like_base64(value: &str) -> bool {
    value.len() >= 8
        && value.len().is_multiple_of(4)
        && value
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '+' | '/' | '='))
}

fn xml_unescape(value: &str) -> String {
    value
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use flate2::write::ZlibEncoder;
    use flate2::Compression;

    use super::*;

    #[test]
    fn parses_qrc_prefix_word_lines() {
        let doc = parse_lyric_content("[1000,900](1000,300,0)你(1300,600,0)好\n").unwrap();

        assert_eq!(doc.lines[0].text, "你好");
        assert_eq!(doc.lines[0].words[1].offset_ms, 300);
    }

    #[test]
    fn parses_qrc_postfix_word_lines() {
        let doc = parse_lyric_content("[1000,900]你(1000,300)好(1300,600)\n").unwrap();

        assert_eq!(doc.lines[0].text, "你好");
        assert_eq!(doc.lines[0].words[1].duration_ms, 600);
    }

    #[test]
    fn preserves_parenthesized_qrc_postfix_text() {
        let doc = parse_lyric_content(
            "[0,20098]龙(0,1827)战(1827,1827)骑(3654,1827)士(5482,1827) - (7309,1827)周(9136,1827)杰(10963,1827)伦(12790,1827) ((14617,1827)Jay (16445,1827)Chou)(18272,1827)\n",
        )
        .unwrap();

        assert_eq!(doc.lines[0].text, "龙战骑士 - 周杰伦 (Jay Chou)");
        assert_eq!(doc.lines[0].words[4].text, " - ");
        assert_eq!(doc.lines[0].words[8].text, " (");
        assert_eq!(doc.lines[0].words[10].text, "Chou)");
    }

    #[test]
    fn parses_qrc_inline_title_line() {
        let doc =
            parse_lyric_content("[龙战骑士 - 周杰伦（Jay）]\n[1000,900]你(1000,300)好(1300,600)\n")
                .unwrap();

        assert_eq!(doc.meta.title.as_deref(), Some("龙战骑士 - 周杰伦（Jay）"));
        assert_eq!(doc.lines[0].text, "你好");
    }

    #[test]
    fn ignores_numeric_bracket_lines_as_qrc_titles() {
        let doc = parse_lyric_content("[12345]\n[1000,900]你(1000,300)好(1300,600)\n").unwrap();

        assert_eq!(doc.meta.title, None);
        assert_eq!(doc.lines[0].text, "你好");
    }

    #[test]
    fn extracts_xml_lyric_content() {
        let xml = r#"<?xml version="1.0"?><QrcInfos><LyricInfo><Lyric_1 LyricContent="[1000,500](1000,500,0)Hi"/></LyricInfo></QrcInfos>"#;
        let doc = decode(xml.as_bytes()).unwrap();

        assert_eq!(doc.lines[0].text, "Hi");
    }

    #[test]
    fn extracts_base64_wrapped_xml_lyric_content_for_source_raw() {
        let wrapped =
            base64::engine::general_purpose::STANDARD.encode("[1000,500](1000,500,0)Hi\n");
        let xml = format!(
            r#"<?xml version="1.0"?><QrcInfos><LyricInfo><Lyric_1 LyricContent="{wrapped}"/></LyricInfo></QrcInfos>"#
        );
        let raw = decode_raw_lyric_content(xml.as_bytes()).unwrap();

        assert_eq!(raw, "[1000,500](1000,500,0)Hi\n");
    }

    #[test]
    fn decodes_qq_download_xml_cdata_content() {
        let encrypted = encode_legacy_qrc_hex_for_test("[1000,500](1000,500,0)Hi\n");
        let xml = format!(
            r#"<!--
<command-lable-xwl78-qq-music><content type="file"><![CDATA[{encrypted}]]></content></command-lable-xwl78-qq-music>
-->"#
        );
        let doc = decode(xml.as_bytes()).unwrap();

        assert_eq!(doc.lines[0].text, "Hi");
    }

    #[test]
    fn qrc_des_matches_reference_vector() {
        let mut encrypted = vec![
            0xfd, 0x0e, 0x64, 0x06, 0x65, 0xbe, 0x74, 0x13, 0x77, 0x63, 0x3b, 0x02, 0x45, 0x4e,
            0x70, 0x7a,
        ];
        qrc_des_transform_bytes(&mut encrypted, b"TEST!KEY", true).unwrap();
        assert_eq!(
            encrypted,
            vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 0, 1, 2, 3, 4, 5, 6]
        );

        qrc_des_transform_bytes(&mut encrypted, b"TEST!KEY", false).unwrap();
        assert_eq!(
            encrypted,
            vec![
                0xfd, 0x0e, 0x64, 0x06, 0x65, 0xbe, 0x74, 0x13, 0x77, 0x63, 0x3b, 0x02, 0x45, 0x4e,
                0x70, 0x7a,
            ]
        );
    }

    #[test]
    fn decodes_client_qrc_payload() {
        let payload = encode_client_qrc_for_test("[00:01.00]Hi\n");
        let doc = decode(&payload).unwrap();

        assert_eq!(doc.lines[0].start_ms, 1000);
        assert_eq!(doc.lines[0].text, "Hi");
    }

    fn encode_client_qrc_for_test(plain: &str) -> Vec<u8> {
        let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(plain.as_bytes()).unwrap();
        let mut compressed = encoder.finish().unwrap();
        while !compressed.len().is_multiple_of(8) {
            compressed.push(0);
        }

        qrc_des_transform_bytes(&mut compressed, CLIENT_DES_KEY3, true).unwrap();
        qrc_des_transform_bytes(&mut compressed, CLIENT_DES_KEY2, false).unwrap();
        qrc_des_transform_bytes(&mut compressed, CLIENT_DES_KEY1, true).unwrap();

        let mut payload = QRC_QMC_MAGIC.to_vec();
        payload.extend(
            compressed
                .into_iter()
                .enumerate()
                .map(|(index, byte)| byte ^ qmc_mask(index + QRC_QMC_MAGIC.len())),
        );
        payload
    }

    fn encode_legacy_qrc_hex_for_test(plain: &str) -> String {
        let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(plain.as_bytes()).unwrap();
        let mut compressed = encoder.finish().unwrap();
        while !compressed.len().is_multiple_of(8) {
            compressed.push(0);
        }

        qrc_des_transform_bytes(&mut compressed, CLIENT_DES_KEY3, true).unwrap();
        qrc_des_transform_bytes(&mut compressed, CLIENT_DES_KEY2, false).unwrap();
        qrc_des_transform_bytes(&mut compressed, CLIENT_DES_KEY1, true).unwrap();

        hex::encode_upper(compressed)
    }
}
