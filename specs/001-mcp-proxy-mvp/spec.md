# Feature Specification: Argos v0.1 MCP Security Proxy MVP

**Feature Branch**: `001-mcp-proxy-mvp`  
**Created**: 2026-04-23  
**Status**: Draft  
**Input**: Argos v0.1 MCP security proxy MVP — a single Rust binary (argos-proxy) that sits
transparently between any MCP client and any MCP server, intercepts every tool call, evaluates
it against a TOML policy file (deny by default), and writes every decision to an append-only
Merkle-chained JSONL audit log. Supports stdio mode (wraps local MCP server process) and
HTTP/SSE mode (reverse proxy in front of remote MCP server).

---

## User Scenarios & Testing *(mandatory)*

### User Story 1 — Policy-Enforced stdio Proxy (Priority: P1)

An AppSec engineer deploys `argos-proxy` between Claude Desktop and a local filesystem MCP
server. The engineer defines a TOML policy granting the agent read access to a single
workspace directory and blocking all writes and shell commands. From that point forward, every
tool call the agent attempts is evaluated against the policy: permitted reads pass through
invisibly, attempted writes are rejected with a clean error the agent can parse, and every
decision is recorded in the audit log.

**Why this priority**: This is the core value proposition. Without a working stdio proxy with
policy enforcement and audit logging, nothing else matters. It directly solves the primary
persona's problem: knowing exactly what an agent did and preventing what it shouldn't do.

**Independent Test**: Can be fully tested by spawning a mock MCP server subprocess, running
`argos-proxy --mode stdio` against it, and sending `tools/call` JSON-RPC requests — then
verifying that allowed calls reach the server, blocked calls return a valid JSON-RPC error,
and every decision appears in the audit log file.

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

### User Story 2 — Policy-Enforced HTTP/SSE Proxy (Priority: P2)

An AppSec engineer deploys `argos-proxy` as a reverse proxy in front of a remote MCP server
accessible over HTTPS. The setup is identical from a policy perspective — the same TOML format
controls which tools are allowed — but the proxy accepts HTTP/SSE connections from the MCP
client and forwards to the upstream URL.

**Why this priority**: HTTP/SSE mode is required to support remote MCP servers (GitHub, SaaS
tools, internal APIs). Without it, Argos only works for local agent deployments. However, the
policy engine and audit logic are shared with Story 1, so Story 2 is a transport extension
rather than a standalone capability.

**Independent Test**: Can be fully tested by running `argos-proxy --mode http` with a mock
upstream HTTP server, sending MCP tool call requests over HTTP, and verifying policy evaluation
and audit log output match Story 1 behaviour.

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
+ lib dual target) and by the SaaS control plane future (M7). It is not a user-facing feature
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

- What happens when the policy file is missing or unparseable at startup? The proxy must exit
  with a clear, non-zero error code and a human-readable message — it must not start with no
  policy.
- What happens when the policy file contains an unrecognised `version` value? Hard error at
  startup, not a warning.
- What happens when the upstream MCP server process (stdio mode) exits unexpectedly? The proxy
  must detect the subprocess exit, log the event, and terminate cleanly.
- What happens when the audit log file is not writable? The proxy must refuse to start — it
  must never silently drop audit entries.
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
- **FR-008**: Constraint expressions in v0.1 MUST support at minimum `path_prefix` as a key-
  value constraint on string arguments.

**Audit Writer**

- **FR-009**: Every policy evaluation decision (allow, block, redact) MUST be written to the
  audit log before the call is forwarded or the error response is returned.
- **FR-010**: Each audit log entry MUST be a single-line JSON object (JSONL format) containing:
  `timestamp`, `sequence`, `prev_hash`, `entry_hash`, `session_id`, `decision`, `tool`,
  `arguments`, `policy_rule_matched`, `reason`, `agent`, `policy_version`, `org_id`, `tenant_id`.
- **FR-011**: The `entry_hash` MUST be the SHA-256 hash of the raw JSON bytes of that entry
  (with the `entry_hash` field itself set to an empty string before hashing, or computed on the
  canonical form — the exact convention MUST be documented and tested).
- **FR-012**: The `prev_hash` of the first entry in a new log file MUST be
  `sha256:0000000000000000000000000000000000000000000000000000000000000000` (64 hex zeros).
- **FR-013**: The audit log file MUST be append-only. The proxy MUST refuse to start if the
  log file path is not writable.
- **FR-014**: The audit log schema MUST support a `rotation_marker` entry type, even if no
  `rotation_marker` entries are emitted in v0.1.
- **FR-015**: Tool call arguments MUST be logged verbatim up to a configurable maximum size.
  Arguments exceeding the maximum MUST be truncated with the truncation recorded in the entry.

**Transport — stdio**

- **FR-016**: In stdio mode, the proxy MUST wrap the upstream MCP server as a subprocess,
  forwarding stdin/stdout between the MCP client and the upstream process.
- **FR-017**: The proxy MUST parse JSON-RPC messages on the stdio channel and intercept
  `tools/call` requests for policy evaluation. All other message types MUST pass through
  unmodified.
- **FR-018**: If the upstream subprocess exits unexpectedly, the proxy MUST log the event and
  terminate cleanly.

**Transport — HTTP/SSE**

- **FR-019**: In HTTP/SSE mode, the proxy MUST accept HTTP connections from the MCP client and
  forward evaluated-and-allowed tool calls to the upstream URL over HTTP/SSE.
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
- **PolicyRule**: Single rule entry with `tool` (exact name or `*`), `action`
  (`allow`/`block`/`redact`), optional `constraints` map, optional `reason` string, and
  required `tags` array.
- **ToolCall**: Parsed `tools/call` JSON-RPC request containing `tool` name and `arguments`
  map.
- **PolicyDecision**: Evaluation result — one of `Allow`, `Block(reason)`, `Redact(fields)`.
- **AuditEntry**: Single JSONL line written to the log file. Includes all FR-010 fields plus
  optional `dry_run` flag.
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

- The MCP client speaks standard MCP JSON-RPC over stdio or HTTP/SSE — no proprietary
  extensions.
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
- Argument size truncation limit defaults to 64KB if not specified by the operator.
- The MCP JSON-RPC error code used for blocked calls is `-32000` (server error range) with a
  human-readable message — this is the standard approach until the MCP spec formalises error
  codes for security rejections.
