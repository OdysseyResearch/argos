# Tasks: Argos v0.1 MCP Security Proxy MVP

**Input**: Design documents from `specs/001-mcp-proxy-mvp/`\
**Prerequisites**: plan.md ✅, spec.md ✅, research.md ✅, data-model.md ✅, contracts/ ✅

**Tests**: Included — SC-007 (100% policy decision path coverage) and SC-008 (full stdio + HTTP
integration tests) are explicit, non-negotiable success criteria in the spec.

**Organization**: Tasks grouped by user story to enable independent implementation and testing.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story this task belongs to (US1–US5)
- Exact file paths included in every description

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Cargo project initialization and module stub creation.

- [x] T001 Create `Cargo.toml` with `[[bin]]` target (`argos-proxy`, `src/main.rs`) and `[lib]` target (`argos`, `src/lib.rs`); add all dependencies: `tokio` 1.x (features: full), `serde`/`serde_json` 1.x, `toml` 0.8.x, `sha2` 0.10.x, `globset` 0.4.x, `uuid` 1.x (features: v4), `clap` 4.x (features: derive), `axum` 0.7.x, `reqwest` 0.12.x (features: json, stream), `thiserror` 1.x, `anyhow` 1.x, `tokio-util` 0.7.x (features: codec), `bytes` 1.x; dev-dependencies: `tokio-test`, `tempfile`
- [x] T002 Create all source module directories and empty stub files: `src/main.rs`, `src/lib.rs`, `src/error.rs`, `src/cli/mod.rs`, `src/policy/mod.rs`, `src/policy/types.rs`, `src/policy/loader.rs`, `src/policy/engine.rs`, `src/audit/mod.rs`, `src/audit/types.rs`, `src/audit/writer.rs`, `src/transport/mod.rs`, `src/transport/stdio.rs`, `src/transport/http.rs`, `src/proxy/mod.rs`
- [x] T003 [P] Create integration test stub files: `tests/policy_engine.rs`, `tests/audit_chain.rs`, `tests/stdio_proxy.rs`, `tests/http_proxy.rs` — each with a single `#[test] fn placeholder() {}` so `cargo test` compiles
- [x] T004 [P] Create `examples/basic_policy.rs` stub with `fn main() {}` so `cargo build --example basic_policy` compiles

**Checkpoint**: `cargo build` compiles (stub modules), `cargo test` runs (placeholder tests pass)

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Shared types and CLI parsing that every user story depends on. No user story work
can begin until this phase is complete.

- [x] T005 Implement `PolicyError` and `AuditError` in `src/policy/types.rs` and `src/audit/types.rs` using `thiserror`: `NotFound`, `ParseError`, `UnsupportedVersion`, `InvalidRule`, `UnrecognisedAction` (policy); `NotWritable`, `WriteFailed`, `SerialisationFailed` (audit) — match data-model.md exactly
- [x] T006 [P] Implement shared enums in `src/audit/types.rs`: `MessageType` (`ToolsCall`, `ResourcesRead`, `ResourcesList`, `ResourcesSubscribe` with `serde` rename to `"tools/call"` etc.) and `DecisionLabel` (`Allowed`, `Blocked`, `Redacted` lowercase)
- [x] T007 [P] Implement internal transport types in `src/transport/mod.rs`: `McpRequest` (id, method, params, raw_bytes), `McpFrame` (content_length, body), `ProxySession` (session_id, policy Arc, audit Arc, config), `SessionConfig` (dry_run, max_arg_bytes, agent)
- [x] T008 Implement `CliArgs` in `src/cli/mod.rs` using `clap` derive: all flags per data-model.md (`--policy` required, `--audit-log` required, `--agent` default "unknown", `--max-arg-bytes` default 65536, `--dry-run`, `--verbose`, `--upstream`, `--bind` default "127.0.0.1", `--port` default 8080, `--tls-cert`, `--tls-key`, `server_command` last); implement startup validation: neither/both transport mode flags → error; policy file unreadable → exit 1; audit-log path unwritable → exit 1; tls-cert without tls-key (or vice versa) → exit 1
- [x] T009 Implement `src/error.rs`: top-level binary error type as an `anyhow` wrapper; implement `From<PolicyError>` and `From<AuditError>`

