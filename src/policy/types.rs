use std::collections::HashMap;
use std::path::PathBuf;

use serde::Deserialize;
use thiserror::Error;

/// Public input type for [`crate::policy::PolicyEngine::evaluate`].
///
/// Encapsulates only what the policy engine needs — tool name + arguments,
/// or resource URI. Internal wire-protocol types (`McpRequest`, `McpFrame`)
/// are never exposed through the library API.
#[derive(Debug, Clone)]
pub enum PolicyRequest {
    Tool {
        name: String,
        arguments: serde_json::Value,
    },
    Resource {
        uri: String,
    },
}

impl PolicyRequest {
    /// Convenience constructor for tool calls.
    pub fn tool(name: impl Into<String>, arguments: serde_json::Value) -> Self {
        Self::Tool {
            name: name.into(),
            arguments,
        }
    }

    /// Convenience constructor for resource accesses.
    pub fn resource(uri: impl Into<String>) -> Self {
        Self::Resource { uri: uri.into() }
    }
}

#[derive(Debug, Deserialize)]
pub struct PolicyFile {
    pub meta: PolicyMeta,
    #[serde(default)]
    pub rules: Vec<PolicyRule>,
}

#[derive(Debug, Deserialize)]
pub struct PolicyMeta {
    pub version: String,
    #[serde(default)]
    pub agent: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub session_tags: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct PolicyRule {
    #[serde(default)]
    pub tool: Option<String>,
    #[serde(default)]
    pub resource: Option<String>,
    pub action: PolicyAction,
    #[serde(default)]
    pub constraints: Option<HashMap<String, String>>,
    #[serde(default)]
    pub reason: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub redact: Option<Vec<String>>,
}

#[derive(Debug, Deserialize, PartialEq, Eq, Clone, Copy)]
#[serde(rename_all = "lowercase")]
pub enum PolicyAction {
    Allow,
    Block,
    Redact,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PolicyDecision {
    Allow {
        rule_id: usize,
    },
    Block {
        reason: String,
        rule_id: usize,
    },
    Redact {
        fields: Vec<String>,
        rule_id: usize,
    },
    DenyByDefault,
}

#[derive(Debug, Error)]
pub enum PolicyError {
    #[error("Policy file not found: {0}")]
    NotFound(PathBuf),
    #[error("Policy parse error: {0}")]
    ParseError(#[from] toml::de::Error),
    #[error("Unsupported policy version '{version}'. Supported versions: {supported}")]
    UnsupportedVersion {
        version: String,
        supported: String,
    },
    #[error("Invalid rule at index {index}: {reason}")]
    InvalidRule { index: usize, reason: String },
    #[error("I/O error reading policy file: {0}")]
    Io(#[from] std::io::Error),
    #[error("Glob compilation failed: {0}")]
    GlobError(#[from] globset::Error),
}
