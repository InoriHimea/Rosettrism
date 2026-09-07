pub mod apple_music;
mod detect;
pub mod krc;
pub mod lrc;
pub mod qrc;
pub mod singing_annotations;
pub mod text;
pub mod yrc;

use clap::ValueEnum;
use serde::{Deserialize, Serialize};

use crate::model::LyricDocument;
use crate::{Error, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ValueEnum)]
pub enum InputFormat {
    Auto,
    #[value(alias = "ttml")]
    AppleMusic,
    Json,
    Krc,
    Qrc,
    Text,
    Yrc,
    Lrc,
}

impl InputFormat {
    /// CLI/API 兼容层的稳定格式标识（与 `DecoderPlugin::format_id` 对齐）。
    pub fn format_id(self) -> &'static str {
        match self {
            InputFormat::Auto => "auto",
            InputFormat::AppleMusic => "apple_music",
            InputFormat::Json => "json",
            InputFormat::Krc => "krc",
            InputFormat::Qrc => "qrc",
            InputFormat::Text => "text",
            InputFormat::Yrc => "yrc",
            InputFormat::Lrc => "lrc",
        }
    }
}

/// 一个 decoder 插件 = 稳定格式标识 + 字节嗅探 + 解码入口。
///
/// 内置格式与未来外部插件走同一契约（设计见
/// `docs/provider-plugin-manifest.md` §3.2）。
pub trait DecoderPlugin: Send + Sync {
    /// 稳定格式标识；与 `InputFormat::format_id` 及未来外部 manifest 对齐。
    fn format_id(&self) -> &'static str;

    /// 字节特征嗅探；返回置信度权重（0 = 不匹配）。
    /// 注册表按最高分裁决；平分时注册顺序靠前者胜出。
    fn detect(&self, bytes: &[u8]) -> u32;

    /// 解码为 `LyricDocument`。
    fn decode(&self, bytes: &[u8]) -> Result<LyricDocument>;

    /// 原始解码（如 QRC/KRC 的解密直出）；默认直返原文。
    fn decode_raw(&self, bytes: &[u8]) -> Result<Vec<u8>> {
        Ok(bytes.to_vec())
    }
}

/// 注册表：按 `format_id` 查找与嗅探裁决。
pub struct DecoderRegistry {
    plugins: Vec<std::sync::Arc<dyn DecoderPlugin>>,
    by_format_id: std::collections::HashMap<&'static str, usize>,
}

impl DecoderRegistry {
    /// 由插件序列构建；`format_id` 重复时返回错误。
    pub fn new(plugins: Vec<std::sync::Arc<dyn DecoderPlugin>>) -> Result<Self> {
        let mut by_format_id = std::collections::HashMap::new();
        for (index, plugin) in plugins.iter().enumerate() {
            if by_format_id.insert(plugin.format_id(), index).is_some() {
                return Err(Error::Service(format!(
                    "duplicate decoder plugin format_id: {}",
                    plugin.format_id()
                )));
            }
        }
        Ok(Self {
            plugins,
            by_format_id,
        })
    }

    /// 内置插件注册表（apple_music/json/krc/qrc/text/yrc/lrc）。
    pub fn builtin() -> Self {
        // 构造失败属编程错误（重复 format_id），CI 立即暴露。
        Self::new(builtin_decoder_plugins())
            .expect("builtin decoder registry has unique format ids")
    }

    pub fn lookup(&self, format_id: &str) -> Option<std::sync::Arc<dyn DecoderPlugin>> {
        self.by_format_id
            .get(format_id)
            .map(|&index| std::sync::Arc::clone(&self.plugins[index]))
    }

    /// 嗅探：返回置信度最高的插件；全零时返回 `None`。
    pub fn detect(&self, bytes: &[u8]) -> Option<std::sync::Arc<dyn DecoderPlugin>> {
        self.plugins
            .iter()
            .map(|plugin| (plugin, plugin.detect(bytes)))
            .max_by_key(|&(_, score)| score)
            .filter(|&(_, score)| score > 0)
            .map(|(plugin, _)| std::sync::Arc::clone(plugin))
    }

    pub fn len(&self) -> usize {
        self.plugins.len()
    }

    pub fn is_empty(&self) -> bool {
        self.plugins.is_empty()
    }
}

fn builtin_decoder_plugins() -> Vec<std::sync::Arc<dyn DecoderPlugin>> {
    use std::sync::Arc;
    vec![
        Arc::new(builtin::AppleMusicDecoder),
        Arc::new(builtin::JsonDecoder),
        Arc::new(builtin::KrcDecoder),
        Arc::new(builtin::QrcDecoder),
        Arc::new(builtin::TextDecoder),
        Arc::new(builtin::YrcDecoder),
        Arc::new(builtin::LrcDecoder),
    ]
}

