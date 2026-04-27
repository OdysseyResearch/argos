# Feature Specification: Argos v0.1 MCP Security Proxy MVP

**Feature Branch**: `001-mcp-proxy-mvp`\
**Created**: 2026-04-23\
**Status**: Draft\
**Input**: Argos v0.1 MCP security proxy MVP — a single Rust binary (argos-proxy) that sits
transparently between any MCP client and any MCP server, intercepts every tool call, evaluates
it against a TOML policy file (deny by default), and writes every decision to an append-only
Merkle-chained JSONL audit log. Supports stdio mode (wraps local MCP server process) and
HTTP/SSE mode (reverse proxy in front of remote MCP server).

---

## User Scenarios & Testing *(mandatory)*

### User Story 1 — Policy-Enforced stdio Proxy (Priority: P1)

An AppSec engineer deploys `argos-proxy` between Claude Code (or Roo Code / GitHub Copilot
agent) and a local filesystem MCP server. The engineer defines a TOML policy granting the
agent read access to a single workspace directory and blocking all writes and shell commands.
From that point forward, every tool call the agent attempts is evaluated against the policy:
permitted reads pass through invisibly, attempted writes are rejected with a clean error the
agent can parse, and every decision is recorded in the audit log.

**Why this priority**: This is the core value proposition. Without a working stdio proxy with
policy enforcement and audit logging, nothing else matters. It directly solves the primary
persona's problem: knowing exactly what an agent did and preventing what it shouldn't do.

**Independent Test**: Can be fully tested by spawning a mock MCP server subprocess and running
`argos-proxy --policy policy.toml --audit-log audit.jsonl -- <mock-server>`, then sending
`tools/call` JSON-RPC requests — verifying that allowed calls reach the server, blocked calls
return a valid JSON-RPC error, and every decision appears in the audit log file.

**Acceptance Scenarios**:

1. **Given** a valid TOML policy with an `allow` rule for `read_file` and a `block` rule for
   `write_file`, **When** the agent calls `read_file` with a path inside the permitted prefix,
   **Then** the call passes through to the upstream MCP server unmodified and the audit log
   records a `decision: "allowed"` entry with full tool call arguments.

2. **Given** the same policy, **When** the agent calls `write_file`, **Then** the proxy returns
   a valid MCP JSON-RPC error response to the client (not an empty response, not a crash), the
   upstream server never receives the call, and the audit log records a `decision: "blocked"`
   entry with the matched rule and reason.

3. **Given** a tool call for a tool not mentioned in the policy (e.g., `run_terminal_cmd`),
   **When** the call arrives, **Then** it is blocked by the implicit deny-by-default rule and
   logged accordingly — no explicit `*` catch-all rule is required in the policy file.

4. **Given** a `redact` rule for `read_file` that strips a `token` argument field, **When**
   the agent calls `read_file` with both `path` and `token` arguments, **Then** only `path`
   reaches the upstream server and the audit log records a `decision: "redacted"` entry noting
   which field was stripped.

5. **Given** `--dry-run` mode is active, **When** a tool call matches a `block` rule, **Then**
   the call passes through to the upstream server (not blocked), a warning is logged loudly to
   stderr, and the audit log records the violation with a `dry_run: true` flag.

---

### User Story 1b — Developer Safety Guardrails (Priority: P1, co-primary persona)

An individual developer is experimenting with a locally-run AI agent — Claude Code, Roo Code,
or an open-source agentic framework. They are excited about what the agent can do but uneasy
about what it might accidentally do: touch files outside the project, make unexpected network
calls, overwrite something irreversible. They are not a security engineer and have no compliance
requirement. They want structural peace of mind, not a compliance dashboard.

They write a five-line policy — allow reads in the workspace, block everything else — drop
`argos-proxy` into their MCP config, and run their agent as normal. If the agent tries to step
outside the boundary, it's blocked with a clear error. After the session, they can review the
audit log to see exactly what happened.

**Why this priority**: This persona is the primary driver of FOSS adoption. Developers on
GitHub, Hacker News, and dev communities are the ones who star projects, write tutorials, and
create the social proof that AppSec engineers later use to justify enterprise evaluation. Without
this persona, Argos reaches enterprises slowly through formal evaluation cycles. With it, Argos
spreads organically through the developer community first.

