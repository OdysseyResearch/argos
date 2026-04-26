//! HTTP/SSE reverse-proxy transport (Phase 4 — T029–T032).
//!
//! Accepts HTTP connections on `--bind`:`--port` and reverse-proxies to the
//! `--upstream` URL. Every request body is decoded as JSON-RPC, run through
//! the policy pipeline, and either forwarded (allow/redact), blocked with a
//! JSON-RPC error response, or passed through (non-intercepted methods).
//! Streaming SSE response bodies are piped from upstream to client byte-for-byte
//! without policy evaluation (responses are not enforced).

use std::sync::Arc;

use axum::{
    body::Body,
    extract::{Request, State},
    http::{HeaderMap, HeaderValue, StatusCode},
    response::Response,
    routing::any,
    Router,
};
use bytes::Bytes;
use futures_util::TryStreamExt;
use http::Method;
use uuid::Uuid;

use crate::audit::AuditWriter;
use crate::cli::CliArgs;
use crate::error::AppError;
use crate::policy::PolicyEngine;
use crate::proxy::{intercept, InterceptOutcome};
use crate::transport::{McpFrame, SessionConfig};

#[derive(Clone)]
struct AppState {
    engine: Arc<PolicyEngine>,
    audit: Arc<AuditWriter>,
    config: Arc<SessionConfig>,
    session_id: Arc<String>,
    upstream: Arc<String>,
    client: reqwest::Client,
}

/// Run the proxy in HTTP/SSE reverse-proxy mode (FR-019, FR-028).
pub async fn run_http_proxy(
    args: CliArgs,
    engine: Arc<PolicyEngine>,
    audit: Arc<AuditWriter>,
    session_id: Uuid,
    config: Arc<SessionConfig>,
) -> Result<(), AppError> {
    let upstream = args
        .upstream
        .clone()
        .ok_or_else(|| AppError::Other(anyhow::anyhow!("--upstream is required for HTTP mode")))?;

    let bind_addr = format!("{}:{}", args.bind, args.port);
    let state = AppState {
        engine,
        audit,
        config,
        session_id: Arc::new(session_id.to_string()),
        upstream: Arc::new(upstream),
        client: reqwest::Client::new(),
    };

    let app: Router = Router::new()
        .fallback(any(handle_request))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(&bind_addr)
        .await
        .map_err(|e| AppError::Other(anyhow::anyhow!("failed to bind {bind_addr}: {e}")))?;

    eprintln!("argos-proxy: listening on http://{bind_addr}");

    let cancel = tokio_util::sync::CancellationToken::new();
    let shutdown = {
        let cancel = cancel.clone();
        async move {
            wait_for_shutdown(cancel).await;
        }
    };

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown)
        .await
        .map_err(|e| AppError::Other(anyhow::anyhow!("HTTP server error: {e}")))?;

    Ok(())
}

async fn wait_for_shutdown(cancel: tokio_util::sync::CancellationToken) {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{signal, SignalKind};
        let mut sigterm = match signal(SignalKind::terminate()) {
            Ok(s) => s,
            Err(_) => {
                cancel.cancelled().await;
                return;
            }
        };
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {}
            _ = sigterm.recv() => {}
            _ = cancel.cancelled() => {}
        }
    }
    #[cfg(not(unix))]
    {
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {}
            _ = cancel.cancelled() => {}
        }
    }
}

/// Handle a single inbound HTTP request: decode body → intercept → forward
/// or reject.
async fn handle_request(State(state): State<AppState>, req: Request) -> Response {
    let (parts, body) = req.into_parts();

    let body_bytes = match axum::body::to_bytes(body, usize::MAX).await {
        Ok(b) => b,
        Err(e) => return error_response(StatusCode::BAD_REQUEST, format!("read body: {e}")),
    };

    // POST/PUT bodies with JSON-RPC content go through interception. GETs and
    // empty bodies pass through (handshake / SSE GETs).
    if body_bytes.is_empty() || !is_json_rpc(&parts.headers) {
        return forward_passthrough(&state, parts.method, parts.uri.to_string(), parts.headers, body_bytes).await;
    }

    let frame = McpFrame {
        body: body_bytes.clone(),
    };
    let outcome = match intercept(
        frame,
        state.engine.as_ref(),
        state.audit.as_ref(),
        state.session_id.as_ref(),
        &state.config,
    )
    .await
    {
        Ok(o) => o,
        Err(e) => return error_response(StatusCode::INTERNAL_SERVER_ERROR, format!("audit: {e}")),
    };

    let frame = match outcome {
        InterceptOutcome::Forward(f) | InterceptOutcome::PassThrough(f) => f,
        InterceptOutcome::BlockResponse(f) => {
            // Send JSON-RPC error inline; do not forward upstream.
            return Response::builder()
                .status(StatusCode::OK)
                .header("content-type", "application/json")
                .body(Body::from(f.body))
                .unwrap();
        }
    };

    forward_to_upstream(&state, parts.method, parts.uri.to_string(), parts.headers, frame.body).await
}

fn is_json_rpc(headers: &HeaderMap) -> bool {
    headers
        .get(http::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.starts_with("application/json"))
        .unwrap_or(false)
}

async fn forward_passthrough(
    state: &AppState,
    method: Method,
    path_and_query: String,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    forward_to_upstream(state, method, path_and_query, headers, body).await
}

async fn forward_to_upstream(
    state: &AppState,
    method: Method,
    path_and_query: String,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let url = format!("{}{}", state.upstream.trim_end_matches('/'), path_and_query);

    let reqwest_method = match reqwest::Method::from_bytes(method.as_str().as_bytes()) {
        Ok(m) => m,
        Err(_) => return error_response(StatusCode::BAD_REQUEST, "invalid method".into()),
    };

    let mut req_builder = state.client.request(reqwest_method, &url).body(body);
    for (name, value) in headers.iter() {
        // Skip hop-by-hop headers and Host (reqwest sets it).
        let name_str = name.as_str();
        if matches!(
            name_str.to_ascii_lowercase().as_str(),
            "host" | "connection" | "content-length"
        ) {
            continue;
        }
        if let Ok(v) = value.to_str() {
            req_builder = req_builder.header(name_str, v);
        }
    }

    let upstream_response = match req_builder.send().await {
        Ok(r) => r,
        Err(e) => {
            return error_response(StatusCode::BAD_GATEWAY, format!("upstream: {e}"));
        }
    };

    let status = upstream_response.status();
    let upstream_headers = upstream_response.headers().clone();
    let stream = upstream_response.bytes_stream().map_err(std::io::Error::other);
    let body = Body::from_stream(stream);

    let mut builder = Response::builder().status(StatusCode::from_u16(status.as_u16()).unwrap_or(StatusCode::OK));
    for (name, value) in upstream_headers.iter() {
        if let Ok(v) = HeaderValue::from_bytes(value.as_bytes()) {
            builder = builder.header(name.as_str(), v);
        }
    }
    builder.body(body).unwrap()
}

fn error_response(status: StatusCode, message: String) -> Response {
    let body = serde_json::json!({
        "jsonrpc": "2.0",
        "id": null,
        "error": {"code": -32603, "message": message}
    });
    Response::builder()
        .status(status)
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .unwrap()
}
