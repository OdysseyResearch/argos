# Contract: Policy File Format

**Version**: `"0.1"` | **Format**: TOML

---

## Structure

```toml
[meta]
version = "0.1"                          # required — validated at load time
agent = "code-review-agent"              # optional — human label for this policy's intended agent
description = "..."                      # optional
session_tags = ["code-review", "ci"]    # optional — reserved for M8 compliance mapping

[[rules]]
# one or more rules — evaluated top-to-bottom, first match wins
```

---

## `[meta]` section

| Field          | Type            | Required | Description                                                                                                                                                |
| -------------- | --------------- | -------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `version`      | String          | Yes      | Policy DSL version. Must be `"0.1"`. Any other value is a hard startup error.                                                                              |
| `agent`        | String          | No       | Human-readable label for the agent this policy governs. Informational only — does not affect audit log `agent` field (that comes from `--agent` CLI flag). |
| `description`  | String          | No       | Free-text description of this policy's purpose.                                                                                                            |
| `session_tags` | Array of String | No       | Tag array reserved for compliance template mapping (M8). Defaults to `[]`.                                                                                 |

---

## `[[rules]]` entries

Each rule entry matches either a tool call or a resource access — not both.

| Field         | Type            | Required | Description                                                                                   |
| ------------- | --------------- | -------- | --------------------------------------------------------------------------------------------- |
| `tool`        | String          | Cond.    | Tool name to match. Exact match or `"*"` catch-all. Mutually exclusive with `resource`.       |
| `resource`    | String          | Cond.    | Resource URI glob pattern. e.g. `"file:///workspace/src/**"`. Mutually exclusive with `tool`. |
| `action`      | String          | Yes      | One of `"allow"`, `"block"`, `"redact"`.                                                      |
| `constraints` | Table           | No       | Key-value constraints on arguments. v0.1 supports `path_prefix` only.                         |
| `reason`      | String          | No       | Human-readable reason logged in audit entries and returned to the blocked client.             |
| `tags`        | Array of String | Yes      | Compliance tag array. Must be present; `[]` is acceptable.                                    |
| `redact`      | Array of String | Cond.    | Argument field names to strip before forwarding. Required when `action = "redact"`.           |

Exactly one of `tool` or `resource` must be present. Providing both or neither is a hard startup
error with the offending rule index identified.

---

## Rule Evaluation

- **First-match-wins**: rules are evaluated top-to-bottom; the first matching rule determines
  the decision.
- **Deny by default**: if no rule matches, the request is blocked. No explicit catch-all rule
  is required (though adding one explicitly is recommended for documentation clarity).
- **Tool matching**: `tool = "read_file"` matches exactly. `tool = "*"` matches any tool name.
  Tool name globs (e.g. `tool = "read_*"`) are deferred to M2.
- **Resource matching**: `resource` field uses glob syntax via the `globset` crate.
  `resource = "*"` matches any resource URI. `resource = "file:///workspace/**"` matches all
  URIs under that path.
- **Constraints**: `path_prefix` constraint checks that the argument named `path` has the
  specified string prefix. Other constraint keys are rejected at load time in v0.1.

---

## Example: Developer safety policy

```toml
[meta]
version = "0.1"
description = "Read-only workspace access. No writes, no shell, no network."

[[rules]]
tool = "read_file"
action = "allow"
constraints = { path_prefix = "/workspace/myproject" }
tags = []

[[rules]]
tool = "list_directory"
action = "allow"
constraints = { path_prefix = "/workspace/myproject" }
tags = []

[[rules]]
resource = "file:///workspace/myproject/**"
action = "allow"
tags = []

[[rules]]
tool = "*"
action = "block"
reason = "Default deny — tool not in policy"
tags = []
```

---

## Example: Redaction rule

```toml
[[rules]]
tool = "search_web"
action = "redact"
redact = ["api_key", "token"]
reason = "Strip credentials before forwarding to upstream"
tags = ["credential-protection"]
```

---

## Validation errors

| Error                       | Condition                                                 |
| --------------------------- | --------------------------------------------------------- |
| `UnsupportedVersion`        | `meta.version` is not `"0.1"`                             |
| `MissingVersion`            | `[meta]` section present but `version` field absent       |
| `InvalidRule`               | Both `tool` and `resource` set, or neither set            |
| `UnrecognisedAction`        | `action` is not `allow`, `block`, or `redact`             |
| `MissingRedactFields`       | `action = "redact"` but `redact` array is absent or empty |
| `UnrecognisedConstraintKey` | Constraint key other than `path_prefix` used in v0.1      |
