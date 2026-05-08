use thiserror::Error;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Error)]
pub enum Error {
    #[error("unknown or unsupported lyric format")]
    UnknownFormat,

    #[error("decode failed: {0}")]
    Decode(String),

    #[error("parse failed: {0}")]
    Parse(String),

    #[error("provider failed: {0}")]
    Provider(String),

    #[error("service failed: {0}")]
    Service(String),

    #[error("storage failed: {0}")]
    Storage(String),

    #[error("network request failed: {0}")]
    Network(#[from] reqwest::Error),

    #[error("sqlite failed: {0}")]
    Sqlite(#[from] rusqlite::Error),

    #[error("io failed: {0}")]
    Io(#[from] std::io::Error),

    #[error("json failed: {0}")]
    Json(#[from] serde_json::Error),
}
