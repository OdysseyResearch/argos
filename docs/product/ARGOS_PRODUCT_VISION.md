# Argos — Product Vision Document

**Version:** 0.1\
**Status:** Strategic foundation — informs all downstream SDD and implementation\
**Companion document:** `ARGOS_V01_IDEA.md` (v0.1 MVP scope)\
**Date:** April 2026

---

## 1. What Argos is

Argos is the open infrastructure layer for AI agent security. It gives enterprise security teams the capability enforcement, runtime governance, and tamper-evident audit trail they need to deploy MCP-connected AI agents safely — without sending a single byte of their data to a third party.

The FOSS proxy establishes Argos as the default standard — the way Falco became the standard for container runtime security and OPA became the standard for policy enforcement. The Argos OS raises the ceiling to what no userspace tool can offer: structural enforcement of the data/instruction separation problem at the kernel level. The SaaS control plane, built on top of the OS, converts that standard into a sustainable business.

The sequencing is deliberate: proxy first to earn community and credibility, OS second to build the defensible moat, SaaS third to monetise the combination. Revenue is not the goal of the proxy phase — the goal is becoming the standard before a well-funded competitor does.

---

## 2. The problem Argos solves

See `ARGOS_V01_IDEA.md` Section 2 for the full problem statement. In summary:

- MCP is now the de facto protocol for connecting AI agents to tools. It has no security layer.
- Every useful enterprise agent combines private data, untrusted content, and external communication channels — the structural exploit condition Simon Willison named the "lethal trifecta."
- 12+ MCP CVEs in 2025. 30% of enterprises experienced AI agent data exfiltration in 2025 (Vorlon).
- Commercial guardrails (Lakera, Prompt Security, CalypsoAI/F5) require enterprise data to leave the perimeter. Enterprise security policies prohibit this. Procurement cycles take quarters.
- No FOSS equivalent exists. The market gap is validated and the window is narrow — approximately 12–18 months before a major vendor fills it.

---

## 3. Business Model Canvas

### DESIRABILITY

---

#### 3.1 Customer Segments

**Segment 1 — AppSec lead / platform security engineer (primary)**

The person who integrates Argos. They evaluate FOSS tools, file the GitHub issues, write the integration PRs, and make the internal case for adoption. They gate the CISO's sign-off by solving the technical and trust problems first.

- Size: Every enterprise with >50 developers now deploying AI agents has at least one person in this role
- Reach: GitHub, security conferences (DEF CON, Black Hat), Hacker News, OWASP communities
- What they need: something that works, can be audited, doesn't add attack surface, integrates cleanly
- How they're reached: FOSS first — the product must earn their trust before any commercial conversation

**Segment 2 — CISO / security director (budget holder)**

The executive who approves budget for the SaaS control plane. They do not evaluate the code. They evaluate risk posture, compliance coverage, and board-presentable evidence.