**Independent Test**: Can be fully tested with a minimal two-rule policy (allow one tool, block
`*`), a mock MCP server, and verification that out-of-scope tool calls are blocked with a
human-readable error while in-scope calls pass through — with the full session auditable in the
JSONL log afterwards.

**Acceptance Scenarios**:

1. **Given** a developer with no security background writes a minimal policy (`allow read_file`
   in their workspace, implicit deny-by-default for everything else), **When** they run their
   agent through `argos-proxy`, **Then** the agent operates normally within the allowed boundary
   and receives a clear, non-crashing error on any out-of-scope attempt — without the developer
   needing to understand JSON-RPC or MCP internals.

2. **Given** the agent attempts to call a tool not listed in the policy (e.g., `run_terminal_cmd`
   or `write_file`), **When** the call is blocked, **Then** the error message returned to the
   agent is human-readable enough that the developer can understand from the agent's output what
   was blocked and why.

3. **Given** the session ends, **When** the developer opens the audit JSONL file, **Then** every
   tool call and resource access is listed in chronological order with its decision — giving the
   developer a complete, human-readable record of everything the agent did or attempted.

---

### User Story 2 — Policy-Enforced HTTP/SSE Proxy (Priority: P2)

An AppSec engineer deploys `argos-proxy` as a reverse proxy in front of a remote MCP server
accessible over HTTPS. The setup is identical from a policy perspective — the same TOML format
controls which tools are allowed — but the proxy accepts HTTP/SSE connections from the MCP
client and forwards to the upstream URL.

**Why this priority**: HTTP/SSE mode is required to support remote MCP servers (GitHub, SaaS
tools, internal APIs). Without it, Argos only works for local agent deployments. However, the
policy engine and audit logic are shared with Story 1, so Story 2 is a transport extension
rather than a standalone capability.

**Independent Test**: Can be fully tested by running
`argos-proxy --policy policy.toml --audit-log audit.jsonl --upstream http://mock-server` with
a mock upstream HTTP server, sending MCP tool call requests over HTTP, and verifying policy
evaluation and audit log output match Story 1 behaviour.

**Acceptance Scenarios**:

1. **Given** `argos-proxy` in HTTP mode with an upstream URL, **When** the MCP client sends a
   `tools/call` request over HTTP/SSE, **Then** the proxy evaluates the call against the
   loaded policy identically to stdio mode and forwards or rejects accordingly.

2. **Given** TLS certificate flags (`--tls-cert`, `--tls-key`) are provided on the CLI,
   **When** the proxy starts, **Then** it accepts the TLS configuration without error even if
   mTLS is not enforced in v0.1 — the configuration path must be open for v1.0 mTLS.

3. **Given** an upstream that returns a slow response, **When** the proxy forwards a call,
   **Then** the audit log entry is still written and the client receives the upstream response
   or a timeout error — the proxy does not silently drop calls.

---

### User Story 3 — Audit Log Integrity Verification (Priority: P3)

A security auditor or automated compliance tool verifies that the audit log produced by
`argos-proxy` has not been tampered with. Each JSONL entry chains to the previous via a
SHA-256 hash, making any insertion, deletion, or modification detectable by re-computing the
chain.

**Why this priority**: Tamper-evidence is what differentiates the Argos audit log from a plain
text log. Without it, the log has no forensic value. This is a correctness property of Story 1
and 2, not a separate UI feature — it must pass automatically as part of Story 1 testing.

**Independent Test**: After a test session generates N log entries, an independent verifier
script can re-read the JSONL file, compute `SHA-256(raw_entry_bytes)` for each entry, and
confirm that each entry's `prev_hash` equals the hash of the preceding entry (with the genesis
entry using `sha256:0000...0000`).

**Acceptance Scenarios**:

1. **Given** a fresh log file, **When** the first audit entry is written, **Then** its
   `prev_hash` field is `sha256:` followed by 64 hex zeros (the genesis marker) and its
   `entry_hash` is the SHA-256 of the raw JSON bytes of that entry.

2. **Given** a log with N entries, **When** any byte of any entry is modified externally,
   **Then** a verification pass re-computing the hash chain detects the break at the modified
   entry.

3. **Given** `org_id` and `tenant_id` fields are present in every entry (nullable in v0.1),
   **When** the log is read by a future SaaS ingestion service, **Then** the fields can be
   populated without schema migration.

---

### User Story 4 — Policy Development with Dry-Run Mode (Priority: P4)

