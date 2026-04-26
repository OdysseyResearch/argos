//! HTTP/SSE transport (Phase 4 — T029–T032).

use std::sync::Arc;

use uuid::Uuid;

use crate::audit::AuditWriter;
use crate::cli::CliArgs;
use crate::error::AppError;
use crate::policy::PolicyEngine;
use crate::transport::SessionConfig;

/// Run the proxy in HTTP/SSE reverse-proxy mode (T029-T032).
pub async fn run_http_proxy(
    _args: CliArgs,
    _engine: Arc<PolicyEngine>,
    _audit: Arc<AuditWriter>,
    _session_id: Uuid,
    _config: Arc<SessionConfig>,
) -> Result<(), AppError> {
    Err(AppError::Other(anyhow::anyhow!(
        "HTTP transport not yet implemented (Phase 4 — T029-T032)"
    )))
}
