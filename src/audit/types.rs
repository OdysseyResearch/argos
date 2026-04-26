use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone, Copy)]
pub enum MessageType {
    #[serde(rename = "tools/call")]
    ToolsCall,
    #[serde(rename = "resources/read")]
    ResourcesRead,
    #[serde(rename = "resources/list")]
    ResourcesList,
    #[serde(rename = "resources/subscribe")]
    ResourcesSubscribe,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone, Copy)]
#[serde(rename_all = "lowercase")]
pub enum DecisionLabel {
    Allowed,
    Blocked,
    Redacted,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AuditEntry {
    pub timestamp: String,
    pub sequence: u64,
    pub prev_hash: String,
    pub entry_hash: String,
    pub session_id: String,
    pub message_type: MessageType,
    pub decision: DecisionLabel,
    pub tool_or_resource: String,
    pub arguments: serde_json::Value,
    pub arguments_truncated: bool,
    pub policy_rule_matched: Option<String>,
    pub reason: Option<String>,
    pub agent: String,
    pub policy_version: String,
    pub org_id: Option<String>,
    pub tenant_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dry_run: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RotationMarkerEntry {
    pub entry_type: String,
    pub timestamp: String,
    pub sequence: u64,
    pub prev_hash: String,
    pub entry_hash: String,
    pub session_id: String,
    pub reason: Option<String>,
}

#[derive(Debug, Error)]
pub enum AuditError {
    #[error("Audit log not writable: {0}")]
    NotWritable(PathBuf),
    #[error("Audit write failed: {0}")]
    WriteFailed(#[from] std::io::Error),
    #[error("Audit serialisation failed: {0}")]
    SerialisationFailed(#[from] serde_json::Error),
}
