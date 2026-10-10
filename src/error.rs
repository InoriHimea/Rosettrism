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

    #[error("blocking task failed: {0}")]
    Task(#[from] tokio::task::JoinError),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn string_variants_render_prefix_messages() {
        assert_eq!(
            Error::UnknownFormat.to_string(),
            "unknown or unsupported lyric format"
        );
        assert_eq!(
            Error::Decode("bad krc body".into()).to_string(),
            "decode failed: bad krc body"
        );
        assert_eq!(
            Error::Parse("unbalanced tag".into()).to_string(),
            "parse failed: unbalanced tag"
        );
        assert_eq!(
            Error::Provider("kugou 503".into()).to_string(),
            "provider failed: kugou 503"
        );
        assert_eq!(
            Error::Service("no sources".into()).to_string(),
            "service failed: no sources"
        );
        assert_eq!(
            Error::Storage("db locked".into()).to_string(),
            "storage failed: db locked"
        );
    }

    #[test]
    fn io_error_converts_via_from() {
        let error = Error::from(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "file gone",
        ));
        assert!(matches!(error, Error::Io(_)));
        assert!(error.to_string().starts_with("io failed: "));
    }

    #[test]
    fn serde_json_error_converts_via_from() {
        let json_error = serde_json::from_str::<serde_json::Value>("{").unwrap_err();
        let error = Error::from(json_error);
        assert!(matches!(error, Error::Json(_)));
        assert!(error.to_string().starts_with("json failed: "));
    }

    #[tokio::test]
    async fn join_error_converts_to_task_variant() {
        let joined: std::result::Result<(), tokio::task::JoinError> =
            tokio::task::spawn_blocking(|| -> () {
                panic!("boom");
            })
            .await;
        let join_error = joined.unwrap_err();
        let error = Error::from(join_error);
        assert!(matches!(error, Error::Task(_)));
        assert!(error.to_string().starts_with("blocking task failed: "));
    }
}
