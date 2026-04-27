use std::sync::Arc;

use bytes::Bytes;
use uuid::Uuid;

use crate::audit::AuditWriter;
use crate::policy::PolicyEngine;

pub mod http;
pub mod stdio;

/// Internal representation of a parsed JSON-RPC MCP request.
///
/// Carries the raw framed bytes alongside the parsed `id`/`method`/`params`
/// so pass-through messages can be forwarded byte-for-byte without
/// re-serialisation.
#[derive(Debug, Clone)]
pub(crate) struct McpRequest {
    pub id: serde_json::Value,
    pub method: String,
    pub params: serde_json::Value,
    pub raw_bytes: Bytes,
}

/// One Content-Length-framed JSON-RPC message.
#[derive(Debug, Clone)]
pub(crate) struct McpFrame {
    pub body: Bytes,
}

#[derive(Debug, Clone)]
pub struct SessionConfig {
    pub dry_run: bool,
    pub max_arg_bytes: usize,
    #[allow(dead_code)]
    pub agent: String,
}

#[derive(Clone)]
pub(crate) struct ProxySession {
    pub session_id: Uuid,
    pub policy: Arc<PolicyEngine>,
    pub audit: Arc<AuditWriter>,
    pub config: Arc<SessionConfig>,
}
