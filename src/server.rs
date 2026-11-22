//! Web interface: a single embedded page plus a small JSON API.

use std::net::{IpAddr, SocketAddr};

use anyhow::{Context, Result};
use axum::extract::DefaultBodyLimit;
use axum::http::{header, StatusCode};
use axum::response::{Html, IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::json;
use tokio::net::TcpListener;

use crate::{LeafEncoding, Mode, Options, Whitelist};

const INDEX_HTML: &str = include_str!("../web/index.html");

#[derive(Deserialize)]
struct GenerateRequest {
    #[serde(default)]
    mode: Mode,
    #[serde(default)]
    encoding: LeafEncoding,
    #[serde(default)]
    decimals: u8,
    input: String,
}

pub fn router(max_body_bytes: usize) -> Router {
    Router::new()
        .route("/", get(|| async { Html(INDEX_HTML) }))
        .route("/healthz", get(|| async { "ok" }))
        .route("/api/generate", post(generate))
        .layer(DefaultBodyLimit::max(max_body_bytes))
}

pub async fn serve(host: IpAddr, port: u16, max_body_bytes: usize) -> Result<()> {
    let address = SocketAddr::new(host, port);
    let listener = TcpListener::bind(address)
        .await
        .with_context(|| format!("failed to listen on {address}"))?;

    eprintln!("Listening on http://{address}");
    axum::serve(listener, router(max_body_bytes))
        .with_graceful_shutdown(shutdown_signal())
        .await
        .context("server error")
}

async fn generate(Json(request): Json<GenerateRequest>) -> Response {
    let options =
        Options { mode: request.mode, encoding: request.encoding, decimals: request.decimals };

    // Hashing a large list is CPU-bound, so keep it off the async workers.
    let task = tokio::task::spawn_blocking(move || {
        let whitelist = Whitelist::generate(&request.input, options)?;
        let mut body = Vec::new();
        whitelist.write_json(&mut body, false).expect("writing JSON to memory cannot fail");
        Ok::<_, crate::InputError>(body)
    });

    match task.await {
        Ok(Ok(body)) => ([(header::CONTENT_TYPE, "application/json")], body).into_response(),
        Ok(Err(error)) => (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": error.summary(), "details": error.line_errors() })),
        )
            .into_response(),
        Err(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": "internal error while generating the tree" })),
        )
            .into_response(),
    }
}

async fn shutdown_signal() {
    let ctrl_c = async {
        let _ = tokio::signal::ctrl_c().await;
    };

    #[cfg(unix)]
    let terminate = async {
        use tokio::signal::unix::{signal, SignalKind};
        match signal(SignalKind::terminate()) {
            Ok(mut stream) => {
                stream.recv().await;
            }
            Err(_) => std::future::pending::<()>().await,
        }
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {}
        _ = terminate => {}
    }
}
