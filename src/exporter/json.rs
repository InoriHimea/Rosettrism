use crate::model::LyricDocument;
use crate::Result;

pub fn to_vec(document: &LyricDocument) -> Result<Vec<u8>> {
    Ok(serde_json::to_vec_pretty(document)?)
}