- Size: Every enterprise with a regulated AI deployment has a CISO with AI security now in their top 3 priorities (Team8 2025: 37% of CISOs named it #1)
- Reach: CISO peer networks, Gartner briefings, security analyst reports, procurement processes initiated by Segment 1
- What they need: audit evidence for regulators, board dashboards, vendor accountability
- How they're reached: SaaS layer — after Segment 1 has already adopted the FOSS core

**Segment 3 — AI framework / platform teams (community)**

Teams building on top of LangChain, AutoGen, LlamaIndex, or building internal agent platforms. Not buyers — contributors and early adopters who extend Argos and drive organic adoption across the developer ecosystem.

- Reach: GitHub, framework community Discords, developer conferences
- What they need: a stable embeddable library, good documentation, a policy format they can build tooling around
- Their value to Argos: they create the ecosystem gravity that makes Argos the default

---

#### 3.2 Value Propositions

**For Segment 1 (AppSec / platform engineer) — FOSS core:**

> Argos is the only self-hosted, open-source MCP security proxy that enforces capability policies at the transport layer and produces a tamper-evident audit trail — with zero data egress and sub-millisecond overhead.

Pain relievers:

- Fills the security vacuum in MCP-connected agent deployments — no existing FOSS does this
- Self-hosted: enterprise prompts and tool outputs never leave the perimeter
- Deny-by-default posture with a human-readable TOML policy format
- Merkle-chained audit log produces tamper-evident evidence continuously, as a byproduct of normal operation
- Single Rust binary — no runtime dependencies, no CVE surface, integrates as a sidecar or library

Gain creators:

- Open source passes procurement without a vendor review cycle
- Policy spec format creates a reusable approval contract for new agent deployments
- Architectural capability enforcement (not probabilistic detection) — honest defence-in-depth positioning

**For Segment 2 (CISO) — SaaS control plane:**

> Argos Cloud turns your self-hosted audit logs and policy specs into compliance-ready evidence, board-presentable dashboards, and automated approval workflows — mapped to EU AI Act, NIST AI RMF, and ISO 42001 out of the box.

Pain relievers:

- Compliance evidence is generated continuously by the FOSS runtime — the SaaS layer maps it to specific regulatory requirements
- Eliminates the manual scramble of assembling audit evidence retroactively under regulatory deadline
- Central policy management across multiple agent deployments and teams

Gain creators:

- EU AI Act Article 13 transparency documentation produced automatically
- NIST AI RMF Govern/Measure evidence generated as a byproduct of normal operation
- Board-presentable dashboards showing AI risk posture without requiring engineering involvement
- Threat intel feed for emerging MCP attack patterns — security teams stay ahead of the CVE curve

---

#### 3.3 Channels

**FOSS core distribution:**

- GitHub (primary) — where Segment 1 discovers, evaluates, and contributes
- Crates.io — Rust ecosystem distribution
- OWASP LLM Top 10 alignment — positions Argos within the reference framework security teams already use
- Security conference talks (DEF CON, Black Hat) — credibility with Segment 1
- Hacker News, Lobsters, /r/netsec — organic developer community reach
- AI framework integration guides (LangChain, AutoGen, LlamaIndex) — reaches Segment 3 where they work

**SaaS control plane distribution:**

- Inbound from FOSS adoption — Segment 1 adopts FOSS, raises budget conversation with Segment 2
- Direct outreach to CISO networks after FOSS traction is established
- Gartner AI TRiSM market positioning — analysts already have a category for this
- Security integrator partnerships (SI channels) — long-term, not day one

---

#### 3.4 Customer Relationships

**Segment 1 — community-driven:**

- GitHub Issues and Discussions as the primary support channel
- Discord community for real-time help and contribution coordination
- Public roadmap — contributors can see where the project is going
- Transparent CVE disclosure process — builds trust with security-conscious users
- Documentation and tutorials for common enterprise deployment patterns

**Segment 2 — account-managed:**

- Self-service onboarding for SaaS (trial to paid)
- Dedicated account management for enterprise contracts (>$50k ARR)
- Quarterly compliance report review calls
- SLA-backed support for production incidents

---

### VIABILITY

---

#### 3.5 Revenue Streams

**Stream 1 — SaaS subscription (primary, recurring)**

Argos Cloud: policy console, compliance report generation, threat intel feed, approval workflows, SIEM integration.

Pricing model:

- Per-agent-session pricing: scales naturally with customer AI adoption
- Suggested tiers:
  - **Starter** (≤10 concurrent agent sessions): €299/month — for startups and small teams
  - **Team** (≤100 concurrent agent sessions): €999/month — for mid-market
  - **Enterprise** (unlimited + SSO + SLA + custom compliance mappings): €4,000+/month — for regulated enterprises

Rationale: per-agent-session pricing aligns cost with value. As customers deploy more agents, they pay more — and they are more exposed, so the product is worth more to them. This avoids the per-seat trap where pricing conflicts with open-source adoption.

**Stream 2 — Enterprise support contracts (secondary, recurring)**

Priority issue resolution, guaranteed response SLAs, dedicated Slack channel. Separate from SaaS subscription — sold to enterprises that self-host the full stack and do not use Argos Cloud.

Pricing: €2,000–€8,000/month depending on SLA tier.

**Stream 3 — Professional services (tertiary, non-recurring)**

Policy design workshops, integration engagements, compliance readiness assessments. Sold as fixed-price engagements. Valuable in early stages for learning customer deployment patterns; deprioritised as the product matures.

**What is not monetised:**

- The FOSS core (argos-proxy, policy engine, audit log, MCP proxy) — permanently free and open source
- Individual contributors are never charged for the core
- No freemium tricks on the FOSS layer — the community trust this creates is the primary asset

---

#### 3.6 Cost Structure

As a one-person project, cost discipline is existential. All costs are either founder time or infrastructure.

**Founder time (primary cost):**

- Core Rust development
- Security research (MCP CVE tracking, threat intel)
- Community management (GitHub, Discord)
- SaaS platform development and operation
- Compliance template maintenance (regulations change)
- Sales and account management (eventually)

**Infrastructure costs (SaaS layer):**

- Cloud hosting for Argos Cloud (estimated €300–800/month at early scale)
- CI/CD pipeline (GitHub Actions — free tier initially)
- Monitoring and alerting (Grafana Cloud free tier initially)
- Security tooling (dependency scanning, SAST)

**One-time costs:**

- Legal: FOSS license selection and review, SaaS terms of service, privacy policy — estimated €1,500–3,000
- Domain, branding assets

**Cost structure implication:** the business is viable at very low ARR because the product is almost entirely founder time. First paying customer at €299/month is meaningful. Break-even on infrastructure at 3–5 customers. First €100k ARR is achievable with fewer than 25 customers.

---

### FEASIBILITY

---

#### 3.7 Key Resources

**Intellectual:**

- The Argos codebase (Rust) — the primary asset
- The policy spec format and DSL — if this becomes the standard format security teams write, the ecosystem locks in regardless of runtime choice. This is the OPA lesson.
- The threat intel feed (eventually) — a proprietary, continuously updated dataset of MCP attack patterns and injection signatures
- Compliance templates — pre-mapped policy outputs for EU AI Act, NIST AI RMF, ISO 42001

**Human:**

- Founder's Rust expertise and security knowledge
- Community contributors (earned through FOSS trust)

**Community:**

- GitHub stars and adoption metrics — social proof that accelerates enterprise adoption
- OWASP alignment — borrowed credibility from a trusted security standard

---

#### 3.8 Key Activities

**Phase 1 — Establish the standard (v0.1–v0.3):**

- Build and ship the FOSS MCP security proxy (argos-proxy) — see `ARGOS_V01_IDEA.md`
- Evolve the TOML policy format toward a full capability DSL
- Track and respond to MCP ecosystem CVEs publicly — become the voice of MCP security
- Write extensively about MCP security: blog posts, OWASP contributions, conference talks
- Build community: GitHub stars, Discord, first external contributors

**Phase 2 — Expand the runtime (v0.4–v1.0):**

- Add prompt content inspection layer (injection pattern detection as defence-in-depth)
- Add multi-agent session management
- Add OpenTelemetry GenAI span emission
- Add Sigstore/Rekor audit log anchoring (optional, for regulated environments)
- Support the top 3 enterprise agent frameworks natively (LangChain, AutoGen, OpenAI Agents SDK)

**Phase 3 — Launch SaaS (v1.0+):**

- Build and launch Argos Cloud (policy console, compliance reports, approval workflows)
- Build threat intel feed (proprietary MCP attack pattern database)
- Build SIEM integrations (Splunk, Elastic, Microsoft Sentinel)
- Begin enterprise sales

**Continuous:**

- Security research — stay ahead of the CVE curve
- Compliance template maintenance — regulations evolve
- Community management — GitHub, Discord, documentation

---

#### 3.9 Key Partnerships

**MCP ecosystem:**

- Anthropic (MCP protocol owner) — alignment with their security guidance increases credibility
- Microsoft (GitHub MCP, VS Code Copilot) — integration with their tooling reaches the largest enterprise developer base
- These are not formal partnerships initially — they are community relationships earned through security research contributions

**Standards bodies:**

- OWASP — align Argos with the LLM Top 10 reference framework. Become a recommended tool.
- NIST — contribute to AI RMF guidance on runtime controls. Positions Argos as standards-aligned.

**Agent framework communities:**

- LangChain, AutoGen, LlamaIndex — integration guides and official plugin status
- These communities drive Segment 3 adoption, which creates ecosystem gravity

**Distribution partnerships (later stage):**

- Security integrators and MSSPs — for enterprise channel distribution
- Cloud providers — marketplace listings (AWS Marketplace, Azure Marketplace) for frictionless enterprise procurement

---

### ADAPTABILITY

---

#### 3.10 Environment Analysis

**Industry forces (threats and opportunities):**

- Gartner predicts GenAI TRiSM market consolidation by end of 2026. F5/CalypsoAI, Check Point/Lakera, SentinelOne/Prompt Security have already demonstrated the M&A pattern.
- *Threat:* a major vendor (Palo Alto, CrowdStrike, Wiz) builds or acquires an MCP security layer, commoditising the core.
- *Defence:* the FOSS standard, once established, is not acquirable. HashiCorp showed this works; it also showed its limits when they moved to BSL. Argos should use Apache 2.0 or MIT — no licence switching risk.
- *Opportunity:* consolidation validates the market and drives enterprise urgency. Being the open standard before consolidation is the only defensible position that doesn't depend on being acquired.

**Market forces:**

- MCP adoption is accelerating. Every major agent framework now has MCP support. The protocol is 18 months old and already ubiquitous.
- Enterprise AI agent counts are growing from dozens to hundreds per organisation. Each one is a new security surface. This scales the pain.
- CISO AI security budgets growing: 70% of organisations allocating >10% of security budget to AI (IANS 2025).

**Key trends:**

- EU AI Act enforcement began August 2025. NIST AI RMF is entering US federal procurement requirements. ISO 42001 certifications are being demanded in enterprise vendor RFPs. Compliance is no longer optional.
- OpenTelemetry GenAI Semantic Conventions standardised in 2025 — Argos should align with this from v0.1 so the audit log is compatible with existing observability infrastructure.
- "Guardian agent" pattern emerging (Gartner 2026) — AI systems supervising other AI systems. Argos is positioned to become the guardian layer.

**Macroeconomic forces:**

- Security budget growth slowing (4% in 2025 vs 8% prior year, IANS) — but AI security is the growth area within flat budgets. New category, not a budget reallocation fight.
- One-person business model means extremely low burn — can survive on minimal revenue while building community.

---

## 4. The product flywheel

The business model has three distinct phases. Understanding the sequencing is essential to making
correct product decisions at every stage.

```
Phase 1 — Standard
FOSS proxy adoption
      │
      ▼
Community trust → GitHub stars → Segment 3 integrations → Ecosystem gravity
      │
      ▼
Enterprise AppSec teams adopt (Segment 1) → internal advocacy
      │
      ▼
Grants + angel funding unlocked by community credibility
      │
      ▼
Phase 2 — Moat
Argos OS — kernel-level enforcement + data/instruction separation
      │
      ▼
Architectural guarantees no userspace tool can match
      │
      ▼
Segment 1 upgrades → Segment 2 (CISO) conversation opens
      │
      ▼
Phase 3 — Revenue
Argos Cloud — SaaS built on OS-level audit guarantees
      │
      ▼
ARR funds ongoing OS + proxy development
      └──────────────────────────────────────────────────────────┘
```

**Why this sequencing**: a solo founder cannot outspend large vendors on SaaS features. Racing
to compliance dashboards and approval workflows puts Argos in a fight it cannot win. The OS +
data/instruction separation is a multi-year research bet that large vendors will not take because
it does not fit their quarterly roadmap. Deep focus and no organisational overhead are the
competitive advantages here — they compound on the hard problem, not the easy one.

**The key constraint on Phase 1**: the FOSS core must be genuinely excellent and permanently free.
Any attempt to move features from FOSS to SaaS (the HashiCorp mistake) breaks community trust
and ends the flywheel before Phase 2 can start.

**The funding path**: FOSS reputation and community → grants (NLnet, Sovereign Tech Fund,
EU Horizon AI) → angel/seed from security-focused investors once the OS thesis is demonstrated.
SaaS revenue is not required to reach the OS phase — grants are the bridge.

---

## 5. Product roadmap

See [`docs/ROADMAP.md`](../ROADMAP.md) — the single source of truth for milestones, sequencing,
and current status.

Summary: three phases in deliberate order — **Standard** (FOSS proxy, M1–M6) to earn community
credibility, **Moat** (Argos OS, OS1–OS2) to build the defensible technical advantage, **Revenue**
(SaaS, M7–M9) built on top of the OS so the commercial product launches differentiated rather
than racing incumbents on compliance dashboards.

---

## 6. Future compatibility constraints for v0.1

These are the constraints the MVP must respect to avoid architectural debt that blocks the full product. Each constraint is justified by a specific later-stage requirement.

See `ARGOS_V01_IDEA.md` Section 13 for the full constraint list with implementation notes.

Summary:

1. **Audit log schema must include reserved fields** for `org_id` and `tenant_id` — required for SaaS multi-tenancy
2. **Policy file must have a `version` field** — required for non-breaking DSL evolution
3. **Session ID must be UUID v4** — required for global uniqueness across distributed deployments
4. **SHA-256 as hash function** — required for Sigstore/Rekor compatibility
5. **OTel span emission as a feature flag** — required for v0.4 observability without breaking v0.1 behaviour
6. **argos-proxy must be embeddable as a library crate** (not only a binary) — required for SaaS control plane integration
7. **MCP error responses must be spec-compliant** — required for forward compatibility as MCP evolves
8. **HTTP/SSE mode must accept mTLS configuration** (even if not enforced in v0.1) — required for enterprise deployment patterns
9. **Audit log format must support rotation markers** — required for v1.0 log rotation without format changes
10. **Policy rules must carry a `tags` field** — required for compliance report template mapping (EU AI Act articles, NIST controls)

---

## 7. What Argos is not — and will not become

Argos is a runtime security layer. It is not, and will not become:

- An **agent platform** or framework (OpenFang, LangChain, AutoGen)
- An **agent orchestrator** or scheduler
- A **tool provider** or capability runtime
- A **chatbot interface** or conversation manager
- A **secrets manager** or credential vault (policy rules can block access to secret paths —
  the vault is not built in)

This focus is intentional and strategic. Security credibility is earned by doing one thing
exceptionally well and being auditable. A product that also runs agents, manages channels, and
provides autonomous capabilities cannot credibly claim to be a neutral security boundary — it
has inherent conflicts of interest between capability and enforcement.

Argos's value proposition is precisely that it is **agnostic to the agent platform**. It can
secure an OpenFang deployment, a LangChain agent, a custom AutoGen pipeline, or anything else
that speaks MCP — because it operates at the transport layer, not inside the runtime. Expanding
into orchestration or agent capabilities would turn Argos into a competitor of the platforms it
needs to integrate with, and destroy the ecosystem neutrality that drives adoption.

**When a feature request sounds like "Argos should also do X for agents", the answer is no.**
The right answer is "Argos should enforce the security boundary around X, whatever X is."

---

## 8. Long-term platform direction — Argos OS

The M1–M9 proxy roadmap establishes Argos as the standard for MCP runtime security. The logical
long-term extension is a purpose-built OS for AI agent execution — a separate product line, not
a version of the proxy.

**Why the proxy hits a ceiling**: a userspace proxy can be bypassed by a sufficiently compromised
agent — one with elevated privileges could circumvent the proxy, write directly to the network,
or tamper with audit logs on the same filesystem. An OS closes those attack vectors by pushing
enforcement below the application layer: into the kernel, the scheduler, and hardware-rooted
trust (TPM, secure boot). Tamper-evidence becomes structural rather than cryptographic convention.

**The precedent**: Bottlerocket (AWS) and Talos Linux demonstrate that purpose-built OS
distributions for specific runtime security concerns are viable and valued in enterprise
infrastructure. An "AI agent execution OS" is a coherent product category that does not yet exist.

**The sequencing**: the proxy must win the community and establish the standard first — that earns
the credibility and distribution to make an OS adoption story possible. The proxy is the wedge;
the OS is the long-term defensible moat.

**Architectural implication**: nothing in the proxy roadmap should foreclose OS-layer integration.
The library crate requirement (constitution Principle IV, constraint 6) already supports this —
the policy engine embedded in an OS-level enforcement daemon is a natural evolution of the same
codebase.

### Data/instruction separation — the hard problem the OS unlocks

The fundamental vulnerability of MCP-connected agents is unified context: the model receives a
single token stream with no intrinsic way to distinguish user instructions from tool output from
adversarial payload. Filtering cannot fix this — the channel is unified by design.

An OS-level solution can enforce separation *before* content reaches the model:

**Memory tagging / taint tracking**: tag every byte of data with its provenance
(`TRUSTED_INSTRUCTION`, `USER_INPUT`, `TOOL_OUTPUT`, `EXTERNAL_CONTENT`) and maintain those tags
through the entire pipeline. When a `TOOL_OUTPUT`-tagged token appears in an instruction position,
that is a structural violation — flag or block it before the model processes it. ARM MTE makes
this viable in hardware.

**Dual-channel context construction**: enforce two separate channels into the model runtime — a
signed instruction channel and an untrusted data channel. The runtime treats data-channel tokens
as data regardless of content. Requires cooperation from the inference runtime (llama.cpp, vLLM)
but is architecturally sound — it mirrors CPU code/data segment separation.

**Immutable instruction manifests**: at session start, the instruction set is cryptographically
committed and locked. Any attempt to inject instructions mid-session is detectable as a manifest
violation, not a model judgement call.

**Capability-sealed tool outputs**: tool outputs are delivered in OS-enforced sealed envelopes.
Nothing from a tool output can appear in the instruction prefix of the context. The model sees
tags that are structurally enforced, not convention.

The honest claim this enables: *Argos OS enforces structural separation of instructions and data
at the pipeline level, eliminating the class of injection attacks that exploit unified context
construction.* Strong, defensible, and something no current vendor can claim.

---

## 8. What success looks like

**6 months (post v0.1 ship):**

- 500+ GitHub stars
- 3+ enterprise security teams using argos-proxy in production or evaluation
- Referenced in at least one OWASP LLM Top 10 discussion or community contribution
- v0.2 shipped with DSL improvements informed by real user feedback

**12 months:**

- 2,000+ GitHub stars
- 10+ enterprise production deployments
- First paid Argos Cloud customer
- Mentioned in at least one Gartner or Forrester research note
- Community has produced at least one third-party integration (LangChain plugin, etc.)

**24 months:**

- €30k+ MRR from Argos Cloud
- 50+ enterprise deployments
- Argos is the answer when someone asks "what FOSS tool do you use for MCP security?"
- Threat intel feed launched and used as a differentiator in enterprise sales
