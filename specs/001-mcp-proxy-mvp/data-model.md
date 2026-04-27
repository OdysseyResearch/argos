# Data Model: Argos v0.1 MCP Security Proxy MVP

**Branch**: `001-mcp-proxy-mvp` | **Date**: 2026-04-24

All types are Rust. Public types (exposed in the `[lib]` crate API) are marked **public**.
Internal types are used within the binary only.

---

## Policy Domain

### `PolicyFile` — **public**

```rust
#[derive(Debug, Deserialize)]
pub struct PolicyFile {
    pub meta: PolicyMeta,
    pub rules: Vec<PolicyRule>,
}
```

Loaded from TOML at startup. Validated: `meta.version` must be in `SUPPORTED_VERSIONS`.
Unrecognised version → `PolicyError::UnsupportedVersion`.

### `PolicyMeta` — **public**

```rust
#[derive(Debug, Deserialize)]
pub struct PolicyMeta {
    pub version: String,        // required; validated post-load
    pub agent: Option<String>,  // human label in policy file (separate from --agent CLI flag)
    pub description: Option<String>,
    pub session_tags: Vec<String>, // reserved; empty vec acceptable
}
```

`session_tags` defaults to empty vec if absent in TOML.

### `PolicyRule` — **public**

```rust
#[derive(Debug, Deserialize)]
pub struct PolicyRule {
    pub tool: Option<String>,       // tool name or "*"; mutually exclusive with resource
    pub resource: Option<String>,   // glob pattern or "*"; mutually exclusive with tool
    pub action: PolicyAction,
    pub constraints: Option<HashMap<String, String>>, // e.g. { path_prefix: "/workspace" }
    pub reason: Option<String>,
    pub tags: Vec<String>,          // required field; empty vec acceptable
    pub redact: Option<Vec<String>>, // field names to strip; required when action = Redact
}
```

Validation at load time:

- Exactly one of `tool` or `resource` must be set (not both, not neither)
- `action = Redact` requires `redact` to be non-empty
- `action` must be a recognised value

### `PolicyAction` — **public**

```rust
#[derive(Debug, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum PolicyAction {
    Allow,
    Block,
    Redact,
}
```

### `PolicyDecision` — **public**

```rust
#[derive(Debug, Clone)]
pub enum PolicyDecision {
    Allow,
    Block { reason: String, rule_id: usize },
    Redact { fields: Vec<String>, rule_id: usize },
    DenyByDefault,  // no rule matched
}
```

`rule_id` is the 0-based index of the matched rule in `PolicyFile.rules` (for audit logging).
`DenyByDefault` is the implicit deny when no rule matches.

### `PolicyRequest` — **public**

```rust
/// Public input type for `PolicyEngine::evaluate`. Encapsulates only what the
/// policy engine needs — tool name + arguments, or resource URI. Internal
/// wire-protocol types (`McpRequest`, `McpFrame`) are never exposed.
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
```

`proxy::intercept()` converts the internal `McpRequest` into a `PolicyRequest` before
calling `evaluate`. Library users construct `PolicyRequest` directly without any
knowledge of Content-Length framing or JSON-RPC wire format.

### `PolicyEngine` — **public**

```rust
pub struct PolicyEngine {
    file: PolicyFile,
    resource_globs: GlobSet, // compiled once at load time from resource rules
}

impl PolicyEngine {
    pub fn load(path: &Path) -> Result<Self, PolicyError>;
    pub fn evaluate(&self, request: &PolicyRequest) -> PolicyDecision;
    pub fn version(&self) -> &str; // returns meta.version for audit entries
}
```

`evaluate` is stateless and `&self` (no mutation) — safe to call from multiple concurrent tasks
without locking.

---

## Audit Domain

### `AuditEntry` — **public**

```rust
#[derive(Debug, Serialize, Deserialize)]
pub struct AuditEntry {
    pub timestamp: String,           // RFC 3339 with microseconds
    pub sequence: u64,               // monotonically increasing, 1-based
    pub prev_hash: String,           // "sha256:<64 hex chars>"
    pub entry_hash: String,          // "sha256:<64 hex chars>"; empty string during hash computation
    pub session_id: String,          // UUID v4
    pub message_type: MessageType,
    pub decision: DecisionLabel,
    pub tool_or_resource: String,    // tool name or resource URI
    pub arguments: serde_json::Value, // truncated to --max-arg-bytes
    pub arguments_truncated: bool,   // true if truncation occurred
    pub policy_rule_matched: Option<String>, // e.g. "read_file:allow[0]"; None for deny-by-default
    pub reason: Option<String>,
    pub agent: String,               // from --agent flag, default "unknown"
    pub policy_version: String,      // from PolicyMeta.version
    pub org_id: Option<String>,      // nullable, reserved for M7
    pub tenant_id: Option<String>,   // nullable, reserved for M7
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dry_run: Option<bool>,       // present and true only in --dry-run mode for blocked calls
}
```

### `RotationMarkerEntry` — **public**

