---
name: speckit-argos-pr-review
description: 'Spec-grounded review of an open PR/MR: reads spec/plan/tasks/contracts/constitution
  and the branch diff, then posts a structured report via the detected git provider.'
compatibility: Requires spec-kit project structure with .specify/ directory
metadata:
  author: github-spec-kit
  source: argos:commands/speckit.argos.pr-review.md
---

## User Input

```text
$ARGUMENTS
```

You **MUST** consider the user input before proceeding. The first positional argument is an optional PR number — if omitted, auto-detect from the current branch's open PR. The flag `--no-comment` skips the GitHub comment and prints the report to the console only.

## Pre-Execution Checks

**Check for extension hooks (before pr-review)**:

- Check if `.specify/extensions.yml` exists in the project root.
- If it exists, read it and look for entries under the `hooks.before_pr_review` key.
- If the YAML cannot be parsed or is invalid, skip hook checking silently and continue normally.
- Filter out hooks where `enabled` is explicitly `false`. Treat hooks without an `enabled` field as enabled by default.
- For each remaining hook, do **not** attempt to interpret or evaluate hook `condition` expressions:
  - If the hook has no `condition` field, or it is null/empty, treat the hook as executable.
  - If the hook defines a non-empty `condition`, skip the hook and leave condition evaluation to the HookExecutor implementation.
- When constructing slash commands from hook command names, replace dots (`.`) with hyphens (`-`).
- For each executable hook, output the same optional/mandatory blocks the other speckit skills use; if no hooks are registered, skip silently.

## Goal

Produce a **spec-grounded** review of an open pull request: does the code in this PR actually deliver what the active feature's spec, plan, tasks, contracts, and constitution require? Surface gaps, drift, and untested paths. Post the report as a comment on the PR for the audit trail.

This skill is **complementary** to the existing tools, not a replacement:

- `/speckit-analyze` reads only spec/plan/tasks artifacts; it never reads code.
- Generic code review (CodeRabbit, etc.) reads code but has no spec context.
- `/ultrareview` is a separate, multi-agent cloud review with broader scope.

This skill is the diagonal: it reads the **code** through the lens of the **spec**.

## Operating Constraints

**STRICTLY READ-ONLY against repository files.** Do not modify any files in the working tree, including the spec, plan, tasks, code, or contracts. The only side effect this skill produces is **a single PR comment via the detected provider** (skipped when `--no-comment` is set). All findings must be reported, never auto-fixed; remediation is a separate, explicit decision by the operator.

**Active feature only.** Use the active feature directory resolved by `.specify/scripts/bash/check-prerequisites.sh`. Do not scan unrelated specs.

**Constitution Authority.** Constitution principle violations are automatically `CRITICAL`.

## Execution Steps

### 1. Initialize Review Context

Run `.specify/scripts/bash/check-prerequisites.sh --json --require-tasks --include-tasks` once from repo root and attempt to parse JSON for `FEATURE_DIR`.

**If the script succeeds**, derive full artifact paths and run all detection passes:

- `SPEC` = `FEATURE_DIR/spec.md`
- `PLAN` = `FEATURE_DIR/plan.md`
- `TASKS` = `FEATURE_DIR/tasks.md`
- `CONTRACTS_DIR` = `FEATURE_DIR/contracts/` (optional)
- `CONSTITUTION` = `.specify/memory/constitution.md`

**If the script fails due to branch naming** (error message references feature branch format), enter **reduced mode**:

- Load `CONSTITUTION` = `.specify/memory/constitution.md` only.
- Skip FR coverage, SC evidence, and contract conformance passes.
- Run constitution alignment, scope drift, and test gap passes against the diff.
- Print at the top of the report: *"Reduced mode: non-speckit branch — FR/SC/contract passes skipped."*

**If `.specify/` is missing entirely**, abort with: *"No .specify/ directory found. Is this a speckit project?"*

### 2. Detect Provider

Provider detection is required only when posting a comment. If `--no-comment` is set, skip this step entirely.

Probe in order — use the first one that resolves:

| Priority | Provider   | Detection command          | PR/MR term |
|----------|------------|----------------------------|------------|
| 1        | GitHub     | `gh auth status`           | PR         |
| 2        | GitLab     | `glab auth status`         | MR         |
| 3        | Bitbucket  | `bb --version`             | PR         |

Store the result as `PROVIDER` (one of `github`, `gitlab`, `bitbucket`, or `none`).

If no provider CLI is detected and `--no-comment` was NOT passed, warn the user:
*"No provider CLI detected (tried gh, glab, bb). The report will be printed to stdout only. Install a provider CLI and re-run to post it as a comment."*
Then continue with `--no-comment` behaviour.

