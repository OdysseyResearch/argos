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

PROMPT="Review the following files for sensitive data that must not be committed to a public repository. Sensitive data includes: API keys, tokens, secrets, passwords, credentials, private keys, PII (emails, phone numbers, national IDs), internal hostnames/IPs, or anything that looks like it should be private. Respond ONLY with 'CLEAN' if nothing sensitive is found, or 'SENSITIVE: <brief description of what and where>' if something is found.

$CONTENT"

if command -v claude &>/dev/null; then
    RESULT=$(claude -p "$PROMPT" 2>/dev/null)
elif [[ -n "${OPENAI_API_KEY:-}" ]]; then
    BASE_URL="${OPENAI_API_BASE_URL:-https://api.openai.com}"
    MODEL="${OPENAI_MODEL:-gpt-4o-mini}"
    RESULT=$(curl -sf "$BASE_URL/v1/chat/completions" \
        -H "Authorization: Bearer $OPENAI_API_KEY" \
        -H "content-type: application/json" \
        -d "$(jq -n --arg p "$PROMPT" --arg m "$MODEL" '{model:$m,max_tokens:256,messages:[{role:"user",content:$p}]}')" \
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
