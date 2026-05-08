use clap::ValueEnum;

use crate::model::LyricDocument;
use crate::Result;

pub mod json;
pub mod lrc;

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum OutputFormat {
    Lrc,
    Json,
    Raw,
}

pub fn export_document(document: &LyricDocument, format: OutputFormat) -> Result<Vec<u8>> {
    match format {
        OutputFormat::Lrc => Ok(lrc::to_string(document).into_bytes()),
        OutputFormat::Json => json::to_vec(document),
        OutputFormat::Raw => Ok(Vec::new()),
    }
}