An AppSec engineer writes an initial policy and wants to validate it against real agent
behaviour before enforcing it. `--dry-run` mode lets them observe what would be blocked without
actually disrupting the agent — violations are logged and warned loudly but all calls pass
through.

**Why this priority**: Without dry-run, initial policy development requires either risking
agent disruption or testing against synthetic traffic. Dry-run reduces the barrier to adoption.
It also validates the logging pipeline independently of enforcement.

**Independent Test**: Can be tested by running the proxy with `--dry-run` and a policy that
would block the test tool call — verifying that the upstream receives the call AND the audit
log records the violation.

**Acceptance Scenarios**:

1. **Given** `--dry-run` is active and a `block` rule matches a call, **When** the call
   arrives, **Then** it is forwarded to upstream (not blocked), a warning is emitted to stderr,
   and the audit log entry carries `dry_run: true` alongside `decision: "blocked"`.

2. **Given** `--dry-run` is active, **When** the proxy starts, **Then** a prominent warning
   is printed to stderr indicating that dry-run mode is active and no enforcement is occurring.

---

### User Story 5 — Library API for Programmatic Integration (Priority: P5)

A platform engineer embeds the Argos policy engine and audit writer into a custom Rust
application (e.g., an agent runtime or orchestration layer) without running `argos-proxy` as a
subprocess. They use the `argos` crate as a library dependency.

**Why this priority**: The library API is architecturally required by the constitution (binary

- lib dual target) and by the SaaS control plane future (M7). It is not a user-facing feature
  in v0.1 but must be exposed and documented as a public API so downstream crates can depend on
  it.

**Independent Test**: The `argos` crate must compile as a library (`cargo build --lib`) with
the policy engine and audit writer accessible as public API surfaces. A minimal example binary
in `examples/` should demonstrate loading a policy and evaluating a tool call.

**Acceptance Scenarios**:

1. **Given** the `argos` crate is added as a dependency in another Rust project, **When** the
   dependent project calls `PolicyEngine::load()` and `PolicyEngine::evaluate()`, **Then** it
   receives a policy decision without invoking any CLI or subprocess.

2. **Given** the `argos` crate is compiled with `cargo build --lib`, **When** the build
   completes, **Then** no errors or warnings about private/unexposed types are present for the
   policy engine and audit writer modules.

---

### Edge Cases

- What happens when `--policy` is omitted entirely? The proxy must exit with code 1 and a
  human-readable error — identical to the file-missing case. There is no default policy path.
- What happens when the policy file is missing or unparseable at startup? The proxy must exit
  with a clear, non-zero error code and a human-readable message — it must not start with no
  policy.
- What happens when the policy file contains an unrecognised `version` value? Hard error at
  startup, not a warning.
- What happens when the upstream MCP server process (stdio mode) exits unexpectedly? The proxy
  must detect the subprocess exit, log the event, and terminate cleanly.
- What happens when SIGTERM or SIGINT is received? The proxy stops accepting new requests,
  drains any in-flight requests to completion (audit entry written before response sent), flushes
  all buffered audit data to disk, and exits with code 0.
- What do exit codes mean? 0 = clean shutdown; 1 = startup or policy error; 2 = runtime audit
  write failure; 3 = upstream subprocess failure. These are documented and stable for use by
  process supervisors and shell scripts.
- What happens when the audit log file is not writable at startup? The proxy must refuse to
  start — it must never silently drop audit entries.
- What happens when an audit write fails mid-session (e.g., disk full)? The in-flight call is
  blocked, a JSON-RPC error is returned to the client, the failure is logged to stderr, and the
  proxy terminates. There is no degraded/continue mode — the audit contract is non-negotiable.
- What happens when a tool call argument is very large (e.g., 100MB file content passed inline)?
  The proxy must not OOM — argument logging should truncate at a configurable maximum (with the
  truncation recorded in the audit entry).
- What happens when the policy contains a rule with an unrecognised action type? Hard error at
  startup with the offending rule identified.
- What happens when the `tools/call` request is malformed JSON-RPC? The proxy returns a valid
  JSON-RPC parse error response and logs the malformed request.
- What happens when both an exact tool rule and a wildcard `*` rule exist? The evaluation
  order must be deterministic — first matching rule wins (top-to-bottom order in the policy
  file), and this is documented.

---

## Clarifications

### Session 2026-04-23

