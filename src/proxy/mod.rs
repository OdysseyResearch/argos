//! Request interception pipeline.
//!
//! Every intercepted request flows through [`intercept`]:
//!   1. parse the JSON-RPC `method` field
//!   2. classify as pass-through or intercepted
//!   3. for intercepted methods: parse params → evaluate → audit → forward/block
//!
//! Pass-through methods are forwarded byte-for-byte without re-serialisation
//! and without an audit entry.

use std::sync::Arc;

use bytes::Bytes;
use chrono::Utc;
use serde_json::Value;

use crate::audit::{AuditEntry, AuditError, AuditWriter, DecisionLabel, MessageType};
use crate::policy::{PolicyDecision, PolicyEngine, PolicyRequest};
use crate::transport::{McpFrame, SessionConfig};

/// JSON-RPC error code for policy-blocked calls (FR-023).
pub const ERR_POLICY_BLOCKED: i32 = -32000;
/// JSON-RPC error code for malformed requests (FR-024).
pub const ERR_PARSE_ERROR: i32 = -32700;

/// Outcome of running a single frame through the policy pipeline.
#[derive(Debug)]
pub(crate) enum InterceptOutcome {
    /// Forward the (possibly redacted) frame to upstream untouched.
    Forward(McpFrame),
    /// Send the contained JSON-RPC error frame back to the client and do not
    /// forward to upstream.
    BlockResponse(McpFrame),
    /// Pass-through method — forward the original frame, no audit entry written.
    PassThrough(McpFrame),
}

/// Pipeline entry point. Runs evaluation + audit and returns the outcome the
/// transport adapter should act on.
pub(crate) async fn intercept(
    frame: McpFrame,
    engine: &PolicyEngine,
    audit: &AuditWriter,
    session_id: &str,
    config: &Arc<SessionConfig>,
) -> Result<InterceptOutcome, AuditError> {
    let parsed: Value = match serde_json::from_slice(&frame.body) {
        Ok(v) => v,
        Err(_) => {
            let id = Value::Null;
            return Ok(InterceptOutcome::BlockResponse(jsonrpc_error_frame(
                &id,
                ERR_PARSE_ERROR,
                "Parse error",
            )));
        }
    };

    let method = parsed
        .get("method")
        .and_then(Value::as_str)
        .unwrap_or_default();

    let id = parsed.get("id").cloned().unwrap_or(Value::Null);

    if !is_intercepted(method) {
        return Ok(InterceptOutcome::PassThrough(frame));
    }

    let message_type = match method {
        "tools/call" => MessageType::ToolsCall,
        "resources/read" => MessageType::ResourcesRead,
        "resources/list" => MessageType::ResourcesList,
        "resources/subscribe" => MessageType::ResourcesSubscribe,
        _ => unreachable!("is_intercepted gate"),
    };

    let params = parsed.get("params").cloned().unwrap_or(Value::Null);
    let (policy_request, tool_or_resource, raw_arguments) =
        build_policy_request(message_type, &params);

    let decision = engine.evaluate(&policy_request);

    let (decision_label, reason, rule_id, redact_fields) = match &decision {
        PolicyDecision::Allow { rule_id } => (DecisionLabel::Allowed, None, Some(*rule_id), vec![]),
        PolicyDecision::Block { reason, rule_id } => (
            DecisionLabel::Blocked,
            Some(reason.clone()),
            Some(*rule_id),
            vec![],
        ),
        PolicyDecision::Redact { fields, rule_id } => (
            DecisionLabel::Redacted,
            None,
            Some(*rule_id),
            fields.clone(),
        ),
        PolicyDecision::DenyByDefault => (
            DecisionLabel::Blocked,
            Some("deny by default".to_string()),
            None,
            vec![],
        ),
    };

    let action_label = match decision_label {
        DecisionLabel::Allowed => "allow",
        DecisionLabel::Blocked => "block",
        DecisionLabel::Redacted => "redact",
    };
    let policy_rule_matched = rule_id
        .map(|id| format!("{}:{}[{}]", tool_or_resource, action_label, id))
        .or_else(|| match decision_label {
            DecisionLabel::Blocked => None, // deny-by-default
            _ => None,
        });

    let (logged_args, args_truncated) = truncate_arguments(raw_arguments, config.max_arg_bytes);

    let dry_run_flag = if config.dry_run && matches!(decision_label, DecisionLabel::Blocked) {
        Some(true)
    } else {
        None
    };

    let entry = AuditEntry {
        timestamp: rfc3339_micros(),
        sequence: 0,
        prev_hash: String::new(),
        entry_hash: String::new(),
        session_id: session_id.to_string(),
        message_type,
        decision: decision_label,
        tool_or_resource: tool_or_resource.clone(),
        arguments: logged_args,
        arguments_truncated: args_truncated,
        policy_rule_matched,
        reason: reason.clone(),
        agent: audit.agent().to_string(),
        policy_version: audit.policy_version().to_string(),
        org_id: None,
        tenant_id: None,
        dry_run: dry_run_flag,
    };

    audit.write(entry).await?;

    if dry_run_flag.is_some() {
        eprintln!(
            "DRY RUN VIOLATION: {} {} would be blocked: {}",
            method,
            tool_or_resource,
            reason.as_deref().unwrap_or("policy decision")
        );
    }

    match decision_label {
        DecisionLabel::Allowed => Ok(InterceptOutcome::Forward(frame)),
        DecisionLabel::Redacted => {
            let redacted = redact_request_frame(&parsed, &redact_fields)?;
            Ok(InterceptOutcome::Forward(redacted))
        }
        DecisionLabel::Blocked => {
            if config.dry_run {
                Ok(InterceptOutcome::Forward(frame))
            } else {
                let msg = reason.unwrap_or_else(|| "Blocked by policy".to_string());
                Ok(InterceptOutcome::BlockResponse(jsonrpc_error_frame(
                    &id,
                    ERR_POLICY_BLOCKED,
                    &msg,
                )))
            }
        }
    }
}

