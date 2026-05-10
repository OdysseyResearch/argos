# Prompts

This directory archives prompts given to AI assistants (e.g. Claude Code) that
produced meaningful changes to checked-in artifacts — the roadmap, the product
vision, architecture decisions, and similar living documents.

The goal is **traceability**. When a document changes, the diff shows *what*
changed and the commit message shows a short *why*. The prompt that drove the
change is usually richer than either — it captures the framing, constraints,
and intent the contributor had at the time. Keeping prompts here lets future
collaborators (or future-us) reconstruct the reasoning behind a document's
evolution without needing the original chat transcript.

## When to add a prompt

Add a prompt here when **all** of the following hold:

- The prompt drove a non-trivial change to one or more checked-in documents
  (roadmap, vision, ADRs, top-level READMEs, etc.).
- The framing or intent in the prompt is not fully recoverable from the
  resulting diff and commit message alone.
- The prompt represents a decision point worth being able to revisit later.

Skip it for trivial edits, typo fixes, or prompts that are essentially
"refactor this small thing" — the diff speaks for itself in those cases.

## When *not* to use this directory

- **Reusable prompts** → custom slash commands under `.claude/commands/*.md`.
- **Speckit feature specs** → `specs/NNN-feature-slug/` via the speckit
  workflow (`/speckit-specify`, `/speckit-plan`, etc.).
- **Standing instructions for the AI** → the project `CLAUDE.md`.
- **Ephemeral one-off prompts** → just paste them in chat; no archiving needed.

## Filename convention

```
YYYY-MM-DD-short-slug.md
```

- Date prefix (ISO 8601) so files sort chronologically.
- Slug should make the topic obvious at a glance.

Example: `2026-05-07-roadmap-capability-milestones.md`.

## File contents

Each prompt file should contain:

1. A short header block with metadata:
   - **Artifact(s):** the file(s) the prompt produced or modified.
   - **Result:** the resulting commit hash, PR number, or branch.
   - **Notes (optional):** any clarifying follow-ups that came up during the
     session and shaped the final output.
2. The prompt **verbatim**, in its original phrasing — including imperfections.
   Do not clean it up after the fact; the rough edges are part of the record.

Minimal template:

```markdown
---
artifact: docs/ROADMAP.md
result: <commit-sha-or-PR-number>
date: YYYY-MM-DD
---

<original prompt, verbatim>
```

## Conventions for collaborators

- Treat these files as **append-only history**. Do not edit a prompt after
  it's been committed; if the framing changed, write a new prompt file.
- Commit the prompt file in the same PR as the artifact change it drove,
  so the diff and its rationale travel together.
- If a prompt produced multiple artifacts across separate commits, list all
  of them in the `artifact:` / `result:` header.
