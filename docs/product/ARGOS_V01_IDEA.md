# Argos — v0.1 Idea Document

**Version:** 0.2 (renamed from Sentinel, updated with future compatibility constraints)\
**Status:** SDD entrypoint — not yet a specification\
**Companion document:** `ARGOS_PRODUCT_VISION.md` (full product vision and Business Model Canvas)\
**Date:** April 2026

---

## How this document fits the broader product

This document defines v0.1 — the MCP security proxy MVP. It is deliberately narrow. The full product (runtime governance, SaaS control plane, compliance reports, threat intel, multi-framework support) is defined in `ARGOS_PRODUCT_VISION.md`.

The relationship between the two documents is explicit:

- `ARGOS_PRODUCT_VISION.md` defines **what Argos becomes** and the business model that funds it
- This document defines **the first shippable piece** — what gets built first, why, and to what standard
- Section 13 of this document (Future Compatibility Constraints) lists the non-negotiable architectural decisions that v0.1 must not foreclose, derived directly from the vision document's roadmap

**Do not make architectural decisions in v0.1 that conflict with Section 13 without explicit acknowledgement of the tradeoff.**

---

## 1. The one-line pitch

Argos is an open-source MCP security proxy written in Rust that lets enterprise security teams enforce capability policies on AI agents and produce tamper-evident audit trails — without sending a single byte of data to a third party.

---

## 2. The problem

Every serious AI agent deployment in 2025–2026 uses the Model Context Protocol (MCP) to connect language models to tools: file systems, databases, APIs, code execution environments, email, calendars. MCP is now the de facto standard — adopted by Claude Desktop, Cursor, Windsurf, and every major agent framework.

There is no security layer for it.

The consequences are already materialising:

- **12+ MCP CVEs in 2025 alone.** CVE-2025-6514 (CVSS 9.6, remote code execution via mcp-remote) had 437,000 downloads before disclosure.
- **Prompt injection via MCP is trivially exploitable.** A poisoned GitHub issue can cause an agent with private-repo access to exfiltrate source code into public pull requests. Demonstrated live at Black Hat 2025.
- **No enterprise has visibility into what their MCP-connected agents are actually doing.** No standard audit log. No capability boundary enforcement. No tamper-evident record for regulators.

The structural risk is what Simon Willison called the "lethal trifecta": any agent that combines access to private data, exposure to untrusted content, and an external communication channel is exploitable by design. Every useful enterprise agent has all three. Filtering cannot fix this — the model cannot reliably separate instructions from data. The only defensible mitigation is architectural: enforce strict capability boundaries at the transport layer, before tool calls execute.

Commercial solutions (Lakera Guard, Prompt Security, CalypsoAI/F5) all require enterprise prompts and tool outputs to be sent to a third-party cloud API. This is the Samsung data-leak scenario by design. Enterprise security policies routinely prohibit it. Procurement cycles stretch to quarters.

No open-source equivalent exists.

---

## 3. The solution

Argos is a transparent proxy that sits between any MCP client and any MCP server. It:

1. **Intercepts every tool call** before it reaches the MCP server
2. **Evaluates it against a TOML policy spec** defining exactly what this agent is permitted to do
3. **Allows, blocks, or redacts** based on the policy decision
4. **Writes every decision to a tamper-evident audit log** (append-only, Merkle-chained JSON)
5. **Passes allowed calls through** to the upstream MCP server with zero observable latency impact for the agent

It operates at the MCP transport layer — stdio for local clients (Claude Desktop, Cursor), HTTP/SSE for remote server connections. It requires no changes to the MCP client or server. It is invisible to the agent unless a call is blocked.

---

## 4. Target customer

See `ARGOS_PRODUCT_VISION.md` Section 3.1 for the full customer segment analysis.

**Primary persona for v0.1: AppSec lead / platform security engineer**

The person who integrates Argos. Their job is to make AI agent deployments safe enough to get the CISO's sign-off without becoming a deployment bottleneck.

- Cares about: does it work, how does it integrate, can I trust the code, will it break things
- Emotional job: reduce the anxiety of not knowing what an agent just did with prod credentials
- Blocker they face: every new agent deployment is a one-off security review with no standard contract

**v0.1 does not attempt to serve the CISO directly.** The CISO (Segment 2) is served by the SaaS control plane in v1.1+. However, v0.1's audit log and policy spec format must be designed to produce the evidence the CISO will eventually need — this is captured in Section 13.

