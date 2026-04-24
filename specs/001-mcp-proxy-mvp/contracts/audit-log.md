# Contract: Audit Log Format

**Version**: 0.1 | **Format**: JSONL (one JSON object per line, UTF-8)

---

## Entry types

Two entry types exist in the schema. In v0.1 only `decision` entries are emitted.

| `entry_type`      | Emitted in v0.1 | Description                                                 |
| ----------------- | --------------- | ----------------------------------------------------------- |
| `decision`        | Yes             | Policy evaluation result for a tool call or resource access |
| `rotation_marker` | No              | Log rotation boundary marker (schema reserved for M6)       |

Decision entries do not include an explicit `entry_type` field — their structure is
distinguished by the presence of `decision`. `rotation_marker` entries include
`"entry_type": "rotation_marker"`.

---

## Decision entry schema

```json
{
  "timestamp":          "2026-04-24T10:31:05.123456Z",
  "sequence":           1,
  "prev_hash":          "sha256:0000000000000000000000000000000000000000000000000000000000000000",
  "entry_hash":         "sha256:a3f1...",
  "session_id":         "01950c2a-7e3f-7000-8000-000000000042",
  "message_type":       "tools/call",
  "decision":           "blocked",
  "tool_or_resource":   "write_file",
  "arguments":          { "path": "/etc/passwd", "content": "..." },
  "arguments_truncated": false,
  "policy_rule_matched": "write_file:block[2]",
  "reason":             "Write access not permitted for this agent",
  "agent":              "claude-code",
  "policy_version":     "0.1",
  "org_id":             null,
  "tenant_id":          null
}
```

`dry_run: true` is present (and `true`) only when `--dry-run` is active and the decision is
`blocked`. It is omitted from all other entries.

---

## Field reference

| Field                 | Type         | Description                                                                             |
| --------------------- | ------------ | --------------------------------------------------------------------------------------- |
| `timestamp`           | String       | RFC 3339 with microsecond precision, UTC (`Z` suffix).                                  |
| `sequence`            | u64          | Monotonically increasing, 1-based, per session.                                         |
| `prev_hash`           | String       | `"sha256:<64 hex chars>"`. For the first entry: 64 hex zeros.                           |
| `entry_hash`          | String       | `"sha256:<64 hex chars>"`. SHA-256 of the canonical entry bytes (see below).            |
| `session_id`          | String       | UUID v4, generated once per proxy invocation.                                           |
| `message_type`        | String       | One of `"tools/call"`, `"resources/read"`, `"resources/list"`, `"resources/subscribe"`. |
| `decision`            | String       | One of `"allowed"`, `"blocked"`, `"redacted"`.                                          |
| `tool_or_resource`    | String       | Tool name (for `tools/call`) or resource URI (for `resources/*`).                       |
| `arguments`           | Object       | Tool call arguments or resource request params, up to `--max-arg-bytes`.                |
| `arguments_truncated` | bool         | `true` if arguments were truncated to `--max-arg-bytes`.                                |
| `policy_rule_matched` | String\|null | `"<tool_or_resource>:<action>[<rule_index>]"` or `null` for deny-by-default.            |
| `reason`              | String\|null | Rule's `reason` field, or `"deny by default"` for implicit blocks, or `null`.           |
| `agent`               | String       | Value of `--agent` flag, default `"unknown"`.                                           |
| `policy_version`      | String       | Value of `meta.version` from the loaded policy file.                                    |
| `org_id`              | String\|null | Always `null` in v0.1. Reserved for M7 multi-tenancy.                                   |
| `tenant_id`           | String\|null | Always `null` in v0.1. Reserved for M7 multi-tenancy.                                   |
| `dry_run`             | bool         | Omitted unless `--dry-run` is active and decision is `blocked`.                         |

---

## `entry_hash` computation convention

This convention is authoritative. Any verifier must implement exactly this to reproduce hashes.

1. Construct the `AuditEntry` with all fields populated, and `entry_hash` set to `""` (empty
   string — not `null`, not absent).
2. Serialize to compact JSON (no extra whitespace) using field order as defined above.
3. Compute `SHA-256` over the UTF-8 bytes of that JSON string.
4. Format as `"sha256:<64 lowercase hex chars>"`.
5. Set `entry_hash` to that value.
6. Write the final JSON (with `entry_hash` populated) as a single line to the JSONL file,
   followed by `\n`.

**The `entry_hash` covers the final entry bytes minus the `entry_hash` field value itself.**
Verifiers must reproduce step 1 (blank `entry_hash`) before hashing.

---

## Hash chain bootstrap

The first entry in a new log file uses:

```
"prev_hash": "sha256:0000000000000000000000000000000000000000000000000000000000000000"
```

(The string `"sha256:"` followed by exactly 64 zeros.)

Each subsequent entry's `prev_hash` is the `entry_hash` of the immediately preceding entry.

---

## Independent verification

The chain can be verified by any tool that can read JSONL and compute SHA-256 — no Argos
tooling required. The algorithm is:

```
for each entry in log:
    recompute_hash = sha256(entry with entry_hash="")
    assert recompute_hash == entry.entry_hash
    assert entry.prev_hash == previous_entry.entry_hash  (or genesis for first)
```

`argos-proxy verify --audit-log <path>` implements this algorithm as a built-in subcommand.

---

## Rotation marker entry schema (v0.1 schema, not emitted)

```json
{
  "entry_type":   "rotation_marker",
  "timestamp":    "2026-04-24T11:00:00.000000Z",
  "sequence":     1024,
  "prev_hash":    "sha256:abc123...",
  "entry_hash":   "sha256:def456...",
  "session_id":   "01950c2a-...",
  "reason":       "scheduled rotation"
}
```

`entry_hash` is computed with the same convention as decision entries (set `entry_hash: ""`
before hashing). Rotation markers participate in the hash chain like any other entry.

---

## Append-only guarantee

The log file is opened with `O_APPEND` (or equivalent). The proxy never seeks, truncates, or
overwrites existing entries. If the file is not writable at startup, the proxy refuses to start.
If a write fails mid-session, the proxy terminates with exit code 2.
