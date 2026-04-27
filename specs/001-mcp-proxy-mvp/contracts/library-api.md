# Contract: Library API (`argos` crate)

**Version**: 0.1 | **Crate**: `argos` (lib target)

The `argos` crate exposes the policy engine and audit writer as a stable public API so
downstream crates can embed them without invoking the CLI.

---

## Public modules

```
argos::policy   — PolicyEngine, PolicyRequest, PolicyFile, PolicyRule, PolicyDecision, PolicyAction, PolicyError
argos::audit    — AuditWriter, AuditEntry, AuditError, MessageType, DecisionLabel
```

---

## `argos::policy`

### `PolicyEngine::load`

```rust
pub fn load(path: &std::path::Path) -> Result<PolicyEngine, PolicyError>
```

Loads and validates a TOML policy file from `path`. Returns an error if:

- The file does not exist or is not readable (`PolicyError::NotFound`)
- The TOML is malformed (`PolicyError::ParseError`)
- `meta.version` is not a supported version (`PolicyError::UnsupportedVersion`)
- Any rule has a structural violation (`PolicyError::InvalidRule`)

### `PolicyEngine::evaluate`

```rust
pub fn evaluate(&self, request: &PolicyRequest) -> PolicyDecision
```

Evaluates a `PolicyRequest` against the loaded policy. Stateless — safe to call
concurrently from multiple threads/tasks without external locking.

`PolicyRequest` is the public input type — library users construct it directly without
any knowledge of internal wire-protocol types (`McpRequest`, `McpFrame`).

Returns one of:

- `PolicyDecision::Allow` — request matched an `allow` rule
- `PolicyDecision::Block { reason, rule_id }` — request matched a `block` rule
- `PolicyDecision::Redact { fields, rule_id }` — request matched a `redact` rule
- `PolicyDecision::DenyByDefault` — no rule matched

### `PolicyEngine::version`

```rust
pub fn version(&self) -> &str
```

Returns `meta.version` from the loaded policy. Used to populate `policy_version` in audit
entries.

---

## `argos::audit`

### `AuditWriter::open`

```rust
pub fn open(
    path: &std::path::Path,
    session_id: uuid::Uuid,
    agent: &str,
    policy_version: &str,
) -> Result<AuditWriter, AuditError>
```

Opens (or creates) the audit log file at `path` in append mode. Returns an error if the path
is not writable. The writer is `Clone` + `Send` + `Sync` via `Arc<Mutex<...>>` internals.

### `AuditWriter::write`

```rust
pub async fn write(&self, entry: AuditEntry) -> Result<(), AuditError>
```

Acquires the internal mutex, computes `entry_hash`, writes the JSONL line, and releases the
mutex. Returns `AuditError::WriteFailed` on any I/O error.

### `AuditWriter::flush`

```rust
pub async fn flush(&self) -> Result<(), AuditError>
```

Flushes the OS write buffer to disk. Call before process exit to guarantee no entries are lost.

---

## Minimal usage example

```rust
use argos::policy::{PolicyEngine, PolicyRequest, PolicyDecision};
use argos::audit::{AuditWriter, AuditEntry, MessageType, DecisionLabel};
use std::path::Path;
use uuid::Uuid;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Load policy
    let engine = PolicyEngine::load(Path::new("policy.toml"))?;

    // Open audit log
    let session_id = Uuid::new_v4();
    let writer = AuditWriter::open(
        Path::new("audit.jsonl"),
        session_id,
        "my-agent",
        engine.version(),
    )?;

    // Construct a policy request — no wire-protocol knowledge required
    let request = PolicyRequest::Tool {
        name: "read_file".to_string(),
        arguments: serde_json::json!({ "path": "/workspace/main.rs" }),
    };
    let decision = engine.evaluate(&request);

    // Write audit entry — construct AuditEntry directly (no from_decision convenience method)
    let (decision_label, reason) = match &decision {
        PolicyDecision::Allow { .. } => (DecisionLabel::Allowed, None),
        PolicyDecision::Block { reason, .. } => (DecisionLabel::Blocked, Some(reason.clone())),
        PolicyDecision::Redact { .. } => (DecisionLabel::Redacted, None),
        PolicyDecision::DenyByDefault => (DecisionLabel::Blocked, Some("deny by default".into())),
    };
    let entry = AuditEntry {
        timestamp: chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S%.6fZ").to_string(),
        sequence: 0,
        prev_hash: String::new(),
        entry_hash: String::new(),
        session_id: session_id.to_string(),
        message_type: MessageType::ToolsCall,
        decision: decision_label,
        tool_or_resource: "read_file".to_string(),
        arguments: serde_json::json!({ "path": "/workspace/main.rs" }),
        arguments_truncated: false,
        policy_rule_matched: None,
        reason,
        agent: "my-agent".to_string(),
        policy_version: engine.version().to_string(),
        org_id: None,
        tenant_id: None,
        dry_run: None,
    };
    writer.write(entry).await?;
    writer.flush().await?;

    Ok(())
}
```

This example is provided as `examples/basic_policy.rs` in the repository. It must compile and
run as part of the CI pipeline (SC-009).

---

## Stability guarantee

The public API described in this document is **pre-stable** in v0.1. Breaking changes are
permitted until M6 (Stable API, 1.0.0). Downstream crates should pin to a specific v0.x minor
version.

The `argos::policy` and `argos::audit` module paths are stable from v0.1 onwards — they will
not be moved or renamed in subsequent minor versions.
