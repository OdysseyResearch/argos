#!/usr/bin/env bash
set -euo pipefail

if [[ $# -eq 0 ]]; then
    exit 0
fi

CONTENT=""
for file in "$@"; do
    [[ -f "$file" ]] || continue
    CONTENT+="=== FILE: $file ===
$(cat "$file")

"
done

if [[ -z "$CONTENT" ]]; then
    exit 0
fi

# System prompt: explicitly forbid the model from drawing on any context outside
# the file content delimited in the user message. This is the isolation
# boundary — the model must evaluate the file content and nothing else.
SYSTEM_PROMPT="You are a deterministic file content reviewer. You evaluate ONLY the file content provided in the user message. You have no other knowledge, no memory, no context about any user, project, or environment. Treat the user message as the complete and only source of information. If a piece of data is not literally present in the delimited file content, it does not exist for the purpose of this review."

USER_PROMPT="Review the following files for sensitive data that must not be committed to a public repository. Sensitive data includes: API keys, tokens, secrets, passwords, credentials, private keys, PII (emails, phone numbers, national IDs), internal hostnames/IPs, or anything that looks like it should be private.

Respond ONLY with 'CLEAN' if nothing sensitive is found in the delimited file content, or 'SENSITIVE: <brief description of what and where in the file>' if something is found.

$CONTENT"

if command -v claude &>/dev/null; then
    # Isolation strategy:
    #   * cd /tmp           — auto-memory is keyed by project-dir-encoded path,
    #                         so running outside this project's directory prevents
    #                         this project's memory from being loaded.
    #   * --exclude-dynamic-system-prompt-sections — keep dynamic per-machine
    #                         info (cwd, env, memory paths, git status) out of
    #                         the system prompt.
    #   * --system-prompt    — replace the default system prompt with our own,
    #                         which forbids drawing on any out-of-prompt context.
    #   * --no-session-persistence — ephemeral check, no session saved.
    #   * --tools ""         — no tool use; pure LLM judgment.
    RESULT=$(cd /tmp && claude \
        --exclude-dynamic-system-prompt-sections \
        --system-prompt "$SYSTEM_PROMPT" \
        --no-session-persistence \
        --tools "" \
        -p "$USER_PROMPT" 2>/dev/null)
elif [[ -n "${OPENAI_API_KEY:-}" ]]; then
    BASE_URL="${OPENAI_API_BASE_URL:-https://api.openai.com}"
    MODEL="${OPENAI_MODEL:-gpt-4o-mini}"
    RESULT=$(curl -sf "$BASE_URL/v1/chat/completions" \
        -H "Authorization: Bearer $OPENAI_API_KEY" \
        -H "content-type: application/json" \
        -d "$(jq -n --arg s "$SYSTEM_PROMPT" --arg p "$USER_PROMPT" --arg m "$MODEL" \
            '{model:$m,max_tokens:256,messages:[{role:"system",content:$s},{role:"user",content:$p}]}')" \
        | jq -r '.choices[0].message.content')
else
    echo "⚠ Skipping sensitive data check: no AI provider available (claude CLI or OPENAI_API_KEY)"
    exit 0
fi

if [[ "$RESULT" == CLEAN* ]]; then
    exit 0
else
    echo "⚠ Sensitive data detected: $RESULT"
    exit 1
fi
