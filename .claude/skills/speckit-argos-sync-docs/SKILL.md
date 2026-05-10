---
name: speckit-argos-sync-docs
description: Propagate spec changes to all docs/ markdown and README.md to keep them
  in sync with the active feature spec.
compatibility: Requires spec-kit project structure with .specify/ directory
metadata:
  author: github-spec-kit
  source: argos:commands/speckit.argos.sync-docs.md
---

## User Input

```text
$ARGUMENTS
```

You **MUST** consider the user input before proceeding (if not empty).

## Pre-Execution Checks

**Check for extension hooks (before sync-docs)**:

- Check if `.specify/extensions.yml` exists in the project root.
- If it exists, read it and look for entries under the `hooks.before_sync_docs` key.
- If the YAML cannot be parsed or is invalid, skip hook checking silently and continue normally.
- Filter out hooks where `enabled` is explicitly `false`. Treat hooks without an `enabled` field as enabled by default.
- For each remaining hook, do **not** attempt to interpret or evaluate hook `condition` expressions:
  - If the hook has no `condition` field, or it is null/empty, treat the hook as executable.
  - If the hook defines a non-empty `condition`, skip the hook and leave condition evaluation to the HookExecutor implementation.
- When constructing slash commands from hook command names, replace dots (`.`) with hyphens (`-`). For example, `speckit.git.commit` → `/speckit-git-commit`.
- For each executable hook, output the following based on its `optional` flag:
  - **Optional hook** (`optional: true`):
    ```
    ## Extension Hooks

    **Optional Pre-Hook**: {extension}
    Command: `/{command}`
    Description: {description}

    Prompt: {prompt}
    To execute: `/{command}`
    ```
  - **Mandatory hook** (`optional: false`):
    ```
    ## Extension Hooks

    **Automatic Pre-Hook**: {extension}
    Executing: `/{command}`
    EXECUTE_COMMAND: {command}

    Wait for the result of the hook command before proceeding to the Goal.
    ```
- If no hooks are registered or `.specify/extensions.yml` does not exist, skip silently.

## Goal

After any skill updates the feature spec, scan every Markdown file under `docs/` and `README.md` and update any passages that are stale or contradictory relative to the spec. The spec is the single source of truth. All docs are downstream of it.

## Execution Steps

### 1. Resolve Spec Path

Run `.specify/scripts/bash/check-prerequisites.sh --json --paths-only` to resolve `FEATURE_SPEC`. If it fails, warn and exit cleanly.

### 2. Load Spec

Read the current spec at `FEATURE_SPEC` in full.

### 3. Extract Canonical Decision Areas

Extract the current values for every decision area present in the spec. Decision areas to always check (if present in the spec):

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

### 4. Collect Documents

Find all Markdown files to sync — two sources:

- All files under `docs/` recursively: `find docs/ -name "*.md"`
- `README.md` at the repo root (always included if it exists)

Read each file.

### 5. Sync Each Document

For each file (`docs/**/*.md` and `README.md`):

a. For each decision area extracted in step 3, search the document for any passage that describes that area — whether by explicit mention, example, or implication.

b. If a passage is found and it contradicts or is stale relative to the spec's canonical value: replace it. Do not duplicate — replace in place.

c. Preserve the document's own structure, heading hierarchy, voice, and tone. Minimise the diff: change only what the spec decision necessitates.

d. If a decision area is referenced in the doc but the spec itself is ambiguous about it, skip that area for that document and note it.

e. If no stale content is found in a document, do not write it.

### 6. Write Updated Documents

Write all modified documents back to their paths.

### 7. Report

- Spec read from: `FEATURE_SPEC`
- Documents scanned: total count
- Documents updated: list each updated file with a one-line summary of what changed
- Documents already in sync: count only (no list needed)
- Skipped (ambiguous): any areas skipped due to spec ambiguity

## Behavior Rules

- The spec is the source of truth. Never update the spec based on a doc — only the reverse.
- Never remove entire sections from a doc — only update stale passages within them.
- Never add content to a doc that does not derive from the spec.
- If a decision area is not mentioned anywhere in a document, do not add it — the doc may intentionally omit it.
- This skill is idempotent: running it twice in a row produces no further changes.
- If `docs/` does not exist and `README.md` is absent, output "No docs/ directory or README.md found." and exit cleanly.
- If only `README.md` exists (no `docs/`), sync README.md alone and report normally.

## Post-Execution Checks

**Check for extension hooks (after sync-docs)**:

- Check if `.specify/extensions.yml` exists in the project root.
- If it exists, read it and look for entries under the `hooks.after_sync_docs` key.
- If the YAML cannot be parsed or is invalid, skip hook checking silently and continue normally.
- Filter out hooks where `enabled` is explicitly `false`. Treat hooks without an `enabled` field as enabled by default.
- For each remaining hook, do **not** attempt to interpret or evaluate hook `condition` expressions:
  - If the hook has no `condition` field, or it is null/empty, treat the hook as executable.
  - If the hook defines a non-empty `condition`, skip the hook and leave condition evaluation to the HookExecutor implementation.
- When constructing slash commands from hook command names, replace dots (`.`) with hyphens (`-`). For example, `speckit.git.commit` → `/speckit-git-commit`.
- For each executable hook, output the following based on its `optional` flag:
  - **Optional hook** (`optional: true`):
    ```
    ## Extension Hooks

    **Optional Hook**: {extension}
    Command: `/{command}`
    Description: {description}

    Prompt: {prompt}
    To execute: `/{command}`
    ```
  - **Mandatory hook** (`optional: false`):
    ```
    ## Extension Hooks

    **Automatic Hook**: {extension}
    Executing: `/{command}`
    EXECUTE_COMMAND: {command}
    ```
- If no hooks are registered or `.specify/extensions.yml` does not exist, skip silently.