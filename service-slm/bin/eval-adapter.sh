#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0 OR MIT
#
# bin/eval-adapter.sh — Adapter eval harness (P1-1.3, F12 gate-keeper)
#
# Runs a candidate adapter against the signed holdout set and emits a
# structured result file.  The F12 contract: adapter promotion to
# production is ONLY permitted when this script emits `"promoted": true`.
#
# Usage:
#   FOUNDRY_ROOT=/srv/foundry ./bin/eval-adapter.sh --adapter=coding-lora-2026-05-22
#
# Required env / flags:
#   --adapter=<version>      adapter version label to evaluate
#
# Optional env:
#   FOUNDRY_ROOT             workspace root          (default: /srv/foundry)
#   SLM_DOORMAN_URL          Doorman base URL        (default: http://127.0.0.1:9080)
#   HOLDOUT_FILE             signed holdout JSONL    (default: $FOUNDRY_ROOT/data/training-corpus/eval/holdout-v1.jsonl)
#   MAX_REGRESSION           max tolerated regression 0–1 vs baseline  (default: 0.05)
#   MODULE_ID                Doorman module_id header (default: pointsav)
#
# Output:
#   data/training-corpus/eval/results/<adapter_version>.json
#   JSON fields: adapter_version, total, passed, pass_rate,
#                baseline_pass_rate, regression, promoted

set -euo pipefail

FOUNDRY_ROOT="${FOUNDRY_ROOT:-/srv/foundry}"
SLM_DOORMAN_URL="${SLM_DOORMAN_URL:-http://127.0.0.1:9080}"
MAX_REGRESSION="${MAX_REGRESSION:-0.05}"
MODULE_ID="${MODULE_ID:-pointsav}"
ADAPTER=""

for arg in "$@"; do
    case "${arg}" in
        --adapter=*) ADAPTER="${arg#--adapter=}" ;;
        *) echo "Unknown argument: ${arg}" >&2; exit 1 ;;
    esac
done

if [[ -z "${ADAPTER}" ]]; then
    echo "ERROR: --adapter=<version> is required." >&2; exit 1
fi

if ! command -v jq &>/dev/null; then
    echo "ERROR: jq is required." >&2; exit 1
fi

HOLDOUT_FILE="${HOLDOUT_FILE:-${FOUNDRY_ROOT}/data/training-corpus/eval/holdout-v1.jsonl}"
if [[ ! -f "${HOLDOUT_FILE}" ]]; then
    echo "ERROR: Holdout set not found: ${HOLDOUT_FILE}" >&2
    echo "       Run scripts/eval-prepare.sh + operator signing first." >&2
    exit 2
fi

RESULTS_DIR="${FOUNDRY_ROOT}/data/training-corpus/eval/results"
mkdir -p "${RESULTS_DIR}"
OUT_FILE="${RESULTS_DIR}/${ADAPTER}.json"

echo "=== Adapter Eval Harness (P1-1.3) ==="
echo "Adapter  : ${ADAPTER}"
echo "Holdout  : ${HOLDOUT_FILE}"
echo "Doorman  : ${SLM_DOORMAN_URL}"
echo ""

# ── Score one pass through the holdout set ───────────────────────────────
# Returns: pass count out of total
score_pass() {
    local adapter_label="$1"
    local total=0 passed=0

    while IFS= read -r line; do
        [[ -z "${line}" ]] && continue
        brief_body=$(jq -r '.brief.body // .prompt // ""' <<< "${line}")
        expected=$(jq -r '.expected_diff // .actual_diff // ""' <<< "${line}")
        [[ -z "${brief_body}" || -z "${expected}" ]] && continue

        total=$((total + 1))

        # POST to Doorman /v1/complete with adapter version header.
        response=$(curl -sf -X POST "${SLM_DOORMAN_URL}/v1/complete" \
            -H "Content-Type: application/json" \
            -H "X-Foundry-Module-ID: ${MODULE_ID}" \
            -d "$(jq -nc --arg body "${brief_body}" --arg adapter "${adapter_label}" \
                    '{prompt: $body, adapter_version: $adapter, max_tokens: 512}')" \
            2>/dev/null || echo '{}')

        actual=$(jq -r '.content // ""' <<< "${response}")

        # Score: normalised edit-distance between expected and actual.
        # python3 -c via difflib is available everywhere; no extra deps.
        ratio=$(python3 - "${expected}" "${actual}" <<'EOF'
import sys, difflib
a = sys.argv[1]; b = sys.argv[2]
print(f"{difflib.SequenceMatcher(None, a, b).ratio():.4f}")
EOF
        )
        # Pass threshold: similarity ≥ 0.70
        result=$(python3 -c "print('pass' if float('${ratio}') >= 0.70 else 'fail')")
        if [[ "${result}" == "pass" ]]; then
            passed=$((passed + 1))
        fi
    done < "${HOLDOUT_FILE}"

    echo "${passed} ${total}"
}

echo "Running candidate adapter (${ADAPTER})..."
read -r cand_passed cand_total < <(score_pass "${ADAPTER}")

if [[ "${cand_total}" -eq 0 ]]; then
    echo "ERROR: No scoreable tuples found in holdout set." >&2; exit 3
fi

cand_rate=$(python3 -c "print(f'{${cand_passed}/${cand_total}:.4f}')")

echo "Running baseline (no adapter)..."
read -r base_passed base_total < <(score_pass "")
base_rate=$(python3 -c "print(f'{${base_passed}/${base_total}:.4f}')")

regression=$(python3 -c "print(f'{max(0.0, float(\"${base_rate}\") - float(\"${cand_rate}\")):.4f}')")
promoted=$(python3 -c "print('true' if float('${regression}') <= ${MAX_REGRESSION} else 'false')")

jq -n \
    --arg adapter "${ADAPTER}" \
    --arg holdout "${HOLDOUT_FILE}" \
    --argjson total "${cand_total}" \
    --argjson passed "${cand_passed}" \
    --arg pass_rate "${cand_rate}" \
    --arg baseline_pass_rate "${base_rate}" \
    --arg regression "${regression}" \
    --argjson promoted "${promoted}" \
    --arg ts "$(date -u +%Y-%m-%dT%H:%M:%SZ)" \
    '{adapter_version: $adapter, holdout: $holdout, total: $total,
      passed: $passed, pass_rate: ($pass_rate|tonumber),
      baseline_pass_rate: ($baseline_pass_rate|tonumber),
      regression: ($regression|tonumber),
      promoted: $promoted, evaluated_at: $ts}' \
    > "${OUT_FILE}"

echo ""
echo "Results: ${OUT_FILE}"
jq '.' "${OUT_FILE}"
echo ""
if [[ "${promoted}" == "true" ]]; then
    echo "F12 PASS — adapter ${ADAPTER} PROMOTED (regression=${regression} ≤ ${MAX_REGRESSION})"
else
    echo "F12 FAIL — adapter ${ADAPTER} NOT promoted (regression=${regression} > ${MAX_REGRESSION})"
    exit 4
fi
