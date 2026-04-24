# Research: Argos v0.1 MCP Security Proxy MVP

**Branch**: `001-mcp-proxy-mvp` | **Date**: 2026-04-24

---

## 1. MCP/LSP Content-Length Framing in Rust async

**Decision**: Implement a custom `tokio_util::codec::Decoder`/`Encoder` pair for Content-Length
framing rather than using line-based codecs.

**Rationale**: The MCP wire protocol uses `Content-Length: N\r\n\r\n<N bytes of JSON>` framing
(identical to LSP). Standard line codecs (`LinesCodec`) cannot handle this because JSON payloads
may contain newlines. A custom decoder reads the header line-by-line until the blank line, then
calls `read_exact` for exactly N bytes. This is the same approach used by `tower-lsp` and the
official LSP SDKs.

**Implementation pattern**:

```rust
// State machine: ReadingHeader → ReadingBody(n)
// Header: accumulate lines until "\r\n" on empty line
// Body: read exactly n bytes as UTF-8 JSON
```

**Alternatives considered**:

- `tokio::io::AsyncBufReadExt::read_line` in a loop — workable but more error-prone at the
  header/body boundary; the codec abstraction is cleaner and composable with `FramedRead`.
- `tower-lsp`'s codec — could be reused directly, but adds a transitive dependency on the full
  LSP stack. Implementing the framing ourselves is ~80 lines and keeps the dependency tree minimal.

---

## 2. HTTP Reverse Proxy Stack

**Decision**: `axum` 0.7.x for the inbound HTTP server; `reqwest` 0.12.x for the upstream HTTP
client. Both are built on `hyper` 1.x and `tokio`.

**Rationale**: axum is the ergonomic standard for Tokio-based HTTP servers in Rust. reqwest
handles streaming response bodies (needed for SSE) natively via `Response::bytes_stream()`.
Together they avoid duplicating hyper connection management. The SSE forwarding path is:
incoming HTTP request → policy evaluation → reqwest streaming forward → stream bytes to axum
response body.

**Alternatives considered**:

- Raw `hyper` — gives more control but requires manual connection pooling, routing, and request
  construction. Not worth the complexity for a reverse proxy with no custom routing logic.
- `pingora` (Cloudflare) — production-grade but heavy; designed for high-traffic infrastructure,
  not single-developer workloads.

---

## 3. Child Process Management (stdio mode)

**Decision**: `tokio::process::Command` with `stdin(Stdio::piped())` and `stdout(Stdio::piped())`.
Two Tokio tasks run concurrently: one forwarding client→server (with policy interception), one
forwarding server→client (pass-through for responses).

**Rationale**: Tokio's process API integrates cleanly with the async executor. The bidirectional
forwarding model ensures the proxy never blocks one direction waiting for the other, which is
critical for request/response interleaving.

**Exit detection**: `child.wait()` in a dedicated task; on exit, send a shutdown signal via
`CancellationToken` so the main loop terminates cleanly with exit code 3.

**Alternatives considered**:

- `std::process::Command` in a blocking thread — would require bridging to async with
  `tokio::task::spawn_blocking`, adding latency and complexity.

---

## 4. Graceful Shutdown

**Decision**: `tokio_util::sync::CancellationToken` propagated to all request handler tasks.
On SIGTERM or SIGINT (`tokio::signal`), cancel the token and `join_all` outstanding handlers
before flushing the audit writer and exiting.

**Rationale**: `CancellationToken` is the idiomatic Tokio pattern for cooperative cancellation.
It avoids the overhead of a broadcast channel when cancellation is one-way (shutdown is
unidirectional — no resumption). The drain-then-flush order guarantees FR-018c: every in-flight
request writes its audit entry before the process exits.

**Signal handling**:

```rust
tokio::select! {
    _ = tokio::signal::ctrl_c() => {},
    _ = sigterm_stream.recv() => {},
}
// then: cancel token, join handlers, flush audit writer, exit(0)
```

---

## 5. Concurrent Audit Writer

**Decision**: `tokio::sync::Mutex<BufWriter<File>>` held only for the duration of hash
computation + serialization + write. Policy evaluation and request forwarding proceed outside
the lock.

