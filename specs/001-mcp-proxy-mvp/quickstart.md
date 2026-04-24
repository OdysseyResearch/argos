# Quickstart: Argos v0.1 MCP Security Proxy

**Audience**: Developer or AppSec engineer setting up `argos-proxy` for the first time.

---

## Install

```bash
cargo install argos-proxy
# or build from source:
git clone https://github.com/ogil109/argos
cd argos && cargo build --release
# binary at: target/release/argos-proxy
```

---

## Write a policy

Create `policy.toml`:

```toml
[meta]
version = "0.1"
description = "Read-only workspace access."

[[rules]]
tool = "read_file"
action = "allow"
constraints = { path_prefix = "/workspace/myproject" }
tags = []

[[rules]]
resource = "file:///workspace/myproject/**"
action = "allow"
tags = []

[[rules]]
tool = "*"
action = "block"
reason = "Default deny — not in policy"
tags = []
```

---

## Run with Claude Code (stdio mode)

In your Claude Code MCP config (`~/.claude/claude_code_config.json` or equivalent), replace
the server command with `argos-proxy`:

```json
{
  "mcpServers": {
    "filesystem": {
      "command": "argos-proxy",
      "args": [
        "--policy", "/path/to/policy.toml",
        "--audit-log", "/path/to/audit.jsonl",
        "--agent", "claude-code",
        "--",
        "uvx", "mcp-server-filesystem", "/workspace/myproject"
      ]
    }
  }
}
```

The proxy starts automatically when Claude Code launches the MCP server. No daemon, no ports.

---

## Run with Roo Code / Cursor / Windsurf

Same pattern — replace the MCP server command in the client's MCP configuration file with the
`argos-proxy` invocation above. Each client has its own config path:

| Client               | Config path                    |
| -------------------- | ------------------------------ |
| Roo Code             | VS Code settings → MCP Servers |
| Cursor               | `.cursor/mcp.json`             |
| Windsurf             | `.windsurf/mcp.json`           |
| GitHub Copilot agent | VS Code settings → MCP Servers |
| Continue.dev         | `.continue/config.json`        |

---

## Run as HTTP reverse proxy

For a remote MCP server accessible over HTTP/SSE:

```bash
argos-proxy \
  --policy policy.toml \
  --audit-log audit.jsonl \
  --agent my-agent \
  --upstream "https://mcp.internal.example.com" \
  --bind 127.0.0.1 \
  --port 8080
```

Point your MCP client at `http://127.0.0.1:8080`.

---

## Try dry-run mode first

If you are not sure your policy is correct yet, run with `--dry-run`:

```bash
argos-proxy \
  --policy policy.toml \
  --audit-log audit.jsonl \
  --agent claude-code \
  --dry-run \
  -- uvx mcp-server-filesystem /workspace
```

Violations are logged and warned on stderr, but all calls pass through to the upstream server.
Review `audit.jsonl` to see what would have been blocked.

---

## Verify the audit log

After a session:

```bash
argos-proxy verify --audit-log audit.jsonl
```

Output:

```
Chain intact: 47 entries verified.
```

If any entry has been modified, deleted, or inserted, the verification fails with the exact
entry index where the chain breaks.

---

## Read the audit log

The log is plain JSONL — one JSON object per line. Each line is a policy decision:

```bash
cat audit.jsonl | jq '.'
```

Example entry:

```json
{
  "timestamp": "2026-04-24T10:31:05.123456Z",
  "sequence": 3,
  "decision": "blocked",
  "message_type": "tools/call",
  "tool_or_resource": "write_file",
  "reason": "Default deny — not in policy",
  "agent": "claude-code"
}
```

---

## Exit codes

| Code | Meaning                                 |
| ---- | --------------------------------------- |
| 0    | Clean shutdown                          |
| 1    | Startup or policy error                 |
| 2    | Audit write failure (disk full)         |
| 3    | Upstream subprocess exited unexpectedly |
