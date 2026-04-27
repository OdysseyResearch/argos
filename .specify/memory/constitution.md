<!--
SYNC IMPACT REPORT
==================
Version change: 1.2.0 → 1.2.1 (PATCH — merge strategy rule added to Development Workflow)

Sections modified:
  - Development Workflow — added rule requiring true merge commits for PRs; squash and
    rebase-only merges are prohibited to preserve non-linear commit history

Templates updated:
  ✅ .specify/templates/plan-template.md — no changes required
  ✅ CLAUDE.md — no changes required
  ✅ .specify/templates/spec-template.md — no changes required
  ✅ .specify/templates/tasks-template.md — no changes required

Deferred items: none

---

Previous amendment (1.1.0 → 1.2.0): license changed from Apache 2.0 to AGPL-3.0,
dual commercial license added to Principle III rationale; Principle VII added.
-->

# Argos Constitution

## Core Principles

### I. Deny by Default

The proxy's default posture is block-everything. Any tool call not explicitly permitted by a loaded
policy MUST be blocked and logged. There is no opt-in to a permissive mode — `--dry-run` is for
policy development only and must log every violation loudly. This principle defines the Argos brand:
if a configuration cannot be made safe, the system fails safe.

**Rationale**: a security product that defaults to permissive trains users to ignore it and
provides no protection against misconfiguration.

### II. Zero Data Egress

No byte of customer data — prompts, tool arguments, tool outputs, audit log entries — may be
transmitted outside the customer's infrastructure by the FOSS core. Any feature requiring external
connectivity (Sigstore anchoring, OTel export, SaaS reporting) MUST be opt-in, clearly documented,
and architecturally isolated so it can be compiled out or disabled without affecting core behaviour.

**Rationale**: enterprise adoption is blocked by data-egress policies. This constraint is the
primary differentiator from every commercial alternative and must never be compromised.

### III. FOSS Core Integrity

The security proxy (`argos-proxy`), policy engine, audit log writer, and MCP transport adapters
are permanently free and open source under AGPL-3.0. No capability from this list may be moved
behind a paywall, a license-key feature flag, or a cloud dependency. The SaaS layer (Argos Cloud)
MUST add only operational services — managed hosting, compliance report generation, threat intel
feeds — never capabilities that belong in the runtime.

A commercial license is offered in parallel for organisations that cannot accept AGPL-3.0
obligations (e.g. embedding Argos in a proprietary product). The commercial license is a revenue
stream, not a capability restriction — it grants the same runtime rights as AGPL-3.0 without the
copyleft obligations. The FOSS core MUST remain fully functional under AGPL-3.0 without the
commercial license.

**Rationale**: AGPL-3.0's network copyleft clause prevents a well-funded competitor from forking
`argos-proxy`, adding proprietary features, and running a competing SaaS without contributing
back. Apache 2.0 would permit this. The commercial license dual tracks enterprise adoption for
organisations with AGPL procurement restrictions. HashiCorp's BSL migration is the canonical
anti-pattern to avoid — BSL removes open-source status entirely; dual licensing under AGPL-3.0
does not.

### IV. Future Compatibility

No implementation decision in M1 may foreclose a capability defined in `docs/ROADMAP.md` without
explicit acknowledgement of the tradeoff. The 10 constraints in `docs/product/ARGOS_V01_IDEA.md`
§13 are binding architectural requirements. Every spec and plan MUST include a compatibility
constraint validation pass before implementation begins.

**Rationale**: architectural debt incurred in M1 compounds. The 10 constraints are the minimum
set that keeps all roadmap doors open without rewrites.

### V. Architectural Honesty

Argos is a capability enforcer, not a probabilistic guardrail, AI safety system, or injection
detector. Technical claims MUST be accurate and defensible. "Enforces capability policies at the
transport layer" is true. "Prevents prompt injection" is not — and must never appear in
documentation or marketing. Defence-in-depth features (M5+) MUST be positioned as additional
layers, never as the primary value proposition.

**Rationale**: false security claims are a trust liability. The target customer (AppSec engineer)
will verify claims. Overstating capabilities destroys credibility with the exact audience Argos
needs to win.

Argos MUST NOT expand into agent platforms, orchestration, tool provision, or any capability that
belongs inside the agent runtime. Argos enforces the security boundary around agents — it does not
participate in what agents do. This focus is the source of ecosystem neutrality: Argos can secure
any agent platform precisely because it competes with none of them. See
`docs/product/ARGOS_PRODUCT_VISION.md` §7 for the full rationale.

### VI. Test-Proven Correctness

For a security product, correctness is existential. All policy evaluation logic, Merkle chain
integrity, and proxy transport flow paths MUST have automated tests before any M1 success
criterion is claimed as met. Required minimum coverage:

- Unit tests: policy engine (allow/block/redact decisions, rule ordering, wildcard matching)
- Unit tests: Merkle chain writer (hash chaining, genesis entry, chain verification)
- Integration tests: stdio proxy flow end-to-end (allow path, block path, redact path)
- Integration tests: HTTP/SSE proxy flow end-to-end (allow path, block path)

No security-critical path ships without tests.

### VII. Three-Phase Strategy

Product decisions MUST be evaluated against the three-phase sequencing defined in
`docs/ROADMAP.md`: **Standard** (FOSS proxy, M1–M6) → **Moat** (Argos OS, OS1–OS2) →
**Revenue** (SaaS, M7–M9). The OS phase precedes the SaaS phase — this is non-negotiable.

Specifically:
- Features that belong in M7–M9 (SaaS) MUST NOT be built before OS1 is underway.
- Nothing in M1–M6 may be designed in a way that forecloses OS-layer integration.
- The funding bridge between M6 and OS1 is grants (NLnet, Sovereign Tech Fund, EU Horizon AI),
  not SaaS revenue. Decisions that require SaaS revenue to reach the OS phase are architectural
  mistakes.

**Rationale**: a solo founder cannot outspend incumbents on SaaS features. The OS +
data/instruction separation is the only defensible long-term moat — a multi-year research bet
that large vendors will not take. Deep focus compounds on the hard problem. SaaS built on top of
the OS launches differentiated; SaaS built before the OS races incumbents on compliance
dashboards and loses.

## Architecture Constraints

- The primary deliverable is a single Rust binary (`argos-proxy`) with no runtime dependencies.
- The `argos` crate MUST be buildable as both `[[bin]]` and `[lib]` in a single Cargo workspace.
  The policy engine and audit writer MUST be exposed as public library APIs.
- Async runtime: Tokio. No alternatives evaluated for M1.
- Policy format: TOML with mandatory `version` field validated at load time. Unrecognised versions
  produce a hard error, not a warning.
- Audit log: JSONL, SHA-256 Merkle-chained. The hash function MUST NOT be changed — Sigstore/Rekor
  compatibility (M4) depends on SHA-256 specifically.
- Session IDs: UUID v4, generated once per proxy invocation.
- HTTP/SSE mode: MUST accept TLS certificate configuration at the CLI level even if mTLS is not
  enforced in M1. Retrofitting TLS config later is a breaking change.
- Audit log schema: MUST include `org_id` and `tenant_id` fields (nullable in M1) and a
  `rotation_marker` entry type (not emitted in M1). Omitting them forecloses SaaS multi-tenancy
  (M7) and stable-API log rotation (M6).
- Policy rules: MUST carry a `tags` field (empty array acceptable in M1). Required for compliance
  report template mapping (M8).

The long-term platform direction is a purpose-built OS for AI agent execution (OS1–OS2), where
enforcement moves below the application layer. Nothing in the proxy architecture should foreclose
OS-level integration — the library crate requirement already supports this trajectory. See
`docs/product/ARGOS_PRODUCT_VISION.md` §8 for the full technical rationale.

## Development Workflow

- All features follow the SDD process: `/speckit-specify` → `/speckit-clarify` → `/speckit-plan`
  → `/speckit-tasks` → `/speckit-implement`.
- Features begin on a dedicated branch created with `/speckit-git-feature`.
- All commits MUST follow Conventional Commits, enforced by the commitizen pre-commit hook.
- All commits touching `docs/`, `.claude/`, or `.specify/` are scanned for sensitive data by the
  Claude pre-commit hook before merge. This project is developed in public.
- Every spec MUST include a validation pass against the 10 compatibility constraints in
  `docs/product/ARGOS_V01_IDEA.md` §13 before implementation begins.
- Security-critical paths (policy engine, audit writer, transport adapters) require self-review
  against Principles I, II, V, and VI before any PR is considered complete.
- PRs MUST be merged with a true merge commit (no squash, no rebase-only). Non-linear commit
  history MUST be preserved in the repository so that individual commits remain reachable and
  reviewable after merge.
- Milestone status is tracked in `docs/ROADMAP.md` — the single source of truth for sequencing
  and current progress.

## Governance

This constitution supersedes all other project practices when conflicts arise. Amendments require:

1. A documented rationale for the change.
2. An assessment of which specs, plans, and tasks are affected.
3. A semantic version bump: MAJOR for principle removal or redefinition; MINOR for new principle
   or section; PATCH for clarifications and wording fixes.
4. An update to `LAST_AMENDED_DATE`.

Use `CLAUDE.md` for runtime development guidance (read by the AI assistant on every session).
Use `docs/product/` for product strategy and vision documents.
Use `docs/ROADMAP.md` for milestone sequencing and current status.

**Version**: 1.2.1 | **Ratified**: 2026-04-23 | **Last Amended**: 2026-04-27
