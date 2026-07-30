#!/usr/bin/env bash
# capture-edit.sh — git post-commit hook for apprenticeship corpus capture.
#
# Wires the `actual_diff` field of POST /v1/shadow (§7C step 3).
# Run after every commit; no-op when no brief is pending.
#
# Install (once per git repo):
#   ln -sf "$(git rev-parse --show-toplevel)/service-slm/scripts/capture-edit.sh" \
#          "$(git rev-parse --git-dir)/hooks/post-commit"
#
# The agent session writes the current brief_id before committing:
#   echo "$BRIEF_ID" > "$(git rev-parse --git-dir)/foundry-brief-id"
# and clears it at session end:
#   rm -f "$(git rev-parse --git-dir)/foundry-brief-id"
#
# Env vars (all optional; defaults suit the workspace VM):
#   FOUNDRY_DOORMAN_ENDPOINT  — default http://127.0.0.1:9080
#   FOUNDRY_MODULE_ID         — default foundry
#   FOUNDRY_SENIOR_IDENTITY   — default: git commit author email

set -euo pipefail

DOORMAN="${FOUNDRY_DOORMAN_ENDPOINT:-http://127.0.0.1:9080}"
MODULE_ID="${FOUNDRY_MODULE_ID:-foundry}"
GIT_DIR="$(git rev-parse --git-dir)"
BRIEF_ID_FILE="${GIT_DIR}/foundry-brief-id"

# No-op if no pending brief.
[[ -f "$BRIEF_ID_FILE" ]] || exit 0
BRIEF_ID="$(cat "$BRIEF_ID_FILE" | tr -d '[:space:]')"
[[ -n "$BRIEF_ID" ]] || exit 0

# Capture diff (stat header + unified 3-line context, bounded at 64 KiB).
ACTUAL_DIFF="$(git show --stat HEAD && echo '---' && git show HEAD --unified=3 2>/dev/null)"
ACTUAL_DIFF="${ACTUAL_DIFF:0:65536}"

COMMIT_MSG="$(git log -1 --pretty=%B)"
SENIOR_IDENTITY="${FOUNDRY_SENIOR_IDENTITY:-$(git log -1 --pretty=%ae)}"
CREATED="$(date -u +%Y-%m-%dT%H:%M:%SZ)"

if ! command -v jq &>/dev/null; then
    echo "[capture-edit] jq not found — install jq to enable corpus capture" >&2
    exit 0
fi

PAYLOAD="$(jq -n \
    --arg brief_id   "$BRIEF_ID" \
    --arg created    "$CREATED" \
    --arg identity   "$SENIOR_IDENTITY" \
    --arg module_id  "$MODULE_ID" \
    --arg msg        "$COMMIT_MSG" \
    --arg diff       "$ACTUAL_DIFF" \
    '{
        brief: {
            brief_id:          $brief_id,
            created:           $created,
            senior_role:       "task",
            senior_identity:   $identity,
            task_type:         "code-edit",
            scope:             {module_id: $module_id},
            acceptance_test:   $msg,
            doctrine_citations: [],
            shadow:            true,
            body:              $msg
        },
        actual_diff: $diff
    }')"

HTTP_STATUS="$(curl -s -o /dev/null -w "%{http_code}" \
    -X POST "${DOORMAN}/v1/shadow" \
    -H "Content-Type: application/json" \
    -H "X-Foundry-Module-ID: ${MODULE_ID}" \
    -d "$PAYLOAD" \
    --max-time 5 2>/dev/null)" || HTTP_STATUS="0"

if [[ "$HTTP_STATUS" == "202" ]]; then
    echo "[capture-edit] shadow enqueued (brief_id=${BRIEF_ID})"
else
    echo "[capture-edit] shadow POST ${HTTP_STATUS} — non-fatal; corpus tuple skipped" >&2
fi