**Checkpoint**: `cargo build` compiles with all types and CLI parsing in place

---

## Phase 3: User Story 1 — Policy-Enforced stdio Proxy (Priority: P1) 🎯 MVP

**Goal**: A working stdio proxy that enforces a TOML policy (allow/block/redact, deny-by-default)
and writes every decision to a Merkle-chained JSONL audit log. Serves both the AppSec engineer
(US1) and the developer safety guardrails persona (US1b) — same codebase, same binary.

**Independent Test**: Spawn a mock MCP server subprocess, run `argos-proxy --policy policy.toml
--audit-log audit.jsonl -- <mock-server>`, send `tools/call` JSON-RPC requests over stdio,
verify allowed calls reach mock server, blocked calls return JSON-RPC error, every decision
appears in audit.jsonl with correct hash chain.

### Tests for User Story 1

- [x] T010 [P] [US1] Write policy engine unit tests in `tests/policy_engine.rs`: allow exact match, block exact match, redact with field stripping, wildcard `*` tool match, deny-by-default (no rule matched), resource glob match, first-match-wins order, `path_prefix` constraint (SC-007 — 100% decision path coverage required)
- [x] T011 [P] [US1] Write audit chain unit tests in `tests/audit_chain.rs`: genesis entry has 64-hex-zero `prev_hash`, `entry_hash` computed with `entry_hash=""` convention, sequential `prev_hash` chaining, serialise-then-verify round-trip (FR-011, FR-012)

### Implementation for User Story 1