/// MCP method names whose calls Argos enforces in v0.1 (FR-017, FR-008b).
fn is_intercepted(method: &str) -> bool {
    matches!(
        method,
        "tools/call" | "resources/read" | "resources/list" | "resources/subscribe"
    )
}

fn build_policy_request(
    message_type: MessageType,
    params: &Value,
) -> (PolicyRequest, String, Value) {
    match message_type {
        MessageType::ToolsCall => {
            let name = params
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();
            let arguments = params.get("arguments").cloned().unwrap_or(Value::Null);
            let tool_or_resource = name.clone();
            (
                PolicyRequest::Tool {
                    name,
                    arguments: arguments.clone(),
                },
                tool_or_resource,
                arguments,
            )
        }
        MessageType::ResourcesRead
        | MessageType::ResourcesList
        | MessageType::ResourcesSubscribe => {
            let uri = params
                .get("uri")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();
            (
                PolicyRequest::Resource { uri: uri.clone() },
                uri,
                params.clone(),
            )
        }
    }
}

/// Truncate arguments to `max_bytes` for audit logging (FR-015).
fn truncate_arguments(arguments: Value, max_bytes: usize) -> (Value, bool) {
    let Ok(serialised) = serde_json::to_string(&arguments) else {
        return (Value::String("<unserialisable>".to_string()), true);
    };
    if serialised.len() > max_bytes {
        (Value::String("<truncated>".to_string()), true)
    } else {
        (arguments, false)
    }
}

/// Construct a JSON-RPC error response frame (FR-023, FR-024).
fn jsonrpc_error_frame(id: &Value, code: i32, message: &str) -> McpFrame {
    let body = serde_json::json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": { "code": code, "message": message },
    });
    let bytes = serde_json::to_vec(&body).expect("static JSON-RPC error body");
    McpFrame {
        body: Bytes::from(bytes),
    }
}

/// Build a redacted version of an intercepted request frame by stripping
/// `fields` from `params.arguments` (tool calls) (FR-007).
fn redact_request_frame(parsed: &Value, fields: &[String]) -> Result<McpFrame, AuditError> {
    let mut clone = parsed.clone();
    if let Some(args) = clone
        .get_mut("params")
        .and_then(|p| p.get_mut("arguments"))
        .and_then(Value::as_object_mut)
    {
        for f in fields {
            args.remove(f);
        }
    }
    let bytes = serde_json::to_vec(&clone)?;
    Ok(McpFrame {
        body: Bytes::from(bytes),
    })
}

fn rfc3339_micros() -> String {
    Utc::now().format("%Y-%m-%dT%H:%M:%S%.6fZ").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pass_through_methods_classified() {
        assert!(!is_intercepted("initialize"));
        assert!(!is_intercepted("ping"));
        assert!(!is_intercepted("tools/list"));
        assert!(!is_intercepted("resources/unsubscribe"));
        assert!(!is_intercepted("notifications/resources/updated"));
        assert!(!is_intercepted("prompts/list"));
        assert!(!is_intercepted("sampling/createMessage"));
    }

    #[test]
    fn intercepted_methods_classified() {
        assert!(is_intercepted("tools/call"));
        assert!(is_intercepted("resources/read"));
        assert!(is_intercepted("resources/list"));
        assert!(is_intercepted("resources/subscribe"));
    }

    #[test]
    fn truncation_replaces_oversize_arguments() {
        let huge = Value::String("x".repeat(1000));
        let (out, truncated) = truncate_arguments(huge, 100);
        assert!(truncated);
        assert_eq!(out, Value::String("<truncated>".to_string()));
    }

    #[test]
    fn truncation_passes_through_small_arguments() {
        let small = serde_json::json!({"path": "/x"});
        let (out, truncated) = truncate_arguments(small.clone(), 1024);
        assert!(!truncated);
        assert_eq!(out, small);
    }

    #[test]
    fn redact_strips_specified_fields() {
        let original = serde_json::json!({
            "method": "tools/call",
            "params": {
                "name": "read_file",
                "arguments": {"path": "/x", "token": "secret"}
            }
        });
        let frame = redact_request_frame(&original, &["token".to_string()]).unwrap();
        let parsed: Value = serde_json::from_slice(&frame.body).unwrap();
        assert!(parsed["params"]["arguments"].get("token").is_none());
        assert_eq!(parsed["params"]["arguments"]["path"], "/x");
    }
}