---

## 5. V0.1 scope — what is being built

V0.1 is a single, self-contained Rust binary: `argos-proxy`.

### What it does

- Acts as a transparent MCP proxy for both transport modes:
  - **stdio mode**: wraps a local MCP server process, intercepting stdin/stdout
  - **HTTP/SSE mode**: acts as a reverse proxy in front of a remote MCP server URL
- Loads a **TOML policy file** at startup defining the capability policy for this session
- For every incoming tool call (`tools/call` JSON-RPC request):
  - Evaluates the call against the loaded policy
  - **Allows** matching calls (passes through to upstream server)
  - **Blocks** denied calls (returns a structured MCP-compliant error to the client, logs the event)
  - **Redacts** calls matching a redaction rule (strips specified argument fields before passing through)
- Writes every evaluation decision — allowed, blocked, or redacted — to an **append-only audit log**
- Default posture: **deny by default** — any tool call not explicitly permitted by policy is blocked
- Exposes a `--dry-run` flag: log and warn on violations without blocking (for initial policy development)

### What it explicitly does NOT do in v0.1

- No prompt content inspection (not a guardrail — a capability enforcer)
- No web UI or dashboard
- No cloud connectivity of any kind
- No policy hot-reload (restart required to change policy)
- No multi-tenant or multi-agent session management
- No injection detection (defence-in-depth comes in v0.5)
- No framework-specific integrations (LangChain, AutoGen — v0.3)
- No SaaS features of any kind

---

## 6. Core data structures (pre-spec, for SDD input)

These are the logical concepts. SDD will produce the formal type definitions. Field names marked `[reserved]` are required for future compatibility (see Section 13) but may be null or empty in v0.1.

### Policy file (TOML)

```toml
# argos.toml — example

[meta]
version = "0.1"                          # required — DSL version for forward compatibility
agent = "code-review-agent"              # human-readable agent identifier
description = "Read-only monorepo access. No writes, no shell, no network."
session_tags = ["code-review", "ci"]    # [reserved] — used for compliance mapping in v1.1+

[[rules]]
tool = "read_file"
action = "allow"
constraints = { path_prefix = "/workspace/monorepo" }
tags = []                                # [reserved] — compliance template mapping

[[rules]]
tool = "list_directory"
action = "allow"
constraints = { path_prefix = "/workspace/monorepo" }
tags = []

[[rules]]
tool = "write_file"
action = "block"
reason = "Write access not permitted for this agent"
tags = []

[[rules]]
tool = "run_terminal_cmd"
action = "block"
reason = "Shell execution not permitted for this agent"
tags = []

[[rules]]
tool = "*"
action = "block"
reason = "Default deny — tool not in policy"
tags = []
```

### Audit log entry (JSONL — one JSON object per line)

```json
{
  "timestamp": "2026-04-22T10:31:05.123456Z",
  "sequence": 42,
  "prev_hash": "sha256:abc123def456...",
  "entry_hash": "sha256:789abcdef012...",
  "session_id": "01950c2a-7e3f-7000-8000-000000000042",
  "decision": "blocked",
  "tool": "run_terminal_cmd",
  "arguments": { "command": "cat /etc/passwd" },
  "policy_rule_matched": "run_terminal_cmd:block",
  "reason": "Shell execution not permitted for this agent",
  "agent": "code-review-agent",
  "policy_version": "0.1",
  "org_id": null,
  "tenant_id": null
}
```

Fields marked `null` in v0.1: `org_id`, `tenant_id` — reserved for SaaS multi-tenancy in v1.1+.

### Proxy config (CLI)

```
argos-proxy \
  --policy ./argos.toml \
  --log ./audit.jsonl \
  --mode stdio \
  --upstream "npx @modelcontextprotocol/server-filesystem /workspace" \
  --dry-run
```

```
argos-proxy \
  --policy ./argos.toml \
  --log ./audit.jsonl \
  --mode http \
  --upstream "https://mcp.internal.example.com" \
  --tls-cert ./cert.pem \     # accepted but not required in v0.1
  --tls-key ./key.pem
```

---

## 7. Architecture overview