- [x] T012 [P] [US1] Implement policy domain types in `src/policy/types.rs`: `PolicyAction` (allow/block/redact, serde lowercase), `PolicyDecision` (Allow, Block{reason,rule_id}, Redact{fields,rule_id}, DenyByDefault), `PolicyFile` (meta + rules vec), `PolicyMeta` (version, agent, description, session_tags), `PolicyRule` (tool, resource, action, constraints, reason, tags, redact) — match data-model.md
- [x] T013 [US1] Implement `PolicyEngine::load` in `src/policy/loader.rs`: read TOML file, deserialise into `PolicyFile`, validate `meta.version` against `SUPPORTED_VERSIONS` (`["0.1"]`), validate each rule (exactly one of tool/resource, redact action requires non-empty redact vec, recognised action), compile all `resource` patterns into a `GlobSet` via `globset` crate
- [x] T014 [US1] Implement `PolicyEngine::evaluate` in `src/policy/engine.rs`: accept `&PolicyRequest` (public type — `Tool { name, arguments }` or `Resource { uri }`); iterate rules top-to-bottom, match `tool` rules against `PolicyRequest::Tool` by tool name (exact or `*`), match `resource` rules against `PolicyRequest::Resource.uri` via compiled `GlobSet`, apply `path_prefix` constraint check on arguments, return first-matching `PolicyDecision`; return `DenyByDefault` if no rule matches; `proxy::intercept()` converts internal `McpRequest` → `PolicyRequest` before calling evaluate (FR-001–FR-008b)
- [x] T015 [US1] Implement `AuditEntry` and `RotationMarkerEntry` structs in `src/audit/types.rs` with all fields from data-model.md; derive `Serialize`/`Deserialize`; `dry_run` field uses `#[serde(skip_serializing_if = "Option::is_none")]`
- [x] T016 [US1] Implement `AuditWriter` in `src/audit/writer.rs`: `open` creates/appends `BufWriter<File>` at path, initialises `sequence=0` and `prev_hash="sha256:000...000"`, returns `Err(AuditError::NotWritable)` if path unwritable; `write` acquires `Mutex`, sets `entry.entry_hash=""`, serialises to compact JSON, computes SHA-256, sets `entry_hash="sha256:<hex>"`, writes JSONL line + `\n`, updates `prev_hash` and increments `sequence`; `flush` flushes BufWriter to disk (FR-009–FR-015)
- [x] T017 [US1] Implement Content-Length codec in `src/transport/stdio.rs`: custom `tokio_util::codec::Decoder` with state machine `ReadingHeader` → `ReadingBody(n)`, parse `Content-Length: N\r\n\r\n` header, consume exactly N bytes as UTF-8 JSON body; implement matching `Encoder` that writes `Content-Length: N\r\n\r\n<body>` (FR-017)
- [x] T018 [US1] Implement `proxy::intercept()` pipeline in `src/proxy/mod.rs`: receive `McpFrame`, parse `method` field only; if pass-through method (`initialize`, `ping`, `tools/list`, `resources/unsubscribe`, `notifications/resources/updated`, `prompts/*`, `sampling/createMessage`) → forward raw bytes unchanged; if intercepted method (`tools/call`, `resources/read`, `resources/list`, `resources/subscribe`) → parse full params → evaluate → audit → forward or block (FR-009, FR-017)
- [x] T019 [US1] Implement JSON-RPC blocked response in `src/proxy/mod.rs`: construct `{"jsonrpc":"2.0","id":<id>,"error":{"code":-32000,"message":"<reason>"}}`, encode with Content-Length framing, return to client without forwarding to upstream (FR-023)
- [x] T020 [US1] Implement malformed request handling in `src/proxy/mod.rs`: if JSON parse fails return JSON-RPC parse error `{"code":-32700,"message":"Parse error"}` (FR-024)
- [x] T021 [US1] Implement argument truncation in `src/proxy/mod.rs`: serialise arguments to string, if `len > max_arg_bytes` set `arguments = json!("<truncated>")` and `arguments_truncated = true`, otherwise log verbatim (FR-015)
- [x] T022 [US1] Implement subprocess spawn and stdio forwarding in `src/transport/stdio.rs`: `tokio::process::Command` with `stdin(Stdio::piped())`, `stdout(Stdio::piped())`; run two Tokio tasks — client→proxy→child and child→proxy→client — each decoding Content-Length frames, routing intercepted methods through `proxy::intercept()`, forwarding pass-through frames directly; detect unexpected subprocess exit, log to stderr, terminate with exit code 3 (FR-016, FR-018)
- [x] T023 [US1] Implement graceful shutdown in `src/transport/stdio.rs`: `tokio_util::sync::CancellationToken`; register `tokio::signal::ctrl_c()` and `SIGTERM` (Unix) handlers to cancel the token; on cancellation stop accepting frames, `join_all` in-flight tasks, call `audit_writer.flush().await`, `process::exit(0)` (FR-018c)
- [x] T024 [US1] Implement stderr output contract in `src/main.rs` and `src/transport/stdio.rs`: startup confirmation line to stderr (policy path, agent, mode); fatal error → eprintln + exit; `--verbose` per-request trace to stderr; stdout carries zero non-protocol bytes in stdio mode (FR-018a, FR-018b)
- [x] T025 [US1] Wire `src/main.rs`: parse `CliArgs`, run startup validation, load `PolicyEngine`, open `AuditWriter`, generate `Uuid::new_v4()` session ID, create `ProxySession`, print startup confirmation to stderr, branch on transport mode (stdio vs HTTP)
- [x] T026 [US1] Implement `src/policy/mod.rs` and `src/audit/mod.rs` `pub use` re-exports so `use argos::policy::PolicyEngine` works; implement `src/lib.rs` re-exporting both modules

### Integration Test for User Story 1

- [x] T027 [US1] Write end-to-end stdio integration test in `tests/stdio_proxy.rs`: spawn a minimal mock MCP server binary (or use a `tokio::process` echo server), invoke `argos-proxy` with a policy covering both tool and resource rules, send allow/block/redact/dry-run `tools/call` frames AND at least one allowed and one blocked `resources/read` request over stdio, assert upstream receives allowed calls, blocked calls return JSON-RPC error, every decision appears in audit.jsonl, audit chain verifies (SC-008, FR-017, FR-008a)

**Checkpoint**: US1 fully functional — stdio proxy enforces policy, writes tamper-evident audit log, exits cleanly on signal

---

## Phase 4: User Story 2 — Policy-Enforced HTTP/SSE Proxy (Priority: P2)

**Goal**: Same policy enforcement and audit behaviour as US1 but over HTTP/SSE transport,
reverse-proxying a remote MCP server.