```rust
#[derive(Debug, Serialize)]
pub struct RotationMarkerEntry {
    pub entry_type: &'static str,  // always "rotation_marker"
    pub timestamp: String,
    pub sequence: u64,
    pub prev_hash: String,
    pub entry_hash: String,
    pub session_id: String,
    pub reason: Option<String>,
}
```

Not emitted in v0.1. Schema is defined so future rotation logic can write it without a format
change.

### `MessageType` — **public**

```rust
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
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
```

### `DecisionLabel` — **public**

```rust
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DecisionLabel {
    Allowed,
    Blocked,
    Redacted,
}
```

### `AuditWriter` — **public**

```rust
pub struct AuditWriter {
    inner: Arc<Mutex<AuditWriterInner>>,
    session_id: String,
    agent: String,
    policy_version: String,
}

struct AuditWriterInner {
    file: BufWriter<File>,
    sequence: u64,
    prev_hash: String,
}

impl AuditWriter {
    pub fn open(
        path: &Path,
        session_id: Uuid,
        agent: &str,
        policy_version: &str,
    ) -> Result<Self, AuditError>;

    pub async fn write(&self, entry: AuditEntry) -> Result<(), AuditError>;
    pub async fn flush(&self) -> Result<(), AuditError>;
}
```

`write` acquires the lock, computes `entry_hash` (see research.md §5), writes the JSONL line,
and releases the lock. `flush` ensures the OS buffer is flushed to disk.

---

## Transport Domain

### `McpRequest` — internal

```rust
struct McpRequest {
    pub id: serde_json::Value,   // JSON-RPC id (may be null, string, or number)
    pub method: String,
    pub params: serde_json::Value,
    pub raw_bytes: Bytes,        // original framed bytes, for pass-through forwarding
}
```

Parsed from the Content-Length-framed stream. `raw_bytes` retained for unmodified forwarding
of pass-through messages.

### `McpFrame` — internal

```rust
struct McpFrame {
    pub content_length: usize,
    pub body: Bytes,
}
```

Output of the custom `tokio_util::codec::Decoder`. Input to the custom `Encoder` for writing
responses.

### `ProxySession` — internal

```rust
struct ProxySession {
    pub session_id: Uuid,
    pub policy: Arc<PolicyEngine>,
    pub audit: Arc<AuditWriter>,
    pub config: SessionConfig,
}

struct SessionConfig {
    pub dry_run: bool,
    pub max_arg_bytes: usize,
    pub agent: String,
}
```

Created once per proxy invocation. `Arc`-wrapped so it can be shared across concurrent request
handler tasks without copying.

---

## CLI Domain

### `CliArgs` — internal

```rust
#[derive(Parser)]
struct CliArgs {
    #[arg(long, required = true)]
    policy: PathBuf,

    #[arg(long, required = true)]
    audit_log: PathBuf,

    #[arg(long, default_value = "unknown")]
    agent: String,

    #[arg(long, default_value_t = 65536)]
    max_arg_bytes: usize,

    #[arg(long)]
    dry_run: bool,

    #[arg(long)]
    verbose: bool,

    // HTTP mode flags
    #[arg(long)]
    upstream: Option<String>,

    #[arg(long, default_value = "127.0.0.1")]
    bind: String,

    #[arg(long, default_value_t = 8080)]
    port: u16,

    #[arg(long)]
    tls_cert: Option<PathBuf>,

    #[arg(long)]
    tls_key: Option<PathBuf>,

    // stdio mode: everything after "--"
    #[arg(last = true)]
    server_command: Vec<String>,
}
```

Startup validation (before any subprocess or file is opened):

- Neither `upstream` nor `server_command` provided → error "specify --upstream or -- <cmd>"
- Both `upstream` and `server_command` provided → error (FR-019)
- `policy` file not readable → error, exit 1
- `audit_log` path not writable → error, exit 1
- `tls_cert` provided without `tls_key` (or vice versa) → error, exit 1

---

## Error Domain

### `PolicyError` — **public**

```rust
#[derive(Debug, thiserror::Error)]
pub enum PolicyError {
    #[error("Policy file not found: {0}")]
    NotFound(PathBuf),
    #[error("Policy parse error: {0}")]
    ParseError(#[from] toml::de::Error),
    #[error("Unsupported policy version '{version}'. Supported: {supported}")]
    UnsupportedVersion { version: String, supported: String },
    #[error("Invalid rule at index {index}: {reason}")]
    InvalidRule { index: usize, reason: String },
    #[error("Unrecognised action '{action}' at rule index {index}")]
    UnrecognisedAction { action: String, index: usize },
}
```

### `AuditError` — **public**

```rust
#[derive(Debug, thiserror::Error)]
pub enum AuditError {
    #[error("Audit log not writable: {0}")]
    NotWritable(PathBuf),
    #[error("Audit write failed: {0}")]
    WriteFailed(#[from] std::io::Error),
    #[error("Audit serialisation failed: {0}")]
    SerialisationFailed(#[from] serde_json::Error),
}
```
