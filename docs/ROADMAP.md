# Argos Roadmap

Version numbers follow SemVer strictly — majors bump on breaking changes to the policy format,
CLI interface, audit log schema, or library API; they are not pinned to milestones. The one
intentional marker is **Stable API** (0.x → 1.0.0), which signals that public interfaces are
committed and production-safe. All other milestones ship at whatever version the changelog
dictates.

For the strategic rationale behind this sequencing see
[`docs/product/ARGOS_PRODUCT_VISION.md`](product/ARGOS_PRODUCT_VISION.md) §4.

---

## Phase 1 — Standard (FOSS proxy)

Goal: establish Argos as the default open standard for MCP runtime security before a well-funded
competitor does. Revenue is not the goal of this phase — community credibility is.

Milestones are sequenced from minimum viable proxy outward: each one extends enforcement coverage,
policy expressiveness, observability, or integrity guarantees on top of the previous.

| ID | Milestone                       | Key capability                                                                                                                                                                | Status  |
| -- | ------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------- |
| M1 | **Proxy core**                  | stdio + HTTP/SSE proxy, TOML policy, deny-by-default, Merkle audit log, `argos-proxy verify` (audit chain integrity check). Compatible with any MCP-compliant client          | ✅ Done |
| M2 | **Wire coverage & operability** | `prompts/*` and `sampling/createMessage` enforcement, tool name glob patterns (e.g. `read_*`), policy hot-reload, `argos-proxy check-policy` (policy linter), Windows support | Planned |
| M3 | **Framework adapters**          | LangChain, AutoGen, OpenAI Agents SDK integration helpers and guides                                                                                                          | Planned |
| M4 | **Policy DSL**                  | Richer constraint expressions (`regex`, `one_of`, `contains`), rule composition, policy templates for common use cases                                                        | Planned |
| M5 | **Observability**               | OpenTelemetry GenAI span emission, remote log sinks (S3, syslog, OTel collector)                                                                                              | Planned |
| M6 | **Stable API** *(0.x → 1.0.0)*  | Sigstore/Rekor audit log anchoring, mTLS enforcement, multi-agent session management, performance benchmarks. Public interfaces declared stable                               | Planned |
| M7 | **Defence-in-depth**            | Structural anomaly detection + known MCP attack pattern signatures. Defence-in-depth layer — not a prevention claim                                                           | Planned |

---

## Phase 2 — Moat (Argos OS)

A separate product line sharing the policy engine core. Starts after M6 has established community
credibility and unlocked grant funding. See
[`docs/product/ARGOS_PRODUCT_VISION.md`](product/ARGOS_PRODUCT_VISION.md) §8 for the full
technical rationale and the data/instruction separation mechanisms.

| ID  | Milestone                       | Key capability                                                                                                                      | Status |
| --- | ------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------- | ------ |
| OS1 | **Argos OS foundation**         | Purpose-built agent execution OS: immutable root, no shell, kernel-level policy enforcement, TPM-rooted audit log                   | Future |
| OS2 | **Data/instruction separation** | Memory tagging (taint tracking), dual-channel context construction, immutable instruction manifests, capability-sealed tool outputs | Future |

---

## Phase 3 — Revenue (SaaS)

Built on top of the OS. SaaS audit guarantees backed by kernel-level enforcement are
fundamentally stronger than any proxy-based equivalent, making this a differentiated product
at launch rather than a compliance-dashboard race against well-funded incumbents.

| ID | Milestone              | Key capability                                                    | Status |
| -- | ---------------------- | ----------------------------------------------------------------- | ------ |
| M7 | **Argos Cloud alpha**  | Policy console, basic compliance reports, approval workflows      | Future |
| M8 | **Compliance reports** | EU AI Act, NIST AI RMF, ISO 42001 templates; SIEM integration     | Future |
| M9 | **Threat intel**       | Proprietary MCP attack pattern feed, automated policy suggestions | Future |