**Independent Test**: Run `argos-proxy --policy policy.toml --audit-log audit.jsonl --upstream
http://mock-server`, send MCP tool call requests over HTTP, verify policy evaluation and audit
output match US1 behaviour.

### Tests for User Story 2

- [x] T028 [P] [US2] Write HTTP integration test in `tests/http_proxy.rs`: start a mock upstream HTTP server (e.g., `axum` test server), invoke `argos-proxy` in HTTP mode, send allow/block/dry-run `tools/call` requests, assert correct forwarding/rejection and audit entries (SC-008)

### Implementation for User Story 2

- [x] T029 [US2] Implement `axum` HTTP server in `src/transport/http.rs`: bind on `--bind`:`--port`, define catch-all route that reads request body, routes through `proxy::intercept()`, returns response
- [x] T030 [US2] Implement `reqwest` upstream client in `src/transport/http.rs`: forward allowed requests to `--upstream` URL; pipe `reqwest::Response::bytes_stream()` into `axum` streaming response body for SSE pass-through; response bytes pass through unmodified without evaluation
- [x] T031 [US2] Implement HTTP mode startup validation in `src/cli/mod.rs`: validate `--upstream` is a valid URL, `--tls-cert` and `--tls-key` files are readable (exit 1 if not), reject both `--upstream` and `server_command` together (FR-019, FR-020)
- [x] T032 [US2] Implement graceful shutdown for HTTP mode in `src/transport/http.rs`: `CancellationToken` cancels the axum listener; `join_all` in-flight request tasks; call `audit_writer.flush().await`; exit 0 (FR-018c)

**Checkpoint**: US1 + US2 both independently functional — same policy file, same audit log format, different transports

---

## Phase 5: User Story 3 — Audit Log Integrity Verification (Priority: P3)

**Goal**: `argos-proxy verify --audit-log <path>` re-reads a JSONL log, recomputes the SHA-256
hash chain, and reports whether the chain is intact or where it breaks.

**Independent Test**: Generate N log entries via US1 integration test, run `argos-proxy verify`,
assert "Chain intact: N entries verified." Mutate one byte, assert failure at the correct entry
index.

### Tests for User Story 3

- [x] T033 [P] [US3] Extend `tests/audit_chain.rs` with tamper-detection tests: generate a 5-entry log, modify one byte of entry 3, verify that the verifier reports a chain break at entry 3; verify inserting a duplicate entry is detected (SC-004)

### Implementation for User Story 3

- [x] T034 [US3] Implement `verify` subcommand parsing in `src/cli/mod.rs`: `argos-proxy verify --audit-log <path>` as a clap subcommand distinct from the proxy mode (no `--policy` required for verify)
- [x] T035 [US3] Implement verification logic in `src/main.rs`: read JSONL line-by-line, for each entry deserialise, set `entry_hash=""`, serialise to compact JSON, compute SHA-256, assert equals stored `entry_hash`; assert `prev_hash` equals previous entry's `entry_hash` (genesis: 64 zeros); print "Chain intact: N entries verified." or "Chain broken at entry M: <detail>" with exit code 0 / 1 respectively (SC-004, FR-011, FR-012)

**Checkpoint**: `argos-proxy verify` correctly validates intact logs and detects any tampering

---

## Phase 6: User Story 4 — Policy Development with Dry-Run Mode (Priority: P4)

**Goal**: `--dry-run` lets operators validate a policy against real agent traffic before enforcing
it — violations are logged but all calls pass through.

**Independent Test**: Run proxy with `--dry-run` and a policy that blocks the test tool; verify
upstream receives the call AND audit log records `decision: "blocked"` with `dry_run: true`.

### Implementation for User Story 4

- [x] T036 [US4] Implement dry-run behaviour in `src/proxy/mod.rs`: when `session.config.dry_run == true` and decision is `Block` or `DenyByDefault`, forward call to upstream instead of returning error; set `entry.dry_run = Some(true)` in the audit entry; emit `eprintln!("DRY RUN VIOLATION: ...")` to stderr (FR-021)
- [x] T037 [US4] Implement dry-run startup warning in `src/main.rs`: if `--dry-run` active, print prominent `"WARNING: DRY RUN ACTIVE — policy violations will not be enforced"` to stderr before any traffic is processed (FR-022)

