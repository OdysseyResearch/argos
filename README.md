# Argos

[![License](https://img.shields.io/badge/license-AGPL--3.0-blue.svg)](LICENSE)
[![Status](https://img.shields.io/badge/status-pre--release-orange.svg)](https://github.com/ogil109/argos/releases)
[![Built with Rust](https://img.shields.io/badge/built%20with-Rust-orange.svg)](https://www.rust-lang.org/)

**Open-source MCP security proxy. Enforce capability policies on AI agents. Produce
tamper-evident audit trails. Zero data egress.**

```text
MCP Client → argos-proxy → MCP Server
                ↓
          audit.jsonl (Merkle-chained)
```

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
1. **Allows**, **blocks**, or **redacts** based on the policy decision
1. Writes the decision to an append-only, Merkle-chained audit log

Default posture: **deny by default**. Any tool call not explicitly permitted is blocked.

It requires no changes to the MCP client or server. It is invisible to the agent unless a call
is blocked.

## What it is not

Argos is a capability enforcer, not a probabilistic guardrail. It does not detect prompt
injection, moderate content, or run AI models. It enforces strict boundaries at the transport
layer — a control that is architectural, not statistical.

It is also not an agent platform, orchestrator, or tool runtime. It secures whatever agent
platform you already use.

## Prerequisites

**To use Argos:**

- [Rust](https://rustup.rs/) stable 1.91.1 or later

**To contribute:**

- [uv](https://docs.astral.sh/uv/getting-started/installation/) — Python package manager
- [just](https://github.com/casey/just) — task runner (`cargo install just --version 1.50.0`)

## Installation

> **Pre-release.** No binary releases yet. Build from source:

```bash
cargo build --release
# Binary at: target/release/argos-proxy
```

Releases will be available at [github.com/ogil109/argos/releases](https://github.com/ogil109/argos/releases).

## Usage

> **Pre-release.** Full usage documentation ships with M1.

See [**ROADMAP.md**](docs/ROADMAP.md) for the milestone plan and what is being built now.

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

## Why Rust

Single binary, no runtime dependencies, sub-millisecond overhead, memory safety without a GC.
A security tool that adds attack surface or runtime complexity is a liability. Argos adds neither.

## Contributing

Argos is in early development. Contributions, feedback, and security research are welcome.

**Development setup:**

```bash
cargo install just --version 1.50.0
just setup   # installs all dev dependencies and git hooks
just --list  # see available recipes
```

**How to contribute:**

- **Bug reports and feature requests**: [open an issue](https://github.com/ogil109/argos/issues)
- **Security vulnerabilities**: please do not open a public issue — see [SECURITY.md](SECURITY.md)
- **Pull requests**: please open an issue first to discuss the change

## Support

Open an issue on [GitHub](https://github.com/ogil109/argos/issues) for questions, bugs, or
discussion. There is no mailing list or chat yet — that comes with community growth.

## License

AGPL-3.0 — permanently free and open source. See [LICENSE](LICENSE).

If you need to embed or deploy Argos without the AGPL obligations (e.g. in a proprietary
product), a commercial license is available — open an issue to discuss.
