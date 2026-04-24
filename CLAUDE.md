<!-- SPECKIT START -->

For additional context about technologies to be used, project structure,
shell commands, and other important information, read the current plan:

**Active feature plan**: `specs/001-mcp-proxy-mvp/plan.md`

Supporting artifacts:

- `specs/001-mcp-proxy-mvp/research.md` — technology decisions and rationale
- `specs/001-mcp-proxy-mvp/data-model.md` — Rust types for all domain entities
- `specs/001-mcp-proxy-mvp/quickstart.md` — getting started guide
- `specs/001-mcp-proxy-mvp/contracts/cli.md` — CLI flags, exit codes, stderr contract
- `specs/001-mcp-proxy-mvp/contracts/policy-format.md` — TOML policy schema
- `specs/001-mcp-proxy-mvp/contracts/audit-log.md` — JSONL audit log schema and hash convention
- `specs/001-mcp-proxy-mvp/contracts/library-api.md` — public crate API contract

<!-- SPECKIT END -->

<!--
ROLE OF THIS FILE
=================
This is the agent bootstrap file — read at the start of every session before any work begins.

It contains only what is true project-wide, before any spec exists, and across all features:
  - Pointers to governing documents (constitution, vision, roadmap)
  - Hard project-wide constraints not captured elsewhere
  - Non-obvious toolchain conventions that apply to every session

What does NOT belong here:
  - Build commands, crate structure, test patterns, key dependencies — these are
    spec-specific and documented during /speckit-plan as research/data model artifacts
  - Feature-specific context — lives in specs/<feature>/
  - Anything already in the constitution

When to update this file:
  - A new project-wide governing document is added
  - A toolchain convention is established that applies to all future specs
  - The speckit section is updated by the specify CLI

Do not pad this file. If you are unsure whether something belongs here, it probably belongs
in a spec plan instead.
-->

## Project Context

Read before starting any work:

- **Constitution**: `.specify/memory/constitution.md` — governing principles, architecture
  constraints, and development workflow. Supersedes all other guidance when conflicts arise.
- **Product vision**: `docs/product/ARGOS_PRODUCT_VISION.md` — full product and business model.
- **Roadmap**: `docs/ROADMAP.md` — milestone sequencing and current status.
- **v0.1 scope**: `docs/product/ARGOS_V01_IDEA.md` — MVP definition, success criteria, and the
  10 future-compatibility constraints that bind all M1 implementation decisions.