**Checkpoint**: Dry-run mode observable — violations logged, traffic unblocked, stderr warnings prominent

---

## Phase 7: User Story 5 — Library API for Programmatic Integration (Priority: P5)

**Goal**: The `argos` crate is usable as a library dependency without invoking the CLI. Public
API surfaces are fully re-exported from `src/lib.rs`.

**Independent Test**: `cargo build --lib` succeeds with no warnings about unexposed types;
`cargo run --example basic_policy` runs end-to-end without subprocess invocation.

### Implementation for User Story 5

- [x] T038 [US5] Verify and finalise `src/lib.rs` re-exports: `pub use crate::policy::{PolicyEngine, PolicyRequest, PolicyFile, PolicyRule, PolicyAction, PolicyDecision, PolicyError}; pub use crate::audit::{AuditWriter, AuditEntry, AuditError}; pub use crate::audit::types::{MessageType, DecisionLabel};` — `PolicyRequest` MUST be public; internal types (`McpRequest`, `McpFrame`, `ProxySession`) MUST NOT be re-exported (FR-026, FR-027)
- [x] T039 [US5] Implement `examples/basic_policy.rs`: load a `PolicyEngine` from a temp policy file, open an `AuditWriter` to a temp file, construct a `PolicyRequest::Tool { name, arguments }` (no `McpRequest` — library users never touch internal types), call `engine.evaluate(&request)`, construct and write an `AuditEntry` via `writer.write().await`, call `writer.flush().await`; must compile and run with `cargo run --example basic_policy` (SC-009)

**Checkpoint**: `cargo build --lib` clean; `cargo run --example basic_policy` runs end-to-end; downstream crates can depend on `argos`

---

## Final Phase: Polish & Cross-Cutting Concerns

**Purpose**: Validation of non-functional success criteria and documentation accuracy.

- [x] T040 [P] Verify static binary build: `cargo build --release --target x86_64-unknown-linux-musl` (or equivalent musl target) produces a single self-contained binary with no dynamic library dependencies; confirm with `ldd target/.../argos-proxy`; confirm no outbound network calls are made during normal proxy operation (SC-010, SC-005)
- [x] T041 [P] Run full test suite and confirm 100% policy decision path coverage: `cargo test` must pass all tests including policy_engine.rs allow/block/redact/wildcard/deny-by-default paths (SC-007)
- [x] T042 [P] Measure stdio round-trip latency: in `tests/stdio_proxy.rs`, time 1000 consecutive allow-path round-trips through the proxy and assert median < 5ms (SC-001)
- [x] T043 Set `license = "AGPL-3.0-or-later"` in `Cargo.toml` and create `LICENSE` file at repo root
- [x] T044 [P] Run `cargo run --example basic_policy` end-to-end and confirm it exits cleanly with a readable audit entry in the temp file (SC-009)
- [x] T045 [P] Validate quickstart.md scenario: follow `specs/001-mcp-proxy-mvp/quickstart.md` step-by-step with a real `uvx mcp-server-filesystem` subprocess (if available) or a mock, confirm all commands work as documented
- [x] T046 [P] Update `README.md` with a Claude Code MCP config JSON block (stdio mode, argos-proxy wrapping mcp-server-filesystem) and a minimal three-rule policy file example — satisfies §9 criterion #14 (SC-006)
- [x] T047 [tooling] Create `.claude/skills/argos-sync-docs/SKILL.md` — developer tooling skill that propagates spec decisions to `docs/` markdown files; invoked as an extension hook after speckit specify/clarify/plan/analyze steps; not part of the proxy binary or library API

---

## Dependencies & Execution Order

### Phase Dependencies

