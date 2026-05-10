---
description: "Finding-anchored remediation: consumes findings from pr-review or analyze, applies one fix per finding (one commit per finding), then re-runs pr-review to confirm resolution."
---

## User Input

```text
$ARGUMENTS
```

Parse the following from user input:

- **First positional argument** (optional): PR/MR number to auto-fetch findings from.
- **`--severity <level>`** (optional): only act on findings at or above this severity. Default: `MEDIUM` (acts on CRITICAL, HIGH, MEDIUM; skips LOW).
- **`--dry-run`** (optional): plan every fix and print the plan, but do not write any files or create any commits. Useful for reviewing what would change before authorising.
- **`--no-comment`** (optional): skip posting the post-remediation pr-review report as a PR/MR comment.
- **Pasted findings** (optional): if no PR number is given and no open PR is detected, the operator may paste a findings block directly into the prompt. Accept any text that contains finding rows in the format `| ID | Category | Severity | Location | Summary | Recommendation |`.

## Pre-Execution Checks

**Check for extension hooks (before remediate)**:

- Check if `.specify/extensions.yml` exists in the project root.
- If it exists, read entries under the `hooks.before_remediate` key.
- Same parsing rules as other speckit skills (filter disabled hooks, skip condition evaluation, replace dots with hyphens in slash commands).
- For each executable hook, output the standard optional/mandatory prompt block.

## Goal

Close the loop between diagnosis and repair. `/speckit-argos-pr-review` and `/speckit-analyze` surface findings but never fix them. `/speckit-argos-remediate` takes those findings as its work queue and applies targeted fixes, one commit per finding, then re-runs the review to confirm resolution.

**Finding-anchored scope**: every file this skill touches must resolve a finding. The fix location may differ from the reported location — a finding in a contract document may be best resolved by changing the code, or vice versa. What matters is that the finding is closed, not that the change is in the reported file. There is no free-roaming outside the finding work queue.

**Spec-grounded constraint**: no fix may introduce a new violation of the spec, contracts, or constitution. Before applying each fix, verify it against the loaded artifacts.

## Operating Constraints

**Write-capable.** This skill modifies files in the working tree and creates git commits. Every modification is traced to a finding ID in the commit message.

**Active feature only.** Resolves the active feature context from `.specify/scripts/bash/check-prerequisites.sh`. Does not touch unrelated specs or code.

**Constitution authority.** A proposed fix that would violate a constitution MUST clause is rejected. Flag the finding for human review instead of applying a non-compliant fix.

**CRITICAL pause.** Before applying a fix for a CRITICAL finding, state the proposed change explicitly and ask for operator confirmation unless `--dry-run` is set. CRITICAL findings often require architectural judgment, not just code edits.

**Idempotent on re-run.** If a finding is already resolved (the reported symptom is no longer present in the code), skip it and note it as pre-resolved in the summary.

## Execution Steps

### 1. Initialize Context

Run `.specify/scripts/bash/check-prerequisites.sh --json --require-tasks --include-tasks 2>/dev/null` from repo root and attempt to parse JSON for `FEATURE_DIR`. Redirect stderr so internal branch-naming diagnostics never surface to the user.

**If the script succeeds**, derive full artifact paths:

- `SPEC` = `FEATURE_DIR/spec.md`
- `PLAN` = `FEATURE_DIR/plan.md`
- `TASKS` = `FEATURE_DIR/tasks.md`
- `CONTRACTS_DIR` = `FEATURE_DIR/contracts/`
- `CONSTITUTION` = `.specify/memory/constitution.md`

Spec conformance checks in step 6 run in full.

**If the script fails due to branch naming** (exit code non-zero, output does not contain valid JSON), enter **reduced mode** silently — do not print any notice to the user:

- Load `CONSTITUTION` = `.specify/memory/constitution.md` only.
- Spec artifact loading is skipped; constitution guards still apply to every proposed fix.
- Finding sourcing (step 3) and all fix application steps work normally — spec-grounded constraint checks are limited to the constitution only.
- **Auto-create a working branch**: unless `--dry-run` is set, run `git checkout -b feat/remediate-$(date +%Y%m%d%H%M%S)` so that all remediation commits land on a dedicated branch rather than directly on the current branch. This is transparent — no confirmation needed; mention the branch name once in the opening line of step 4's work-queue output so the operator knows where commits will go.

**If `.specify/` is missing entirely**, abort with: *"No .specify/ directory found. Is this a speckit project?"*

### 2. Detect Provider

Same detection logic as `/speckit-argos-pr-review`:

| Priority | Provider  | Detection command  |
|----------|-----------|--------------------|
| 1        | GitHub    | `gh auth status`   |
| 2        | GitLab    | `glab auth status` |
| 3        | Bitbucket | `bb --version`     |

Store as `PROVIDER`. Used to fetch PR comments and optionally post the post-remediation report. If no provider is detected and `--no-comment` is not set, warn and continue in `--no-comment` mode.

### 3. Resolve Findings

Try sources in order — use the first that yields a parseable findings table:

**Source A — PR/MR comment (auto-fetch)**:

If a PR/MR number was passed, or can be auto-detected from the current branch, fetch the most recent comment containing `<!-- speckit-argos-pr-review-marker -->`:

| Provider  | Command                                                                                   |
|-----------|-------------------------------------------------------------------------------------------|
| github    | `gh pr view <N> --comments --json comments --jq '[.comments[] \| select(.body \| contains("speckit-argos-pr-review-marker"))] \| last \| .body'` |
| gitlab    | `glab mr note list <N> --output json \| jq '[.[] \| select(.body \| contains("speckit-argos-pr-review-marker"))] \| last \| .body'` |
| bitbucket | `bb pr comment list <N> --json \| jq '[.[] \| select(.body \| contains("speckit-argos-pr-review-marker"))] \| last \| .body'` |