/// 内置 decoder 插件实现：把原 `decode_bytes` 的 match 臂迁移为独立单元。
mod builtin {
    use super::{DecoderPlugin, LyricDocument};
    use crate::decoder::{apple_music, krc, lrc, qrc, text, yrc};
    use crate::Result;

    fn trimmed_lossy(bytes: &[u8]) -> String {
        let sample_len = bytes.len().min(2048);
        String::from_utf8_lossy(&bytes[..sample_len])
            .trim_start_matches('\u{feff}')
            .trim_start()
            .to_string()
    }

    pub struct AppleMusicDecoder;

    impl DecoderPlugin for AppleMusicDecoder {
        fn format_id(&self) -> &'static str {
            "apple_music"
        }

        fn detect(&self, bytes: &[u8]) -> u32 {
            let value = trimmed_lossy(bytes);
            // TTML 命中得 95：高于 XML-QRC 的 90，保持旧 detect.rs 的
            // 「Apple Music 先于泛用 XML」优先级。
            u32::from(
                (value.starts_with("<?xml") || value.starts_with("<tt"))
                    && value.contains("<tt")
                    && value.contains("<body")
                    && (value.contains("http://www.w3.org/ns/ttml")
                        || value.contains("itunes.apple.com/lyric-ttml-extensions")
                        || value.contains("<p begin=")
                        || value.contains("<span begin=")),
            ) * 95
        }

        fn decode(&self, bytes: &[u8]) -> Result<LyricDocument> {
            let text = String::from_utf8_lossy(bytes);
            apple_music::parse(&text)
        }
    }

    pub struct JsonDecoder;

    impl DecoderPlugin for JsonDecoder {
        fn format_id(&self) -> &'static str {
            "json"
        }

        fn detect(&self, bytes: &[u8]) -> u32 {
            let value = trimmed_lossy(bytes);
            u32::from(
                value.starts_with('{')
                    && value.contains(r#""lines""#)
                    && (value.contains(r#""meta""#) || value.contains(r#""start_ms""#)),
            )
        }

        fn decode(&self, bytes: &[u8]) -> Result<LyricDocument> {
            serde_json::from_slice(bytes).map_err(crate::Error::from)
        }
    }

    pub struct KrcDecoder;

    impl DecoderPlugin for KrcDecoder {
        fn format_id(&self) -> &'static str {
            "krc"
        }

        fn detect(&self, bytes: &[u8]) -> u32 {
            u32::from(bytes.starts_with(b"krc1"))
        }

        fn decode(&self, bytes: &[u8]) -> Result<LyricDocument> {
            krc::decode(bytes)
        }

        fn decode_raw(&self, bytes: &[u8]) -> Result<Vec<u8>> {
            Ok(krc::decode_raw(bytes)?.into_bytes())
        }
    }

    pub struct QrcDecoder;

    impl DecoderPlugin for QrcDecoder {
        fn format_id(&self) -> &'static str {
            "qrc"
        }

        fn detect(&self, bytes: &[u8]) -> u32 {
            if bytes.starts_with(&[
                0x98, 0x25, 0xb0, 0xac, 0xe3, 0x02, 0x83, 0x68, 0xe8, 0xfc, 0x6c,
            ]) {
                return 100;
            }
            let value = trimmed_lossy(bytes);
            // XML-style QRC
            if value.starts_with("<?xml")
                || value.starts_with("<QrcInfos")
                || value.contains("<LyricInfo")
                || value.contains("LyricContent=")
            {
                return 90;
            }
            // 文本头 QRC（旧 detect.rs 逻辑：LRC 风格头 + 逐字元组，
            // 如 `[1000,800](1000,300,0)你`）。
            if (value.starts_with("[ti:") || value.starts_with("[ar:"))
                && value.contains('(')
                && value.contains(',')
                && value.contains(')')
            {
                return 80;
            }
            0
        }

        fn decode(&self, bytes: &[u8]) -> Result<LyricDocument> {
            qrc::decode(bytes)
        }

        fn decode_raw(&self, bytes: &[u8]) -> Result<Vec<u8>> {
            Ok(qrc::decode_raw(bytes)?.into_bytes())
        }
    }

    pub struct TextDecoder;

    impl DecoderPlugin for TextDecoder {
        fn format_id(&self) -> &'static str {
            "text"
        }

        fn detect(&self, _bytes: &[u8]) -> u32 {
            // text 是兜底格式，不参与嗅探；显式指定时可用。
            0
        }

        fn decode(&self, bytes: &[u8]) -> Result<LyricDocument> {
            let text = String::from_utf8_lossy(bytes);
            text::parse(&text)
        }
    }

    pub struct YrcDecoder;

    impl DecoderPlugin for YrcDecoder {
        fn format_id(&self) -> &'static str {
            "yrc"
        }

        fn detect(&self, bytes: &[u8]) -> u32 {
            let value = trimmed_lossy(bytes);
            u32::from(value.lines().any(|line| {
                let line = line.trim_start();
                line.starts_with(r#"{"t":"#) && line.contains(r#""c":"#)
            }))
        }

        fn decode(&self, bytes: &[u8]) -> Result<LyricDocument> {
            let text = String::from_utf8_lossy(bytes);
            yrc::parse(&text)
        }
    }

    pub struct LrcDecoder;

    impl DecoderPlugin for LrcDecoder {
        fn format_id(&self) -> &'static str {
            "lrc"
        }

        fn detect(&self, bytes: &[u8]) -> u32 {
            let value = trimmed_lossy(bytes);
            // 与旧 detect.rs 一致：二进制/XML 命中后，LRC 头但含逐字元组的
            // 交给 QrcDecoder（其 detect 返回更高分），此处不加分。
            if (value.starts_with("[ti:") || value.starts_with("[ar:"))
                && value.contains('(')
                && value.contains(',')
                && value.contains(')')
            {
                return 0;
            }
            u32::from(
                value.starts_with("[ti:")
                    || value.starts_with("[ar:")
                    || value
                        .lines()
                        .any(|line| line.starts_with('[') && line.contains(']')),
            )
        }

        fn decode(&self, bytes: &[u8]) -> Result<LyricDocument> {
            let text = String::from_utf8_lossy(bytes);
            lrc::parse(&text)
        }
    }
}

