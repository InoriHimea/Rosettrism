pub mod apple_music;
mod detect;
pub mod krc;
pub mod lrc;
pub mod qrc;
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

pub fn detect_format(bytes: &[u8]) -> InputFormat {
    detect::detect(bytes)
}

pub fn decode_bytes(bytes: &[u8], input_format: InputFormat) -> Result<LyricDocument> {
    let detected = match input_format {
        InputFormat::Auto => detect::detect(bytes),
        format => format,
    };

    match detected {
        InputFormat::AppleMusic => {
            let text = String::from_utf8_lossy(bytes);
            apple_music::parse(&text)
        }
        InputFormat::Json => Ok(serde_json::from_slice(bytes)?),
        InputFormat::Krc => krc::decode(bytes),
        InputFormat::Qrc => qrc::decode(bytes),
        InputFormat::Text => {
            let text = String::from_utf8_lossy(bytes);
            text::parse(&text)
        }
        InputFormat::Yrc => {
            let text = String::from_utf8_lossy(bytes);
            yrc::parse(&text)
        }
        InputFormat::Lrc => {
            let text = String::from_utf8_lossy(bytes);
            lrc::parse(&text)
        }
        InputFormat::Auto => Err(Error::UnknownFormat),
    }
}

pub fn decode_raw_bytes(bytes: &[u8], input_format: InputFormat) -> Result<Vec<u8>> {
    let detected = match input_format {
        InputFormat::Auto => detect::detect(bytes),
        format => format,
    };

    match detected {
        InputFormat::AppleMusic => Ok(bytes.to_vec()),
        InputFormat::Json => Ok(bytes.to_vec()),
        InputFormat::Krc => Ok(krc::decode_raw(bytes)?.into_bytes()),
        InputFormat::Qrc => Ok(qrc::decode_raw(bytes)?.into_bytes()),
        InputFormat::Text => Ok(bytes.to_vec()),
        InputFormat::Yrc => Ok(bytes.to_vec()),
        InputFormat::Lrc => Ok(bytes.to_vec()),
        InputFormat::Auto => Err(Error::UnknownFormat),
    }
}