- Q: How does `argos-proxy` appear in the MCP client configuration for stdio mode — does it replace the server command, run as a daemon, or install a shim? → A: Option A — `argos-proxy` replaces the server command directly in the client config, spawning the real MCP server as a subprocess via a `--` separator (e.g., `argos-proxy --policy policy.toml -- uvx mcp-server-filesystem /workspace`). No daemon, no socket file, no PATH shim.
- Q: What is the CLI contract for the HTTP listener address and port? → A: Option A — explicit `--bind <addr>` and `--port <port>` flags with defaults of `127.0.0.1` and `8080` respectively (localhost-only by default, overridable for containerised deployments).
- Q: Should Argos intercept and enforce policy on non-`tools/call` MCP message types in v0.1? → A: `resources/read` and `resources/list` are in v0.1 enforcement scope — they expose private data equivalent to tool calls and omitting them creates a documented bypass undermining the deny-by-default claim. `prompts/*` and `sampling/createMessage` are deferred to M2. `tools/list` and protocol messages (`initialize`, `ping`) pass through silently with no audit entry.
- Q: Is `--mode` an explicit required flag or is the transport mode inferred from other flags? → A: Option B — mode inferred: `--` presence → stdio; `--upstream <url>` presence → HTTP. No `--mode` flag. Conflicting or ambiguous flag combinations are a hard startup error.
- Q: Is `--audit-log <path>` a required CLI flag or does it default to a path when omitted? → A: Required flag — the proxy refuses to start if `--audit-log` is not provided. No default path. Operators must make a conscious, explicit decision about where audit records land.

### Session 2026-04-24

- Q: Should `resources/subscribe` be in v0.1 enforcement scope given it delivers the same data as `resources/read` via push updates? → A: Yes — `resources/subscribe` is enforced in v0.1. A blocked subscription is rejected before the server processes it; `resources/unsubscribe` and `notifications/resources/updated` pass through freely (the latter only arrives for already-allowed subscriptions).
- Q: If an audit entry cannot be written mid-session (e.g., disk full), should the proxy halt or continue? → A: Fatal — block the in-flight call, return a JSON-RPC error to the client, log the failure to stderr, and terminate. The audit contract (every decision audited before action) is binary; a configurable degraded mode would make the log forensically untrustworthy and indistinguishable from tampering.
- Q: What is the concurrency model for simultaneous intercepted requests? → A: Option B — concurrent processing with a mutex around only the audit write step. Claude Code and Roo Code (the primary targets) both emit parallel tool calls; serializing all requests would cause visible latency on every parallel tool use. Policy evaluation is stateless and parallelizes freely; only the audit log chain write requires a short-held lock for deterministic `prev_hash` ordering.
- Q: How are resource URIs matched in policy rules — exact, prefix, or glob? → A: Option C — glob patterns (e.g., `resource = "file:///workspace/src/**"`). More explicit than prefix matching, eliminates the prefix footgun (`file:///workspace` silently matching `file:///workspace-backup/`), consistent with developer mental models (gitignore, shell), and consistent with the existing `tool = "*"` wildcard. Implementation via the `globset` crate (zero-ReDoS).
- Q: Which MCP protocol version does Argos v0.1 target? → A: Version-agnostic (Option C) — the proxy intercepts by JSON-RPC method name, not by MCP version. `initialize` passes through unmodified so client and server self-negotiate their version. Argos documents the method names it intercepts (stable across MCP 2024-11-05 and 2025-03-26) and requires no version declaration. Future version compatibility is provided by method-name stability.

### Session 2026-04-24 (continued)