pub fn detect_format(bytes: &[u8]) -> InputFormat {
    detect::detect(bytes)
}

pub fn decode_bytes(bytes: &[u8], input_format: InputFormat) -> Result<LyricDocument> {
    let registry = DecoderRegistry::builtin();
    let plugin = match input_format {
        InputFormat::Auto => registry.detect(bytes).ok_or(Error::UnknownFormat)?,
        format => registry
            .lookup(format.format_id())
            .ok_or(Error::UnknownFormat)?,
    };
    plugin.decode(bytes)
}

pub fn decode_raw_bytes(bytes: &[u8], input_format: InputFormat) -> Result<Vec<u8>> {
    let registry = DecoderRegistry::builtin();
    let plugin = match input_format {
        InputFormat::Auto => registry.detect(bytes).ok_or(Error::UnknownFormat)?,
        format => registry
            .lookup(format.format_id())
            .ok_or(Error::UnknownFormat)?,
    };
    plugin.decode_raw(bytes)
}

#[cfg(test)]
mod registry_tests {
    use super::*;

    #[test]
    fn builtin_registry_covers_all_non_auto_formats() {
        let registry = DecoderRegistry::builtin();
        assert_eq!(registry.len(), 7);
        for format in InputFormat::value_variants() {
            if matches!(format, InputFormat::Auto) {
                continue;
            }
            assert!(
                registry.lookup(format.format_id()).is_some(),
                "missing builtin decoder plugin for {}",
                format.format_id()
            );
        }
    }

    #[test]
    fn duplicate_format_id_is_rejected() {
        let result = DecoderRegistry::new(vec![
            std::sync::Arc::new(builtin::LrcDecoder),
            std::sync::Arc::new(builtin::LrcDecoder),
        ]);
        assert!(result.is_err());
    }

    #[test]
    fn detect_prefers_ttml_over_generic_xml() {
        let registry = DecoderRegistry::builtin();
        let ttml = br#"<?xml version="1.0"?><tt xmlns="http://www.w3.org/ns/ttml"><body><div><p begin="00:01.000" end="00:02.000">Hi</p></div></body></tt>"#;
        assert_eq!(registry.detect(ttml).unwrap().format_id(), "apple_music");
    }

    #[test]
    fn detect_matches_legacy_detector_on_qrc_text_header() {
        let registry = DecoderRegistry::builtin();
        let qrc = b"[ti:Sample]\n[ar:Tester]\n[1000,800](1000,300,0)\xe4\xbd\xa0";
        assert_eq!(registry.detect(qrc).unwrap().format_id(), "qrc");
        assert_eq!(detect::detect(qrc), InputFormat::Qrc);
    }

    #[test]
    fn detect_returns_none_for_unrecognized_bytes() {
        let registry = DecoderRegistry::builtin();
        assert!(registry.detect(b"\x00\x01\x02 garbage").is_none());
    }
}
