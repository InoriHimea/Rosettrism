use super::*;

pub(super) type ApiResult<T> = std::result::Result<T, ApiError>;

#[derive(Debug, Serialize)]
pub(super) struct ApiErrorBody {
    pub(super) code: &'static str,
    pub(super) message: String,
    pub(super) details: Option<Value>,
    pub(super) retryable: bool,
}

#[derive(Debug)]
pub(super) struct ApiError {
    pub(super) status: StatusCode,
    pub(super) body: ApiErrorBody,
}

impl ApiError {
    pub(super) fn new(
        status: StatusCode,
        code: &'static str,
        message: impl Into<String>,
        details: Option<Value>,
        retryable: bool,
    ) -> Self {
        Self {
            status,
            body: ApiErrorBody {
                code,
                message: message.into(),
                details,
                retryable,
            },
        }
    }

    pub(super) fn bad_request(message: impl Into<String>) -> Self {
        let message = message.into();
        let code = if message == "cache is disabled" {
            "cache_disabled"
        } else {
            "validation_error"
        };
        Self::new(StatusCode::BAD_REQUEST, code, message, None, false)
    }

    pub(super) fn unauthorized(message: impl Into<String>) -> Self {
        Self::new(
            StatusCode::UNAUTHORIZED,
            "auth_missing_or_invalid",
            message,
            None,
            false,
        )
    }

    pub(super) fn not_found(message: impl Into<String>) -> Self {
        Self::new(
            StatusCode::NOT_FOUND,
            "no_lyrics_found",
            message,
            None,
            false,
        )
    }
}

impl From<Error> for ApiError {
    fn from(value: Error) -> Self {
        let message = value.to_string();
        let (status, code, retryable) = match &value {
            Error::Provider(_) if message.contains("ai_skipped") => {
                (StatusCode::BAD_GATEWAY, "ai_skipped", true)
            }
            Error::Provider(_) if message.contains("provider_warning") => {
                (StatusCode::BAD_GATEWAY, "provider_warning", true)
            }
            Error::Provider(_) if looks_like_no_lyrics_error(&message) => {
                (StatusCode::NOT_FOUND, "no_lyrics_found", false)
            }
            Error::Provider(_) | Error::Network(_) => {
                (StatusCode::BAD_GATEWAY, "provider_warning", true)
            }
            Error::Service(_) if message.contains("invalid source") => {
                (StatusCode::BAD_REQUEST, "validation_error", false)
            }
            Error::Service(_) => (StatusCode::INTERNAL_SERVER_ERROR, "internal_error", false),
            Error::Json(_) | Error::Parse(_) | Error::Decode(_) | Error::UnknownFormat => {
                (StatusCode::BAD_REQUEST, "validation_error", false)
            }
            Error::Storage(_) | Error::Sqlite(_) | Error::Io(_) => {
                (StatusCode::INTERNAL_SERVER_ERROR, "internal_error", false)
            }
            Error::Task(_) => (StatusCode::INTERNAL_SERVER_ERROR, "internal_error", false),
        };
        Self::new(
            status,
            code,
            message,
            Some(error_details(&value)),
            retryable,
        )
    }
}

impl From<serde_json::Error> for ApiError {
    fn from(value: serde_json::Error) -> Self {
        Self::new(
            StatusCode::BAD_REQUEST,
            "validation_error",
            value.to_string(),
            Some(json!({ "source": "serde_json" })),
            false,
        )
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        (self.status, Json(self.body)).into_response()
    }
}

pub(super) fn looks_like_no_lyrics_error(message: &str) -> bool {
    let lower = message.to_ascii_lowercase();
    lower.contains("no_lyrics_found")
        || lower.contains("no lyric")
        || lower.contains("no aggregate members could be fetched")
}

pub(super) fn error_details(value: &Error) -> Value {
    json!({
        "source": match value {
            Error::UnknownFormat => "unknown_format",
            Error::Decode(_) => "decode",
            Error::Parse(_) => "parse",
            Error::Provider(_) => "provider",
            Error::Service(_) => "service",
            Error::Storage(_) => "storage",
            Error::Network(_) => "network",
            Error::Sqlite(_) => "sqlite",
            Error::Io(_) => "io",
            Error::Json(_) => "json",
            Error::Task(_) => "task",
        }
    })
}