Parse the findings table from the fetched comment body.

**Source B — Pasted inline**:

If no PR/MR is available or the fetch yields no marker comment, look for a findings table in the user's prompt input. Accept any Markdown table with columns `ID`, `Category`, `Severity`, `Location`, `Summary`, `Recommendation`.

**Source C — Run pr-review first**:

If neither source A nor B yields findings, offer to run `/speckit-argos-pr-review` to generate a fresh report, then use that output as the findings source.

If no findings can be resolved from any source, abort with a clear message.

### 4. Parse and Filter Findings

From the findings table, extract each row as a structured finding:

```
{ id, category, severity, location, summary, recommendation }
```

Apply the `--severity` filter: discard findings below the threshold. Default threshold is MEDIUM (keep CRITICAL, HIGH, MEDIUM).

Sort the work queue by severity descending: CRITICAL → HIGH → MEDIUM → LOW.

Print the work queue to stdout so the operator sees what will be acted on before any file is touched.

If `--dry-run` is set, continue through planning (step 6) for each finding but skip steps 7–8 (apply + commit). Print the plan and exit after step 9.

### 5. Pre-Flight Check

For each finding in the work queue, check whether it is already resolved:

- Load the reported file (if it still exists) and grep for the symptom described in the summary.
- If the symptom is absent, mark the finding as **pre-resolved** and remove it from the work queue.

Report any pre-resolved findings at the start of the summary so the operator knows.

### 6. Plan Each Fix

For each remaining finding, reason about the best fix:

1. **Root cause analysis**: understand why the finding exists — is the reported location the cause, or a symptom of something upstream?
2. **Fix location decision**: choose where the fix belongs. Examples:
   - A contract document out of sync with code → update the contract *or* update the code, whichever is the source of truth.
   - An undocumented `pub` symbol → add it to the contract *or* make it `pub(crate)`, depending on intent.
   - A missing test → add the test in the appropriate test file.
   - Scope drift → either add a task to `tasks.md` retroactively or remove/refactor the drifted code.
3. **Spec conformance check**: verify the planned fix does not violate any FR, SC, contract, or constitution MUST clause. If it does, reject the fix and flag the finding for human review.
4. **Plan output**: produce a one-paragraph plan for each finding before touching any files.

For CRITICAL findings: state the plan explicitly and pause for operator confirmation before proceeding to step 7.

### 7. Apply Each Fix

For each finding (in severity order):

1. Apply the planned fix to the relevant file(s).
2. If the project has a build or test step relevant to the fix (e.g. `cargo check`, `cargo test <module>`), run it to confirm the fix compiles and passes.
3. If the build/test fails, diagnose and revise the fix before committing.

### 8. Commit Each Fix

After each finding is applied and verified:

```sh
git add <changed files>
git commit -m "fix(<FINDING_ID>): <one-line summary of what was fixed>"
```

The finding ID in the commit message creates a permanent, traceable link between the finding report and the remediation commit. Do not batch multiple findings into one commit.

### 9. Re-run `/speckit-argos-pr-review`

After all fixes are committed, invoke `/speckit-argos-pr-review` (passing the same PR/MR number if available) to produce a fresh report. The re-run:

- Shows which previously-reported findings are now absent (resolved).
- Surfaces any regressions introduced by the fixes (new findings not present in the original report).
- Is posted as a new PR/MR comment (unless `--no-comment`).

The re-run is the confirmation step. A finding is considered closed only when it no longer appears in the fresh report.

### 10. Produce Remediation Summary

Print a summary to stdout in this format:

```markdown
## /speckit-argos-remediate summary

**Findings acted on**: N
**Pre-resolved (skipped)**: N
**Fixed and committed**: N
**Failed / flagged for human review**: N

### Fixed

| Finding ID | Commit | Resolution |
|------------|--------|------------|
| B1 | abc1234 | Added `pub mod types` re-exporting `MessageType`, `DecisionLabel`, `PolicyAction` to `lib.rs` |
| ...

### Flagged for human review

| Finding ID | Reason |
|------------|--------|
| C1 | Requires architectural decision — see inline comment |

### Regressions detected

(none — or list new findings from the re-run report)

---
*Re-run `/speckit-argos-pr-review` to refresh the full report.*
```

### 11. Post-Execution Checks

**Check for extension hooks (after remediate)**:

- Check if `.specify/extensions.yml` exists in the project root.
- Read entries under `hooks.after_remediate`.
- Same parsing rules as the pre-hook check.

## Operating Principles

### Finding-anchored, not file-anchored

Every file this skill modifies must resolve a finding. The fix may be upstream or downstream of the reported location — that is expected and correct. What is not permitted is modifying files for reasons unrelated to any finding in the work queue.

### One commit per finding

This makes the remediation history legible and revertable at the finding level. `git revert <commit>` undoes exactly one fix without disturbing others.

### Spec-first repair

The spec and contracts are the source of truth. If a finding represents a mismatch between code and contract, determine which is correct (the spec/contract or the implementation) before deciding what to fix. Do not blindly update contracts to match code — that would silently delete requirements.

### Conservative on CRITICAL

CRITICAL findings often represent architectural decisions, not just code edits. Pause and confirm before applying a CRITICAL fix. A wrong fix on a CRITICAL finding can introduce a security regression that is harder to detect than the original gap.

### Transparent planning

Always show the plan before applying fixes. The operator should be able to read the plan, disagree, and stop the skill before any file is changed. `--dry-run` is the formal mechanism for this, but even in normal mode the plan is printed before application begins.
