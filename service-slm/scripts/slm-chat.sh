#!/usr/bin/env bash
# slm-chat.sh — proof-of-life chat REPL against the Doorman
# Usage: ./slm-chat.sh [module_id]
# Env:   SLM_DOORMAN_ENDPOINT (default http://127.0.0.1:9080)
#        SERVICE_SLM_MODULE_ID (default foundry-workspace)

set -euo pipefail

DOORMAN="${SLM_DOORMAN_ENDPOINT:-http://127.0.0.1:9080}"
MODULE_ID="${1:-${SERVICE_SLM_MODULE_ID:-foundry-workspace}}"

if ! command -v curl &>/dev/null; then
  echo "Error: curl is required but not found in PATH" >&2
  exit 1
fi
if ! command -v jq &>/dev/null; then
  echo "Error: jq is required but not found in PATH" >&2
  exit 1
fi

echo "================================================================"
echo " service-slm chat — Doorman at $DOORMAN"
echo " module: $MODULE_ID | type 'exit' or Ctrl+D to quit"
echo "================================================================"

history='[{"role":"system","content":"You are the service-slm system administrator assistant for the Totebox Archive. Answer questions about the archive'\''s services, configurations, and operations concisely and accurately."}]'

if [[ -d ".agent" ]]; then
  agent_context="LOCAL AGENT CONTEXT:\\n"
  if [[ -f ".agent/manifest.md" ]]; then
    agent_context+="--- .agent/manifest.md ---\\n$(head -n 100 .agent/manifest.md)\\n"
  fi
  if [[ -f ".agent/inbox.md" ]]; then
    agent_context+="--- .agent/inbox.md ---\\n$(head -n 100 .agent/inbox.md)\\n"
  fi
  history=$(echo "$history" | jq --arg ctx "$agent_context" '.[0].content += "\\n\\n" + $ctx')
fi

while true; do
  printf '> '
  if ! read -r line; then
    echo ""
    break
  fi

  [[ -z "$line" ]] && continue
  [[ "$line" == "exit" || "$line" == "quit" ]] && break

  history=$(echo "$history" | jq --arg msg "$line" '. + [{"role":"user","content":$msg}]')

  body=$(echo "$history" | jq -c '{"model":"local","messages":.,"temperature":0.7,"max_tokens":1024}')

  request_id=$(uuidgen)

  raw=$(curl -s -w "\n%{http_code}" --max-time 120 -X POST \
    -H "Content-Type: application/json" \
    -H "X-Foundry-Module-ID: $MODULE_ID" \
    -H "X-Foundry-Request-ID: $request_id" \
    -d "$body" \
    "$DOORMAN/v1/chat/completions" 2>/dev/null) || {
    echo "[Doorman unavailable — is local-doorman.service running?]"
    continue
  }

  http_status="${raw##*$'\n'}"
  response_body="${raw%$'\n'*}"

  if [[ "$http_status" != 2* ]]; then
    echo "[Error: Doorman returned HTTP $http_status — $response_body]"
    continue
  fi

  assistant_msg=$(echo "$response_body" | jq -r '.content // "Error: no response"')
  echo "$assistant_msg"

  history=$(echo "$history" | jq --arg msg "$assistant_msg" '. + [{"role":"assistant","content":$msg}]')
done

echo "Goodbye."
