//! Minimal example of using the `argos` crate as a library: load a policy,
//! evaluate a synthetic request, and write an audit entry — all without the
//! `argos-proxy` CLI subprocess (SC-009).

use std::io::Write;

use argos::audit::{AuditEntry, AuditWriter, DecisionLabel, MessageType};
use argos::policy::{PolicyDecision, PolicyEngine, PolicyRequest};
use uuid::Uuid;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Write a tiny policy file in a temp dir.
    let policy_dir = tempfile::tempdir()?;
    let policy_path = policy_dir.path().join("policy.toml");
    let mut f = std::fs::File::create(&policy_path)?;
    f.write_all(
        br#"
[meta]
version = "0.1"
description = "Library API example"

[[rules]]
tool = "read_file"
action = "allow"
constraints = { path_prefix = "/workspace" }
tags = []

[[rules]]
tool = "*"
action = "block"
reason = "Default deny"
tags = []
"#,
    )?;
    drop(f);

    // Load and evaluate.
    let engine = PolicyEngine::load(&policy_path)?;

    let session_id = Uuid::new_v4();
    let audit_path = policy_dir.path().join("audit.jsonl");
    let writer = AuditWriter::open(&audit_path, session_id, "library-example", engine.version()).await?;

    let request = PolicyRequest::Tool {
        name: "read_file".to_string(),
        arguments: serde_json::json!({"path": "/workspace/main.rs"}),
    };
    let decision = engine.evaluate(&request);
    println!("decision: {decision:?}");

    let (decision_label, reason) = match &decision {
        PolicyDecision::Allow { .. } => (DecisionLabel::Allowed, None),
        PolicyDecision::Block { reason, .. } => (DecisionLabel::Blocked, Some(reason.clone())),
        PolicyDecision::Redact { .. } => (DecisionLabel::Redacted, None),
        PolicyDecision::DenyByDefault => (DecisionLabel::Blocked, Some("deny by default".into())),
    };

    let entry = AuditEntry {
        timestamp: chrono::Utc::now()
            .format("%Y-%m-%dT%H:%M:%S%.6fZ")
            .to_string(),
        sequence: 0, // overwritten by writer
        prev_hash: String::new(),
        entry_hash: String::new(),
        session_id: session_id.to_string(),
        message_type: MessageType::ToolsCall,
        decision: decision_label,
        tool_or_resource: "read_file".to_string(),
        arguments: serde_json::json!({"path": "/workspace/main.rs"}),
        arguments_truncated: false,
        policy_rule_matched: Some("read_file:allow[0]".to_string()),
        reason,
        agent: "library-example".to_string(),
        policy_version: engine.version().to_string(),
        org_id: None,
        tenant_id: None,
        dry_run: None,
    };
    writer.write(entry).await?;
    writer.flush().await?;

    println!("audit entry written to: {}", audit_path.display());
    let log = std::fs::read_to_string(&audit_path)?;
    println!("log contents:\n{log}");

    Ok(())
}
