---
name: "argos-sync-docs"
description: "Propagate spec changes to all documents under docs/ to keep them in sync with the current feature spec. Run after any speckit skill that modifies the spec."
argument-hint: "Optional: focus hint (e.g. 'cli interface changed')"
compatibility: "Requires spec-kit project structure with .specify/ directory"
metadata:
  author: "argos-project"
  source: "custom"
user-invocable: true
disable-model-invocation: false
---

## User Input

```text
$ARGUMENTS
```

## Outline

Goal: After any skill updates the feature spec, scan every Markdown file under `docs/`
and update any passages that are stale or contradictory relative to the spec. The spec
is the single source of truth. All docs are downstream of it.

Execution steps:

1. Run `.specify/scripts/bash/check-prerequisites.sh --json --paths-only` to resolve
   `FEATURE_SPEC`. If it fails, warn and exit cleanly.

2. Read the current spec at `FEATURE_SPEC` in full.

3. Extract the canonical current values for every decision area present in the spec.
   Decision areas to always check (if present in the spec):

   - **Deployment targets** — which MCP clients are named as primary targets
   - **Intercepted message types** — which MCP messages are enforced vs. passed through
   - **CLI interface** — flag names, invocation patterns, and any inline examples
   - **Audit log schema** — field names and structure in audit entries
   - **Policy rule format** — `tool`, `resource`, constraint syntax, glob patterns, etc.
   - **Success criteria** — measurable outcomes and their thresholds
   - **Out-of-scope / deferred** — what is explicitly excluded and to which milestone
   - **License** — license name and dual-license rationale
   - **Primary personas** — who the primary and secondary users are
   - **Concurrency model** — how simultaneous requests are handled
   - **Transport mode inference** — how stdio vs HTTP mode is determined
   - Any other decision recorded in the spec's `## Clarifications` section

4. Find all Markdown files under `docs/` recursively:
   ```
   find docs/ -name "*.md"
   ```
   Read each file.

5. For each file under `docs/`:

   a. For each decision area extracted in step 3, search the document for any passage
      that describes that area — whether by explicit mention, example, or implication.

   b. If a passage is found and it contradicts or is stale relative to the spec's
      canonical value: replace it. Do not duplicate — replace in place.

   c. Preserve the document's own structure, heading hierarchy, voice, and tone.
      Minimise the diff: change only what the spec decision necessitates.

   d. If a decision area is referenced in the doc but the spec itself is ambiguous
      about it, skip that area for that document and note it.

   e. If no stale content is found in a document, do not write it.

6. Write all modified documents back to their paths.

7. Report:
   - Spec read from: `FEATURE_SPEC`
   - Documents scanned: total count
   - Documents updated: list each updated file with a one-line summary of what changed
   - Documents already in sync: count only (no list needed)
   - Skipped (ambiguous): any areas skipped due to spec ambiguity

Behavior rules:

- The spec is the source of truth. Never update the spec based on a doc — only the
  reverse.
- Never remove entire sections from a doc — only update stale passages within them.
- Never add content to a doc that does not derive from the spec.
- If a decision area is not mentioned anywhere in a document, do not add it — the doc
  may intentionally omit it.
- This skill is idempotent: running it twice in a row produces no further changes.
- If `docs/` does not exist, output "No docs/ directory found." and exit cleanly.