- Q: What is the developer persona's primary job-to-be-done with `argos-proxy`, and should they be a co-primary persona alongside the AppSec engineer? → A: Yes, co-primary. Safety guardrails first: the developer persona is an enthusiastic early adopter of agentic AI (Claude Code, Roo Code, open-source agent frameworks) who wants structural protection from accidental or unexpected agent behaviour — not compliance evidence. They reach for `argos-proxy` because they want to try new agentic tools without fear, not because they have a CISO to satisfy. The audit log is a welcome byproduct; the deny-by-default policy is the hero feature for them.
- Q: What stdio wire framing does `argos-proxy` use, and which MCP clients must it be compatible with? → A: Content-Length header framing (MCP/LSP standard): `Content-Length: <N>\r\n\r\n<JSON>`. This is the MCP wire protocol — not a choice but a compatibility requirement. Target clients: Claude Code, Roo Code, GitHub Copilot agent, Goose (Block), and other MCP-compliant open-source agent frameworks. Any client implementing the MCP spec is automatically compatible.
- Q: What is the stderr output contract in stdio mode? → A: Option A — stdout is exclusively MCP protocol (zero non-protocol bytes permitted); stderr carries all operator-facing output as human-readable plaintext. Safety-critical messages (startup confirmation, policy loaded, dry-run active warning, fatal errors) always appear on stderr regardless of flags. Verbose per-request trace logging is off by default and enabled with `--verbose`. This follows LSP/MCP convention: all target clients (Claude Code, VS Code, Roo Code) already capture stderr into their log panels.
- Q: What is the graceful shutdown and exit code contract? → A: Option A — SIGTERM/SIGINT: drain in-flight requests, flush buffered audit entries to disk (ensure no audit gap at shutdown), then exit. Documented exit codes: 0 = clean shutdown, 1 = startup/policy error, 2 = audit write failure, 3 = upstream subprocess failure. B and C were rejected because immediate exit without draining leaves in-flight calls with no audit entry, breaking FR-009 at shutdown time.
- Q: How does an operator configure the argument size truncation limit? → A: Option A — `--max-arg-bytes <N>` CLI flag, default 65536 (64KB), applied uniformly to all audit entries in the session. Policy meta override deferred to M2 to avoid mixing operational config into policy files and to keep the audit writer independent of the policy engine.

### Session 2026-04-24 (gap fill)

- Q: Is `--policy <path>` a required CLI flag or does it have a default? → A: Required flag — the proxy refuses to start if `--policy` is omitted, with a clear human-readable error. No default path. Consistent with `--audit-log` (also required, no default) — operators must make an explicit, conscious decision about which policy governs the session.
- Q: How is the `agent` field in audit log entries populated? → A: Optional `--agent <name>` CLI flag; defaults to `"unknown"` if omitted. The operator labels the session at launch (e.g., `--agent claude-code`). No protocol interaction required — works identically in stdio and HTTP mode. Future MCP client identity negotiation (M2+) can populate this automatically without a schema change.

---

## Requirements *(mandatory)*

### Compatibility Constraints (mandatory validation)

*Reference: `docs/product/ARGOS_V01_IDEA.md` §13. Each constraint this feature touches.*

- [x] Audit log schema includes `org_id`, `tenant_id` (nullable), `rotation_marker` entry type
- [x] Policy file has `version` field validated at load time
- [x] Session IDs are UUID v4
- [x] Hash function is SHA-256
- [x] OTel span emission is architecturally possible (pipeline not closed)
- [x] `argos` crate remains buildable as both binary and library
- [x] MCP error responses are spec-compliant JSON-RPC
- [x] HTTP/SSE mode accepts TLS certificate configuration
- [x] Audit log JSONL format supports `rotation_marker` entry type
- [x] Policy rules carry a `tags` field

### Functional Requirements

**Policy Engine**

- **FR-001**: The policy engine MUST evaluate each tool call against the loaded TOML policy
  and return one of three decisions: `allow`, `block`, or `redact`.
- **FR-002**: The default evaluation result for any tool call not matched by any explicit rule
  MUST be `block` (deny by default). No configuration option may change this default.
- **FR-003**: Rule matching MUST use first-match-wins semantics in top-to-bottom order as they
  appear in the policy file.
- **FR-004**: The policy MUST support exact tool name matching (e.g., `tool = "read_file"`)
  and wildcard matching via `tool = "*"`.
- **FR-005**: Each policy rule MUST carry a `tags` field (empty array acceptable) for future
  compliance template mapping.
- **FR-006**: The policy file MUST have a `version` field. Any unrecognised version value MUST
  produce a hard error at load time, not a warning.
- **FR-007**: Redaction rules MUST specify which argument fields to strip. The stripped call
  MUST be forwarded to the upstream server; the redacted fields MUST be recorded in the audit
  entry.
- **FR-008**: Tool rule constraint expressions in v0.1 MUST support at minimum `path_prefix` as
  a key-value constraint on string arguments. Resource URI matching uses glob patterns on the
  `resource` field directly (FR-008a) rather than a separate constraint expression.
- **FR-008a**: Policy rules MUST support a `resource` field using glob pattern syntax (e.g.,
  `resource = "file:///workspace/src/**"`) in addition to the `tool` field, enabling access
  control over `resources/read`, `resources/list`, and `resources/subscribe` requests. `"*"`
  remains the catch-all wildcard. A rule MAY specify either `tool` or `resource`, but not both.
  Glob matching MUST be implemented with a zero-ReDoS library.