### 3. Resolve the PR/MR Number

If the user passed a PR/MR number as the first argument, use it directly. Otherwise auto-detect using the provider:

| Provider  | Command                                        |
|-----------|------------------------------------------------|
| github    | `gh pr view --json number --jq .number`        |
| gitlab    | `glab mr view --output json \| jq .iid`        |
| bitbucket | `bb pr view --json \| jq .id`                  |

If detection fails or returns nothing, abort with:
*"No open PR/MR for the current branch. Open one and re-run, or pass the number explicitly."*

### 4. Load Inputs

Load only the minimal necessary content from each input:

**From spec.md:**

- Functional Requirements (FR-### identifiers + statements)
- Success Criteria (SC-### identifiers + measurable outcomes)
- User Stories (priorities, acceptance scenarios)
- Edge Cases
- Compatibility Constraints (if present — e.g. ARGOS_V01_IDEA.md §13 references)

**From plan.md:**

- Technical Context
- Architecture / source tree
- Constitution Check
- Phase entries

**From tasks.md:**

- Task IDs + descriptions + completion state (`[ ]` vs `[x]`)
- Each task's referenced FR-/SC-/file paths

**From contracts/ (each file):**

- Public API shape: CLI flags, schema fields, function signatures, error codes

**From constitution.md:**

- Principle names + MUST/SHOULD normative statements
- Architecture Constraints

**From the branch diff (via git — provider-independent):**

Compute the merge-base against the main branch, then:

```sh
# Unified diff of all changes on this branch
BASE=$(git merge-base origin/main HEAD)
git diff "$BASE" HEAD

# Changed-file list with stats
git diff --name-status "$BASE" HEAD
git diff --stat "$BASE" HEAD
```

Filter the changed-file list to extensions relevant to plan.md's stack (e.g. for Rust: `*.rs`, `Cargo.toml`).

Use `git log "$BASE"..HEAD --oneline` to record the commit range for the report header.

### 5. Build Semantic Models

Internal only — do not include raw artifacts in the output:

- **Requirements inventory**: FR-### → spec statement → expected code surface (file paths from tasks/contracts).
- **Success Criteria inventory**: SC-### → measurable threshold → expected verification (test name, benchmark, etc.).
- **Contract surface**: every CLI flag, schema field, public function in contracts/ → expected to appear in code.
- **Task ↔ file map**: each `[x]` task's referenced file(s).
- **Constitution rule set**: principle name → MUST statements that imply observable code properties.

### 6. Detection Passes

Limit to 50 findings total; aggregate the rest in an "Overflow" summary.

#### A. FR Coverage

For each FR-### in the spec:

- Find the task(s) that claim to implement it (tasks.md cross-reference).
- Confirm the referenced files exist in the changed-file set OR were merged earlier.
- Grep the implementation files for behaviour matching the FR statement (best-effort textual matching — exact regex is left to the analyst's judgement).
- **Finding**: any FR with no implementing task, or a task that wasn't completed (`[ ]` still), or referenced files missing from the diff and not present on `main`.

#### B. Contract Conformance

For each contract document in `contracts/`:

- Extract the public-surface tokens (CLI flag names, JSON field names, function signatures, error codes, schema enum variants).
- Grep the changed code for each token. If the contract specifies a flag like `--policy`, the code must declare it.
- **Finding (HIGH)**: contract token absent in code.
- **Finding (HIGH)**: code introduces a public surface element NOT present in any contract — possible undocumented API.

#### C. Success Criterion Evidence

For each SC-###:

- Identify the verification mechanism: a test name, a benchmark, a CI check, or a manual procedure documented in the PR.
- Confirm a test/benchmark exists in the changed-file set OR in the existing test suite that exercises that SC.
- **Finding**: SC with no traceable verification.

#### D. Constitution Alignment

For each constitution principle with a MUST clause:

- Translate the MUST into an observable code property where possible (e.g., "Zero Data Egress" → no outbound network calls outside the explicit upstream forwarder).
- Grep the diff for violations.
- **Finding (CRITICAL)**: any code change that violates a principle MUST.

#### E. Public API Surface

For projects with a `[lib]` target (or equivalent):

- Enumerate every `pub` symbol added or modified in the diff.
- Cross-reference with `contracts/library-api.md` (or equivalent).
- **Finding (HIGH)**: a `pub` symbol not in any contract — accidental API leak.
- **Finding (MEDIUM)**: a contract-promised public symbol that is missing or `pub(crate)` in code.

#### F. Scope Drift

- Identify diff content that touches files NOT referenced by any task in tasks.md AND introduces non-trivial behaviour (more than imports, formatting, comments).
- **Finding (MEDIUM)**: behaviour added outside the task plan — possible scope creep that should be either a new task or removed.

#### G. Test Gap

- For each new function/method/branch added in the diff, look for a test that names it or exercises a path through it.
- **Finding (MEDIUM)**: new code paths with no corresponding test.

### 7. Severity Heuristic

- **CRITICAL**: constitution MUST violation, FR with zero coverage that blocks core functionality, missing test for security-critical path (audit, policy, transport).
- **HIGH**: contract token mismatch, undocumented `pub` surface, SC with no verification.
- **MEDIUM**: scope drift, missing test for non-critical path, contract symbol present but `pub(crate)` instead of `pub`.
- **LOW**: stylistic deviation from contract examples, missing-but-non-load-bearing detail.

### 8. Build the Report

Use this exact Markdown structure so re-runs are diffable and re-postable:

```markdown
<!-- speckit-argos-pr-review-marker -->
## /speckit-argos-pr-review report

**Active feature**: `<FEATURE_DIR>` ([spec](.../spec.md))
**Reviewed PR**: #<PR>
**Base ↔ head**: `<base-sha>` ↔ `<head-sha>` (`<N> files changed`)
**Mode**: read-only — no files modified, no fixes applied

### Findings

| ID  | Category               | Severity | Location              | Summary                                | Recommendation                                              |
|-----|------------------------|----------|-----------------------|----------------------------------------|-------------------------------------------------------------|
| F1  | Contract conformance   | HIGH     | src/cli/mod.rs        | --foo flag in contract not in code     | Add flag or amend contract                                  |
| ... | ...                    | ...      | ...                   | ...                                    | ...                                                         |

### Coverage matrix

| Spec ID  | Has implementing task? | Tasks complete? | Code present? | Test/evidence? |
|----------|------------------------|-----------------|---------------|----------------|
| FR-001   | T014                   | yes             | ✅            | tests/...      |
| ...

### Constitution alignment

(list any principle violations; otherwise: "All principles satisfied.")

### Public API surface diff

(list new/modified `pub` symbols and whether they're documented in contracts/)

### Metrics

- Total findings: N (CRITICAL: a, HIGH: b, MEDIUM: c, LOW: d)
- FR coverage: x/y
- SC verification: x/y
- Constitution violations: 0/N

### Next actions

- If CRITICAL: block merge; resolve before re-running.
- If only LOW/MEDIUM: merge at your discretion; consider opening follow-up tasks.
- Suggested commands: `/speckit-analyze` to confirm artifact consistency, `/ultrareview` for a broader code-quality pass.

---
*Generated by `.claude/skills/speckit-argos-pr-review` — re-run to refresh.*
```

### 9. Post as PR/MR Comment

Unless `--no-comment` was passed (or no provider was detected in Step 2), post using the provider-specific command:

| Provider  | Comment command                                              |
|-----------|--------------------------------------------------------------|
| github    | `gh pr comment <NUMBER> --body "$REPORT"`                   |
| gitlab    | `glab mr note <NUMBER> --message "$REPORT"`                 |
| bitbucket | `bb pr comment <NUMBER> --message "$REPORT"`                |
| none      | print to stdout only                                         |

The leading HTML comment marker `<!-- speckit-argos-pr-review-marker -->` makes future versions of this skill identify and (optionally, in a later iteration) edit the existing comment instead of stacking duplicates. v1 always posts a new comment.

If `--no-comment` was passed, print the report to stdout and exit.

### 10. Post-Execution Checks

**Check for extension hooks (after pr-review)**:

- Check if `.specify/extensions.yml` exists in the project root.
- If it exists, read it and look for entries under the `hooks.after_pr_review` key.
- Same parsing rules as the pre-hook check above.

## Operating Principles

### Read-only

This skill **must not** modify any file in the working tree. The only mutation it produces is a single PR/MR comment via the detected provider. If you find yourself reaching for `Write` or `Edit`, stop — you're outside this skill's scope.

### Spec-grounded

Every finding must reference the spec/plan/tasks/contract/constitution element it's grounded in. A "this looks weird" finding without a spec anchor is not a spec-grounded review — defer those to `/ultrareview`.

### Deterministic IDs

Findings are prefixed by category initial (`F`, `B`, `C`, etc.) plus a sequence number. Re-runs without changes should produce the same IDs and counts so the diff between consecutive reports is meaningful.

### Conservative on severity

When in doubt, prefer the lower severity. CRITICAL is reserved for things that must block merge; over-using it trains the operator to ignore the badge.

### Token efficiency

Cap the findings table at 50 rows; aggregate the rest. Don't dump raw artifacts into the report — quote only the specific lines or symbols at issue.