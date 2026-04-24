# Contract: CLI Interface

**Version**: 0.1 | **Binary**: `argos-proxy`

---

## Synopsis

```
argos-proxy --policy <path> --audit-log <path> [OPTIONS] -- <server-cmd> [args...]
argos-proxy --policy <path> --audit-log <path> [OPTIONS] --upstream <url>
```

---

## Required Flags

| Flag                 | Type | Description                                                                                   |
| -------------------- | ---- | --------------------------------------------------------------------------------------------- |
| `--policy <path>`    | Path | TOML policy file. Must exist and be readable at startup.                                      |
| `--audit-log <path>` | Path | JSONL audit log file. Must be writable at startup (created if absent). Opened in append mode. |

Omitting either flag → exit 1 with a human-readable error.

---

## Optional Flags

| Flag                  | Type   | Default     | Description                                                                                                                                      |
| --------------------- | ------ | ----------- | ------------------------------------------------------------------------------------------------------------------------------------------------ |
| `--agent <name>`      | String | `"unknown"` | Agent label written to every audit entry's `agent` field.                                                                                        |
| `--max-arg-bytes <N>` | usize  | `65536`     | Maximum bytes of tool call arguments logged verbatim. Arguments exceeding this are truncated; truncation is recorded in the audit entry.         |
| `--dry-run`           | bool   | false       | Log and warn on policy violations but do not block calls. Every blocked call passes through to upstream with `dry_run: true` in the audit entry. |
| `--verbose`           | bool   | false       | Enable per-request trace logging to stderr. Off by default.                                                                                      |

---

## Transport Mode Flags

Exactly one transport must be specified. Providing both or neither is a startup error (exit 1).

### stdio mode

Activated by the presence of `--` followed by the upstream server command.

```
argos-proxy --policy policy.toml --audit-log audit.jsonl -- uvx mcp-server-filesystem /workspace
```

| Flag                 | Description                                                                                |
| -------------------- | ------------------------------------------------------------------------------------------ |
| `-- <cmd> [args...]` | Server command and arguments. Proxy spawns this as a subprocess and forwards stdin/stdout. |

### HTTP/SSE mode

Activated by the presence of `--upstream`.

```
argos-proxy --policy policy.toml --audit-log audit.jsonl --upstream https://mcp.example.com
```

| Flag                | Type   | Default       | Description                                                          |
| ------------------- | ------ | ------------- | -------------------------------------------------------------------- |
| `--upstream <url>`  | URL    | —             | Upstream MCP server URL.                                             |
| `--bind <addr>`     | String | `"127.0.0.1"` | Address to bind the HTTP listener.                                   |
| `--port <N>`        | u16    | `8080`        | Port to bind the HTTP listener.                                      |
| `--tls-cert <path>` | Path   | —             | TLS certificate file (PEM). Must be readable at startup if provided. |
| `--tls-key <path>`  | Path   | —             | TLS private key file (PEM). Must be readable at startup if provided. |

`--tls-cert` and `--tls-key` must be provided together or not at all.

---

## Subcommands

### `argos-proxy verify`

```
argos-proxy verify --audit-log <path>
```

Reads the JSONL audit log at `<path>`, recomputes the SHA-256 hash chain entry-by-entry, and
reports whether the chain is intact.

Output on success:

```
Chain intact: 47 entries verified.
```

Output on failure:

```
Chain broken at entry 23: expected prev_hash sha256:abc123..., got sha256:def456...
```

Exit codes: 0 = chain intact; 1 = chain broken or file unreadable.

---

## Startup Stderr Output

The following messages always appear on stderr at startup regardless of flags:

```
argos-proxy v0.1.0 | policy: policy.toml | agent: claude-code | mode: stdio
```

When `--dry-run` is active:

```
WARNING: DRY RUN ACTIVE — policy violations are logged but not enforced
```

---

## Exit Codes

| Code | Meaning                                                                                      |
| ---- | -------------------------------------------------------------------------------------------- |
| 0    | Clean shutdown (SIGTERM/SIGINT received, all in-flight requests drained, audit log flushed)  |
| 1    | Startup or policy error (missing flag, unreadable file, invalid policy, unsupported version) |
| 2    | Runtime audit write failure (disk full, filesystem error mid-session)                        |
| 3    | Upstream subprocess failure (stdio mode: child process exited unexpectedly)                  |

---

## Stderr Contract (stdio mode)

In stdio mode, `stdout` is exclusively MCP protocol bytes. Zero non-protocol bytes are permitted
on stdout — any deviation breaks the MCP client connection.

All operator-facing output goes to `stderr`:

- Startup confirmation
- Dry-run warning
- Fatal errors
- Per-request trace logs (when `--verbose`)
- Subprocess exit notification

This follows LSP/MCP convention: Claude Code, VS Code, Roo Code all capture stderr into their
log panels.