- **FR-008b**: The deny-by-default rule MUST apply to `resources/read`, `resources/list`, and
  `resources/subscribe` requests identically to `tools/call` — any resource access not
  explicitly permitted by a `resource` rule is blocked. `resources/unsubscribe` and
  `notifications/resources/updated` pass through without policy evaluation.

**Audit Writer**

- **FR-009**: Every policy evaluation decision (allow, block, redact) MUST be written to the
  audit log before the call is forwarded or the error response is returned.
- **FR-010**: Each audit log entry MUST be a single-line JSON object (JSONL format) containing:
  `timestamp`, `sequence`, `prev_hash`, `entry_hash`, `session_id`, `message_type`
  (`"tools/call"`, `"resources/read"`, `"resources/list"`, or `"resources/subscribe"`),
  `decision`, `tool_or_resource`, `arguments`, `policy_rule_matched`, `reason`, `agent`
  (sourced from `--agent` flag, defaults to `"unknown"` — see FR-030), `policy_version`,
  `org_id`, `tenant_id`.
- **FR-011**: The `entry_hash` MUST be the SHA-256 hash of the raw JSON bytes of that entry
  (with the `entry_hash` field itself set to an empty string before hashing, or computed on the
  canonical form — the exact convention MUST be documented and tested).
- **FR-012**: The `prev_hash` of the first entry in a new log file MUST be
  `sha256:0000000000000000000000000000000000000000000000000000000000000000` (64 hex zeros).
- **FR-029**: `--policy <path>` is a required CLI flag with no default.
- **FR-030**: `--agent <name>` is an optional CLI flag. When provided, its value is written to the `agent` field of every audit log entry in the session. When omitted, `agent` defaults to `"unknown"`. This is the sole source of agent identity in v0.1; automatic population from MCP client identity negotiation is deferred to M2.
- **FR-013**: `--audit-log <path>` is a required CLI flag with no default. The proxy MUST
  refuse to start if the flag is omitted or if the specified path is not writable. The log file
  is opened in append-only mode.
- **FR-013a**: If an audit write fails at runtime (e.g., disk full, file system error), the
  proxy MUST block the in-flight call, return a valid JSON-RPC error to the client, emit the
  failure reason to stderr, and terminate. There is no configurable degraded mode — the audit
  contract is non-negotiable and cannot be bypassed via flags.
- **FR-014**: The audit log schema MUST support a `rotation_marker` entry type, even if no
  `rotation_marker` entries are emitted in v0.1.
- **FR-015**: Tool call arguments MUST be logged verbatim up to the limit set by
  `--max-arg-bytes <N>` (default 65536). Arguments exceeding the limit MUST be truncated and
  the truncation recorded in the audit entry. The limit applies uniformly to all entries in the
  session; per-policy override is deferred to M2.

**Transport — stdio**

- **FR-016**: In stdio mode, `argos-proxy` MUST be invokable as a drop-in replacement for the
  upstream server command in the MCP client configuration, using a `--` separator to pass the
  real server command and its arguments (e.g., `argos-proxy --policy policy.toml -- <server-cmd>
  [args...]`). The proxy spawns the server as a child process and forwards stdin/stdout between
  the MCP client and the child. stdio mode is activated by the presence of a `--` separator; no
  `--mode` flag is required or accepted.
- **FR-017**: The proxy MUST parse JSON-RPC messages on the stdio channel using
  Content-Length header framing (the MCP/LSP wire protocol: `Content-Length: <N>\r\n\r\n`
  followed by exactly N bytes of UTF-8 JSON). This framing is required for compatibility with
  Claude Code, Roo Code, GitHub Copilot agent, Goose, and all MCP-compliant clients. The proxy
  MUST intercept `tools/call`, `resources/read`, `resources/list`, and `resources/subscribe`
  requests for policy evaluation. `resources/unsubscribe`, `notifications/resources/updated`, `tools/list`,
  `initialize`, `ping`, and all other protocol messages MUST pass through unmodified with no
  audit entry written. `prompts/*` and `sampling/createMessage` pass through unmodified in v0.1
  (enforcement deferred to M2).
- **FR-018**: If the upstream subprocess exits unexpectedly, the proxy MUST log the event to
  stderr and terminate with exit code 3.
