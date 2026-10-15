use std::net::SocketAddr;

use axum::body::Body;
use axum::extract::{Path, Query, State};
use axum::http::{header, HeaderMap, HeaderValue, Response, StatusCode};
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};
use rust_embed::RustEmbed;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::cache::{AiScoreQuery, UpstreamCache};
use crate::provider::{builtin_provider_registry, SearchResult, Source};
use crate::service::{
    source_from_cli_name, AggregateFetchRequest, AiScoringConfig, LyricNeed, MergeMode,
    ServiceContext, SourceSearchRequest, SourceSearchResult, SpecificFetchFormat,
    SpecificFetchResult,
};
use crate::{Error, Result};

mod assets;
mod error;
mod handlers;
mod search;
#[cfg(test)]
mod tests;

use assets::*;
use error::*;
use handlers::*;
use search::*;

#[derive(Clone)]
pub struct ServerOptions {
    pub host: String,
    pub port: u16,
    pub open_browser: bool,
    pub context: ServiceContext,
}

#[derive(Clone)]
struct AppState {
    context: ServiceContext,
    server_token: Option<String>,
}

pub async fn run(options: ServerOptions) -> Result<()> {
    let token = std::env::var("ROSETTRISM_SERVER_TOKEN")
        .ok()
        .filter(|value| !value.trim().is_empty());
    if !is_local_host(&options.host) && token.is_none() {
        return Err(Error::Service(
            "binding to a non-local host requires ROSETTRISM_SERVER_TOKEN".into(),
        ));
    }

    let addr: SocketAddr = format!("{}:{}", options.host, options.port)
        .parse()
        .map_err(|err| Error::Service(format!("invalid server address: {err}")))?;
    let state = AppState {
        context: options.context,
        server_token: token,
    };
    let app = app(state);
    let listener = tokio::net::TcpListener::bind(addr).await?;
    let url = format!("http://{addr}");
    eprintln!("Rosettrism server listening on {url}");
    if options.open_browser {
        open_browser(&url);
    }
    axum::serve(listener, app).await?;
    Ok(())
}

fn app(state: AppState) -> Router {
    Router::new()
        .route("/api/health", get(health))
        .route("/api/providers/health", get(providers_health))
        .route("/api/sources", get(sources))
        .route("/api/search", post(search))
        .route("/api/fetch", post(fetch))
        .route("/api/fetch-result", post(fetch_result))
        .route("/api/ai/replay", post(ai_replay))
        .route("/api/cache", get(cache_list))
        .route("/api/cache/:id", get(cache_detail).delete(cache_delete))
        .route("/api/cache/:id/revalidate", post(cache_revalidate))
        .route(
            "/api/unified-cache/:id",
            get(unified_cache_detail).delete(unified_cache_delete),
        )
        .route(
            "/api/unified-cache/:id/revalidate",
            post(unified_cache_revalidate),
        )
        .route("/api/runs", get(fetch_runs))
        .route("/api/stats", get(stats))
        .route("/", get(index))
        .route("/*path", get(asset))
        .with_state(state)
}

fn authorize(state: &AppState, headers: &HeaderMap) -> ApiResult<()> {
    let Some(expected) = state.server_token.as_deref() else {
        return Ok(());
    };
    let bearer = headers
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "));
    let explicit = headers
        .get("x-rosettrism-token")
        .and_then(|value| value.to_str().ok());
    if bearer == Some(expected) || explicit == Some(expected) {
        Ok(())
    } else {
        Err(ApiError::unauthorized("missing or invalid server token"))
    }
}

fn require_cache(state: &AppState) -> ApiResult<&UpstreamCache> {
    state
        .context
        .cache
        .as_ref()
        .ok_or_else(|| ApiError::bad_request("cache is disabled"))
}

fn is_local_host(host: &str) -> bool {
    matches!(host, "127.0.0.1" | "localhost" | "::1")
}

fn open_browser(url: &str) {
    #[cfg(windows)]
    {
        let _ = std::process::Command::new("cmd")
            .args(["/C", "start", "", url])
            .spawn();
    }
    #[cfg(target_os = "macos")]
    {
        let _ = std::process::Command::new("open").arg(url).spawn();
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let _ = std::process::Command::new("xdg-open").arg(url).spawn();
    }
}