- **Phase 1 (Setup)**: No dependencies — start immediately
- **Phase 2 (Foundational)**: Depends on Phase 1 — BLOCKS all user stories
- **Phase 3 (US1 — stdio proxy)**: Depends on Phase 2 — primary MVP deliverable
- **Phase 4 (US2 — HTTP proxy)**: Depends on Phase 3 (shares `proxy::intercept()`) — can start once T018 is complete
- **Phase 5 (US3 — verify)**: Depends on Phase 3 audit writer (T016) — can start once T016 is complete
- **Phase 6 (US4 — dry-run)**: Depends on Phase 3 `proxy::intercept()` (T018) — thin layer, can start once T018 is done
- **Phase 7 (US5 — library API)**: Depends on Phase 3 re-exports (T026) — can start once T026 is done
- **Final Phase**: Depends on all user story phases complete

### Within Phase 3

- T010–T012 [P]: Tests and types — write in parallel
- T013 depends on T012 (types)
- T014 depends on T013 (evaluate uses types)
- T015–T016 [P]: Audit types and writer — write in parallel with policy engine
- T017 depends on T015 (AuditEntry type)
- T018 depends on T013, T014, T016, T017
- T019–T021: Can be done alongside T018 (all in proxy/mod.rs)
- T022 depends on T017 (codec), T018 (intercept pipeline)
- T023–T024 depend on T022
- T025 depends on T022, T023, T024
- T026 depends on T012, T015 (public types exist)
- T027 depends on T025 (binary runnable)

---

## Parallel Opportunities

### Phase 3 (US1) Parallel Batch

```
# Parallel: write tests and types simultaneously
Task T010: policy_engine.rs unit tests
Task T011: audit_chain.rs unit tests
Task T012: src/policy/types.rs types
Task T015: src/audit/types.rs AuditEntry

# Sequential: engine depends on types
Task T013 (after T012): policy/loader.rs
Task T014 (after T013): policy/engine.rs

# Parallel: audit writer alongside engine
Task T016 (after T015): audit/writer.rs

# Sequential: codec → intercept → subprocess → shutdown
Task T017 → T018 → T022 → T023
```

### Phase 4+5+6+7 Parallel Batch (after T018, T016 complete)

```
# All four can proceed simultaneously once intercept pipeline is ready
Task T028+T029+T030+T031+T032: HTTP transport (US2)
Task T033+T034+T035: verify subcommand (US3)
Task T036+T037: dry-run (US4)
Task T038+T039: library API + example (US5)
```

---

## Implementation Strategy

### MVP First (US1 Only — Phases 1–3)

1. Complete Phase 1: Setup
2. Complete Phase 2: Foundational (CRITICAL — blocks all stories)
3. Complete Phase 3: US1 — stdio proxy with policy enforcement and audit log
4. **STOP and VALIDATE**: Run `tests/stdio_proxy.rs`, check audit chain, confirm SC-001 latency
5. Demo to users — this is the core value proposition

### Incremental Delivery

1. Phases 1–3 → stdio proxy MVP (demo-able)
2. Phase 4 → HTTP/SSE mode (remote server support)
3. Phase 5 → `verify` subcommand (tamper-evidence claim demonstrable)
4. Phase 6 → dry-run mode (policy development workflow)
5. Phase 7 → library API (programmatic embedding, SC-009)
6. Final Phase → polish and success criteria validation

### Parallel Developer Strategy

With two developers after Phase 2:

- Dev A: Phase 3 (US1 — stdio proxy) — critical path
- Dev B: begins Phase 7 (US5 — library types and re-exports) once Phase 2 types exist

After Phase 3:

- Dev A: Phase 4 (US2 — HTTP transport)
- Dev B: Phase 5 (US3 — verify) + Phase 6 (US4 — dry-run)

---

## Notes

- `[P]` tasks operate on different files or independent concerns — safe to parallelise
- Each user story phase produces a testable increment — stop and validate at each checkpoint
- Tests T010–T011 should be written and run before T013–T014 implementation (SC-007 requires 100% coverage, so tests define the target)
- The `entry_hash` computation convention (set `entry_hash=""`, serialise, SHA-256, set) is documented in `contracts/audit-log.md` and must be reproduced exactly in both T016 (writer) and T035 (verifier)
- Commit after each task or logical group; the pre-commit hook runs dprint — re-stage if it reformats a file
- `stdout` in stdio mode must carry zero non-MCP bytes — any `println!` instead of `eprintln!` is a correctness bug