- **FR-018c**: On SIGTERM or SIGINT, the proxy MUST stop accepting new requests, allow all
  in-flight requests to complete and their audit entries to be written, flush all buffered audit
  data to disk, and exit with code 0. Exit codes: 0 = clean shutdown; 1 = startup/policy error;
  2 = runtime audit failure; 3 = upstream subprocess failure. These codes are stable and
  documented for use by process supervisors and shell scripts.
- **FR-018a**: In stdio mode, stdout MUST carry exclusively MCP protocol bytes. Zero non-protocol
  bytes (status messages, warnings, logs) are permitted on stdout. All operator-facing output
  MUST go to stderr.
- **FR-018b**: The following messages MUST always appear on stderr regardless of flags: (1)
  startup confirmation including policy file path, agent name, and mode; (2) "DRY RUN ACTIVE" warning when
  `--dry-run` is set; (3) all fatal error messages with a human-readable description and
  non-zero exit. Per-request trace logging is suppressed by default and enabled with `--verbose`.

**Transport — HTTP/SSE**

- **FR-019**: HTTP/SSE mode is activated by the presence of `--upstream <url>`. The proxy MUST
  accept HTTP connections on the address and port specified by `--bind <addr>` (default
  `127.0.0.1`) and `--port <port>` (default `8080`), and forward evaluated-and-allowed requests
  to the upstream URL over HTTP/SSE. Providing both `--upstream` and a `--` server command MUST
  be a hard startup error.
- **FR-020**: The CLI MUST accept `--tls-cert` and `--tls-key` flags in HTTP mode and validate
  that the files are readable at startup, even if mTLS enforcement is deferred to v1.0.

**Dry-Run Mode**

- **FR-021**: When `--dry-run` is active, calls that would be blocked MUST pass through to
  upstream, a warning MUST be emitted to stderr, and the audit log entry MUST carry
  `dry_run: true`.
- **FR-022**: On startup in dry-run mode, a prominent warning MUST be printed to stderr.

**MCP Protocol Compliance**

- **FR-023**: When a tool call is blocked, the proxy MUST return a valid JSON-RPC 2.0 error
  response with an MCP-compliant error code and a human-readable message. The response MUST
  NOT be empty or malformed.
- **FR-024**: When a JSON-RPC request is unparseable, the proxy MUST return a valid JSON-RPC
  parse error response.

**Concurrency**

- **FR-028**: The proxy MUST process multiple simultaneous intercepted requests concurrently.
  Policy evaluation is stateless and MUST NOT be serialized across requests. The audit log
  writer MUST use a short-held lock (held only for the duration of the file write) to ensure
  deterministic `prev_hash` chain ordering without blocking concurrent policy evaluation.

**Session and Identity**

- **FR-025**: A UUID v4 session ID MUST be generated once per proxy invocation and included in
  every audit log entry for that session.

**Library API**

- **FR-026**: The `argos` crate MUST be buildable as both a `[[bin]]` and a `[lib]` target in
  the same Cargo workspace.
- **FR-027**: The policy engine and audit writer MUST be exposed as public library APIs, usable
  without invoking the CLI.

### Key Entities

- **PolicyFile**: TOML document with `[meta]` section (`version`, `agent`, `description`,
  `session_tags`) and ordered `[[rules]]` array. Validated at load time.
- **PolicyRule**: Single rule entry with either `tool` (exact name or `*`) or `resource` (URI
  string or `*`) — not both — plus `action` (`allow`/`block`/`redact`), optional `constraints`
  map, optional `reason` string, and required `tags` array.
- **ToolCall**: Parsed `tools/call` JSON-RPC request containing `tool` name and `arguments`
  map.
- **PolicyDecision**: Evaluation result — one of `Allow`, `Block(reason)`, `Redact(fields)`.
- **AuditEntry**: Single JSONL line written to the log file. Includes all FR-010 fields plus
  optional `dry_run` flag. The `message_type` field distinguishes `tools/call`,
  `resources/read`, `resources/list`, and `resources/subscribe` entries.
- **Session**: A single proxy invocation, identified by a UUID v4 session ID, with a reference
  to the loaded policy and the open audit log writer.

---

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: A tool call that matches an `allow` rule in a loaded policy reaches the upstream
  MCP server unmodified, with the round-trip overhead below 5ms on a standard developer
  machine (measured end-to-end through the proxy, excluding upstream processing time).