```
MCP Client (Claude Desktop / Cursor / any agent)
        |
        | stdio or HTTP/SSE
        v
  ┌─────────────────────────────────┐
  │         argos-proxy             │
  │                                 │
  │  ┌───────────────────────────┐  │
  │  │   Transport adapter       │  │  ← stdio or HTTP/SSE
  │  └────────────┬──────────────┘  │
  │               │ parsed request  │
  │  ┌────────────▼──────────────┐  │
  │  │   Request parser          │  │  ← MCP JSON-RPC parsing
  │  └────────────┬──────────────┘  │
  │               │ tool call       │
  │  ┌────────────▼──────────────┐  │
  │  │   Policy engine           │◄─┼── argos.toml
  │  └────────────┬──────────────┘  │
  │               │ decision        │
  │  ┌────────────▼──────────────┐  │
  │  │   Audit writer            │──┼──► audit.jsonl (Merkle-chained)
  │  └────────────┬──────────────┘  │
  │               │                 │
  │  ┌────────────▼──────────────┐  │
  │  │   Passthrough / Block     │  │  ← allow → forward; block → MCP error
  │  └────────────┬──────────────┘  │
  └───────────────┼─────────────────┘
                  |
                  | stdio or HTTP/SSE
                  v
  MCP Server (filesystem / GitHub / database / any)
```

---

## 8. Technology decisions

| Decision          | Choice                       | Rationale                                                                              |
| ----------------- | ---------------------------- | -------------------------------------------------------------------------------------- |
| Language          | Rust                         | Memory safety, single binary, sub-ms overhead, no CVE surface from runtime             |
| Async runtime     | Tokio                        | De facto standard for Rust async I/O; broad ecosystem support                          |
| Policy format     | TOML                         | Rust-native (`toml` crate), human-readable, survives DSL evolution via `version` field |
| Audit log format  | JSONL + SHA-256 Merkle chain | Auditor-legible, tamper-evident from day one, OTel-compatible, Sigstore/Rekor-ready    |
| Hash function     | SHA-256                      | Standard, well-audited; required for Sigstore/Rekor compatibility in v0.4              |
| Transport support | stdio + HTTP/SSE             | Covers all current MCP clients and server deployment patterns                          |
| Default posture   | Deny by default              | Non-negotiable for a security product; defines the brand                               |
| Config delivery   | File + CLI flags             | No network required; works air-gapped; no attack surface                               |
| Session ID        | UUID v4                      | Globally unique; required for distributed deployments in v1.0+                         |
| Binary vs library | Both (bin + lib crate)       | Binary for direct use; library crate for SaaS control plane integration in v1.1+       |

---

## 9. Success criteria for v0.1

Argos v0.1 is done when all of the following are true:

1. `argos-proxy` in stdio mode successfully proxies a local MCP server with policy enforcement active
2. `argos-proxy` in HTTP/SSE mode successfully proxies a remote MCP server URL
3. A tool call matching an `allow` rule passes through to the upstream server unmodified
4. A tool call matching a `block` rule is rejected with a valid MCP-compliant JSON-RPC error response; the client receives it cleanly
5. A tool call matching a `redact` rule has specified argument fields stripped before forwarding
6. Every evaluation decision is written to the audit log as a valid JSONL entry
7. Each audit log entry contains a valid `prev_hash` linking it to the previous entry (SHA-256 of the previous entry's raw bytes)
8. The first entry in a new log file uses `prev_hash: "sha256:0000...0000"` (64 zeros) as the genesis marker
9. `--dry-run` mode logs violations without blocking; upstream receives the call unmodified
10. The proxy adds less than 5ms overhead to any passing tool call on a standard developer machine
11. A `version` field is present and validated in all loaded policy files; unrecognised versions produce a clear error
12. Session IDs are UUID v4, generated once per proxy invocation
13. The `argos` crate is usable as a library (policy engine and audit writer are exposed as public APIs)
14. A README exists with: installation steps, a working Claude Desktop integration example, and a minimal policy file
15. All core logic (policy evaluation, Merkle chain, request parsing) has unit tests
16. The full stdio and HTTP proxy flows have integration tests

---

## 10. What this is NOT (scope discipline)

- Not a prompt injection detector
- Not a jailbreak prevention tool
- Not a content moderation layer
- Not a secrets scanner (blocking tools that access secret paths IS a valid policy rule — the scanner is not built in)
- Not a replacement for network-level controls
- Not a multi-agent orchestrator
- Not a commercial product in v0.1 — this is the FOSS foundation

---

## 11. Positioning statement

> Argos is a Rust-based MCP security proxy for teams deploying AI agents in enterprise environments. It enforces capability policies at the transport layer — controlling exactly which tools an agent can call and under what conditions — and writes a tamper-evident audit trail of every decision. It runs entirely on your infrastructure. No data leaves your perimeter.

---

## 12. Open questions for SDD to resolve

These are not answered here. They are the inputs to the formal specification process. SDD must resolve them before implementation begins.

1. **Policy evaluation order**: when multiple rules match a tool call (e.g. a specific rule and a wildcard `*` rule both match), which takes precedence? Options: first match, most specific match, last match, explicit priority field.

2. **Constraint expression semantics**: v0.1 uses simple key-value constraints (`path_prefix = "/workspace"`). What operators are required? Candidates: `prefix`, `suffix`, `contains`, `exact`, `regex`, `not`, `one_of`. What is the minimum viable set for v0.1?

3. **Wildcard tool matching**: should `tool = "read_*"` be valid in v0.1 or deferred to v0.2? What glob syntax is supported?

4. **Redaction semantics**: when a `redact` rule fires, which argument fields are stripped? Is the call still forwarded after redaction, or can redaction also block? Should both behaviours be expressible?

5. **MCP error response format**: what is the exact JSON-RPC error object returned to the client when a call is blocked? What error code, what message format? Must not break MCP-compliant clients.

6. **Audit log Merkle chain bootstrap**: the first entry has no predecessor. The chosen convention (64 zero hex chars for SHA-256) must be documented and tested. Is there a better convention?

7. **Argument logging scope**: should all tool call arguments be logged verbatim, or should there be a configurable maximum size or field exclusion list? Logging raw arguments could itself be a data exposure risk in some deployments.

8. **Policy validation at startup**: should argos-proxy refuse to start if the policy file contains unrecognised fields or constraint keys? Strict vs permissive parsing — what is the right default?

---

## 13. Future compatibility constraints

These constraints are derived from `ARGOS_PRODUCT_VISION.md` Sections 5 and 6. Each one prevents an architectural decision in v0.1 from foreclosing a capability required by the full product.

**These are non-negotiable.** If any constraint must be violated, the violation must be explicitly documented with the tradeoff acknowledged.

| #  | Constraint                                                                                       | Why it matters                                                                                         | Violated by                                                                                    |
| -- | ------------------------------------------------------------------------------------------------ | ------------------------------------------------------------------------------------------------------ | ---------------------------------------------------------------------------------------------- |
| 1  | Audit log entries must include `org_id` and `tenant_id` fields (nullable in v0.1)                | SaaS multi-tenancy in v1.1 requires these fields for tenant isolation                                  | Omitting them from the schema entirely                                                         |
| 2  | Policy file must have a `version` field, validated at load time                                  | DSL evolution in v0.2+ must be non-breaking; old policies must fail loudly on new parsers              | Accepting policy files without version                                                         |
| 3  | Session ID must be UUID v4, generated once per proxy invocation                                  | Global uniqueness required for distributed deployments in v1.0                                         | Using sequential integers or hostname-based IDs                                                |
| 4  | Audit log hash function must be SHA-256                                                          | Sigstore/Rekor anchoring in v0.4 requires SHA-256                                                      | Using any other hash function                                                                  |
| 5  | OTel span emission must be architecturally possible (even if not implemented)                    | v0.4 adds OpenTelemetry GenAI spans without breaking existing deployments                              | Designing the request pipeline in a way that makes span injection impossible without a rewrite |
| 6  | `argos` must be buildable as both a binary and a library crate                                   | SaaS control plane in v1.1 embeds the policy engine and audit writer as a library                      | Making the policy engine only callable via CLI                                                 |
| 7  | MCP error responses must be spec-compliant JSON-RPC                                              | Forward compatibility as MCP protocol evolves; client compatibility                                    | Returning non-standard error formats                                                           |
| 8  | HTTP/SSE mode must accept TLS certificate configuration (even if not enforced)                   | Enterprise deployment requires mTLS in v1.0; retrofitting TLS config into the CLI is a breaking change | Hardcoding HTTP-only in the HTTP mode                                                          |
| 9  | Audit log JSONL format must support a `rotation_marker` entry type (even if not emitted in v0.1) | v1.0 log rotation requires a clean marker entry type in the format spec                                | Defining the JSONL format as only supporting `decision` entry types                            |
| 10 | Policy rules must carry a `tags` field (empty array `[]` acceptable in v0.1)                     | v1.1 compliance report templates map policy rules to specific regulatory controls via tags             | Omitting the `tags` field from the rule schema entirely                                        |