**Rationale**: The Mutex is held for microseconds (hash a string, write ~500 bytes). No request
handler blocks on policy evaluation waiting for the audit lock — they only acquire it at the
point of writing. This satisfies FR-028: policy evaluation is fully concurrent; only the chain
write is serialized (to guarantee deterministic `prev_hash` ordering).

**`entry_hash` computation convention** (FR-011):

1. Build the `AuditEntry` struct with `entry_hash: String::new()` (empty string)
2. Serialize to JSON bytes via `serde_json::to_vec`
3. Compute `sha2::Sha256::digest(&bytes)`, format as `"sha256:<hex>"`
4. Set `entry_hash` to that value, re-serialize for the final written line

This convention is documented in `contracts/audit-log.md` and tested in `tests/audit_chain.rs`.

**Alternatives considered**:

- `std::sync::Mutex` — would block the async executor thread during the write. Fine for
  microsecond-scale locks in practice, but `tokio::sync::Mutex` is the correct async primitive
  and avoids subtle starvation if the filesystem is slow.
- Separate audit writer task with a channel — adds a bounded channel, backpressure logic, and
  the complexity of flushing on shutdown. The mutex approach is simpler and correct.

---

## 6. Glob Matching for Resource URIs

**Decision**: `globset` 0.4.x crate, `GlobSet` compiled once at policy load time.

**Rationale**: `globset` is maintained by BurntSushi (ripgrep author), uses a DFA-based
matching engine with no ReDoS risk, and handles the `**` double-star pattern needed for
directory subtree matching (e.g., `file:///workspace/src/**`). Compile-once means zero
per-request regex/glob compilation cost.

**Alternatives considered**:

- `glob` crate — older, synchronous, no DFA backend, documented ReDoS vectors. Rejected.
- Manual prefix matching — eliminates the glob footgun but loses the expressiveness needed for
  real resource URI policies (e.g., `file:///workspace/**.rs` to allow only Rust files).

---

## 7. Cargo Workspace Structure

**Decision**: Single crate with both `[[bin]]` and `[lib]` targets in one `Cargo.toml`. No
workspace at M1 — a workspace adds coordination overhead that isn't justified until M2+
introduces separate crates (e.g., a CLI frontend separate from the library).

**Rationale**: A single-crate bin+lib is the simplest structure that satisfies FR-026/FR-027.
`src/main.rs` is the binary entry point (thin CLI wrapper). `src/lib.rs` is the library entry
point exposing the public API. All modules live under `src/`.

**Workspace deferred to**: M2 or M3, when the framework integration helpers may warrant a
separate crate (e.g., `argos-langchain`).

---

## 8. MCP JSON-RPC Parsing Strategy

**Decision**: Parse only the `method` field first to determine routing; parse the full `params`
only for intercepted methods (`tools/call`, `resources/read`, `resources/list`,
`resources/subscribe`). All other messages are forwarded as raw bytes without full parsing.

**Rationale**: This minimises the parsing surface (fewer attack vectors, less latency for
pass-through messages) and correctly handles future MCP methods the proxy doesn't know about —
they pass through unmodified. The approach is "parse what you need, forward the rest".

**Error handling**: If a `tools/call` or `resources/*` message fails JSON parsing after the
method routing step, return a JSON-RPC parse error (FR-024) rather than crashing.

---

## 9. Policy TOML Validation

**Decision**: `toml` 0.8.x for parsing; `serde` derive for deserialization directly into typed
structs. Version validation is a runtime check against a hardcoded `SUPPORTED_VERSIONS` constant
after deserialization.

**Rationale**: Deserialization errors (missing required fields, wrong types) are reported by
serde with field-level context. Version validation is a post-deserialization semantic check —
serde can't express "valid values of this field" as a constraint without a custom deserializer.
Keeping it as a runtime check keeps the policy loader simple and the error messages clear.

**`version` handling**: In v0.1, `SUPPORTED_VERSIONS = ["0.1"]`. Any other value → hard error
with message: `"Policy version 'X' is not supported by this proxy version. Supported: 0.1"`.
