use super::*;

#[derive(RustEmbed)]
#[folder = "frontend/dist/"]
pub(super) struct DashboardAssets;

pub(super) async fn index() -> impl IntoResponse {
    asset_response("index.html")
}

pub(super) async fn asset(Path(path): Path<String>) -> impl IntoResponse {
    let path = path.trim_start_matches('/');
    if path.starts_with("api/") {
        return StatusCode::NOT_FOUND.into_response();
    }
    asset_response(if path.is_empty() { "index.html" } else { path })
}

pub(super) fn asset_response(path: &str) -> axum::response::Response {
    if let Some(asset) = DashboardAssets::get(path) {
        let mime = mime_guess::from_path(path).first_or_text_plain();
        return response_with_body(StatusCode::OK, mime.as_ref(), asset.data.into_owned())
            .into_response();
    }

    if should_fallback_to_index(path) {
        if let Some(asset) = DashboardAssets::get("index.html") {
            return response_with_body(StatusCode::OK, "text/html", asset.data.into_owned())
                .into_response();
        }
    }

    (
        StatusCode::NOT_FOUND,
        "dashboard asset was not found; run npm build in frontend",
    )
        .into_response()
}

pub(super) fn should_fallback_to_index(path: &str) -> bool {
    let path = path.trim_start_matches('/');
    if path.is_empty() || path == "index.html" {
        return true;
    }
    if path.starts_with("assets/") || path.contains('.') {
        return false;
    }
    true
}

pub(super) fn response_with_body(
    status: StatusCode,
    content_type: &str,
    body: impl Into<Vec<u8>>,
) -> Response<Body> {
    let mut response = Response::new(Body::from(body.into()));
    *response.status_mut() = status;
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_str(content_type)
            .unwrap_or_else(|_| HeaderValue::from_static("application/octet-stream")),
    );
    response
}