- **SC-002**: A tool call that matches a `block` rule never reaches the upstream MCP server,
  and the MCP client receives a valid, parseable error response within the same latency window.
- **SC-003**: Every tool call — allowed, blocked, or redacted — produces exactly one audit log
  entry before the proxy responds to the client.
- **SC-004**: A hash chain over N audit log entries can be independently verified in O(N) time
  by any tool that can read JSONL and compute SHA-256 — no special Argos tooling required.
- **SC-005**: The proxy starts, enforces policy, and writes a complete audit log without
  network access — the full workflow is air-gap compatible.
- **SC-006**: All 16 success criteria from `docs/product/ARGOS_V01_IDEA.md` §9 are met.
- **SC-007**: 100% of policy engine decision paths (allow, block, redact, wildcard, deny-by-
  default) are covered by automated unit tests.
- **SC-008**: The full stdio and HTTP/SSE proxy flows each have at least one end-to-end
  integration test covering the allow, block, and redact paths.
- **SC-009**: The `argos` library crate can be imported and used to evaluate a policy decision
  in a test harness with zero CLI invocation.
- **SC-010**: `argos-proxy` builds to a single static binary with no runtime dependencies on
  a standard CI environment.

---

## Assumptions

- There are two co-primary personas: (1) the **developer/enthusiast** — an individual running
  agentic AI tools locally (Claude Code, Roo Code, GitHub Copilot agent, open-source agent
  frameworks) who wants structural safety guardrails without needing security expertise; (2) the
  **AppSec engineer** — a platform security engineer deploying `argos-proxy` organisation-wide
  to enforce capability policy and produce audit evidence. FOSS adoption is driven by persona 1;
  enterprise conversion is driven by persona 2.
- The primary deployment targets for v0.1 are developer tooling environments: Claude Code
  (CLI), Roo Code (VS Code extension), GitHub Copilot agent, and open-source agentic frameworks
  running locally. These clients use stdio mode — `argos-proxy` replaces the server command in
  their respective MCP configuration. The spec is client-agnostic but test examples and
  documentation are written for dev tooling first.
- The MCP client speaks standard MCP JSON-RPC over stdio or HTTP/SSE — no proprietary
  extensions.
- Argos is MCP version-agnostic. It intercepts by JSON-RPC method name; `initialize` passes
  through so client and server negotiate their own protocol version. The intercepted method
  names (`tools/call`, `resources/read`, `resources/list`, `resources/subscribe`) are stable
  across MCP 2024-11-05 and 2025-03-26. No version declaration or validation is performed by
  the proxy.
- The stdio wire protocol is Content-Length header framing (MCP/LSP standard). Any
  MCP-compliant client is automatically compatible: Claude Code, Roo Code, GitHub Copilot
  agent, Goose (Block), and open-source agent frameworks implementing the MCP spec.
- `prompts/*` and `sampling/createMessage` MCP message types pass through unmodified in v0.1
  with no policy evaluation and no audit entry. Operators using these primitives are not covered
  by Argos enforcement until M2. This is an explicit, documented scope boundary.
- `tools/list`, `initialize`, `ping`, and other protocol handshake messages pass through
  silently — they carry no capability-exercise risk.
- v0.1 operates in single-agent, single-session mode. Multi-agent session management is
  deferred to M6.
- Policy files are operator-managed (not agent-managed). The agent cannot modify its own
  policy.
- The audit log is a local file. Remote log sinks (S3, syslog, OTel collector) are deferred
  to M4.
- v0.1 constraint expressions support `path_prefix` only. Richer constraint operators
  (`regex`, `one_of`, `contains`) are M2 scope.
- Wildcard matching in v0.1 is limited to the literal `*` catch-all. Glob patterns like
  `read_*` are deferred to M2.
- Policy hot-reload is not supported. A policy change requires a proxy restart. This is
  explicitly deferred to M6.
- No UI, dashboard, or web interface. All interaction is CLI + file-based.
- The proxy runs on Linux and macOS. Windows support is not a v0.1 requirement.
- Argument size truncation limit is set via `--max-arg-bytes <N>`, defaulting to 65536 (64KB).
  Per-policy override is deferred to M2 to keep the audit writer independent of the policy engine.
- The MCP JSON-RPC error code used for blocked calls is `-32000` (server error range) with a
  human-readable message — this is the standard approach until the MCP spec formalises error
  codes for security rejections.
