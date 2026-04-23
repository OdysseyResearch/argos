# Argos

**Open-source MCP security proxy. Enforce capability policies on AI agents. Produce
tamper-evident audit trails. Zero data egress.**

```text
MCP Client → argos-proxy → MCP Server
                ↓
          audit.jsonl (Merkle-chained)
```

---

## The problem

Every serious AI agent deployment uses MCP to connect language models to tools — filesystems,
databases, APIs, code execution. There is no security layer for it.

The structural risk is what Simon Willison called the "lethal trifecta": any agent that combines
access to private data, exposure to untrusted content, and an external communication channel is
exploitable by design. Every useful enterprise agent has all three.

Commercial solutions (Lakera, Prompt Security, CalypsoAI) require your prompts and tool outputs
to leave your infrastructure. Enterprise security policies prohibit this. No open-source
alternative exists.

## What Argos does

Argos is a transparent proxy that sits between any MCP client and any MCP server. For every
incoming tool call it:

1. Evaluates the call against a TOML policy spec
2. **Allows**, **blocks**, or **redacts** based on the policy decision
3. Writes the decision to an append-only, Merkle-chained audit log

Default posture: **deny by default**. Any tool call not explicitly permitted is blocked.

It requires no changes to the MCP client or server. It is invisible to the agent unless a call
is blocked.

## What it is not

Argos is a capability enforcer, not a probabilistic guardrail. It does not detect prompt
injection, moderate content, or run AI models. It enforces strict boundaries at the transport
layer — a control that is architectural, not statistical.

It is also not an agent platform, orchestrator, or tool runtime. It secures whatever agent
platform you already use.

## Policy example

```toml
[meta]
version = "0.1"
agent = "code-review-agent"
description = "Read-only monorepo access. No writes, no shell, no network."

[[rules]]
tool = "read_file"
action = "allow"
constraints = { path_prefix = "/workspace/monorepo" }

[[rules]]
tool = "write_file"
action = "block"
reason = "Write access not permitted for this agent"

[[rules]]
tool = "*"
action = "block"
reason = "Default deny — tool not in policy"
```

## Status

> **Pre-release.** Argos is under active development. The v0.1 MVP is being built now.
> Star the repo to follow progress.

| Version                       | Status            |
| ----------------------------- | ----------------- |
| v0.1 — MCP proxy MVP          | 🚧 In development |
| v0.2 — Policy DSL v1          | Planned           |
| v0.3 — Framework integrations | Planned           |
| v1.0 — Production-ready       | Planned           |
| Argos Cloud (SaaS)            | Future            |

## Why Rust

Single binary, no runtime dependencies, sub-millisecond overhead, memory safety without a GC.
A security tool that adds attack surface or runtime complexity is a liability. Argos adds neither.

## License

Apache 2.0 — permanently free and open source. See [LICENSE](LICENSE).

---
