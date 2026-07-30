#!/usr/bin/env bash
# canary-run.sh — Run the canary task set against the running Doorman.
#
# Phase 3 (P3-3.2) of learning-loop-master-plan-2026-05-18.md.
#
# Loads `service-slm/data/canary/v1.yaml`, sends each task's prompt to
# the Doorman's /v1/messages endpoint, scores the response against the
# task's `contains` + `not_contains` patterns, and writes results to
# `data/canary/results/<adapter_version>-<date>.json`.
#
# Used by:
# - Operator regression check before promoting a new adapter
# - Weekly nightly-run.sh extension (future) for continuous tracking
#
# Usage:
#   ./canary-run.sh [--endpoint=http://127.0.0.1:9090]
#                   [--adapter-version=coding-lora-2026-05-18]
#                   [--task-set=service-slm/data/canary/v1.yaml]
#                   [--out=data/canary/results/<auto>.json]
#                   [--dry-run] [--strict]
#
# Exit codes:
#   0  All tasks passed (or non-strict mode regardless of result)
#   1  Argument error
#   2  Task set file not found
#   3  Required tool missing (jq, curl, python3)
#   4  Doorman unreachable
#   5  (--strict only) Any task failed below promote threshold, or regression detected

set -euo pipefail

ENDPOINT="${SLM_CANARY_ENDPOINT:-http://127.0.0.1:9090}"
ADAPTER_VERSION="${SLM_CANARY_ADAPTER:-baseline}"
FOUNDRY_ROOT="${FOUNDRY_ROOT:-/srv/foundry}"
ARCHIVE_ROOT="${FOUNDRY_ROOT}/clones/project-intelligence"
TASK_SET="${ARCHIVE_ROOT}/service-slm/data/canary/v1.yaml"
DATE_STAMP="$(date -u +%Y-%m-%dT%H%M%SZ)"
OUT_PATH=""
DRY_RUN=0
STRICT=0

while [[ $# -gt 0 ]]; do
    case "$1" in
        --endpoint=*) ENDPOINT="${1#--endpoint=}"; shift ;;
        --adapter-version=*) ADAPTER_VERSION="${1#--adapter-version=}"; shift ;;
        --task-set=*) TASK_SET="${1#--task-set=}"; shift ;;
        --out=*) OUT_PATH="${1#--out=}"; shift ;;
        --dry-run) DRY_RUN=1; shift ;;
        --strict) STRICT=1; shift ;;
        --help|-h) sed -n '2,30p' "$0"; exit 0 ;;
        *) echo "Unknown argument: $1" >&2; exit 1 ;;
    esac
done

if [[ -z "${OUT_PATH}" ]]; then
    OUT_PATH="${FOUNDRY_ROOT}/data/canary/results/${ADAPTER_VERSION}-${DATE_STAMP}.json"
fi

for tool in jq curl python3; do
    command -v "${tool}" >/dev/null 2>&1 || {
        echo "ERROR: required tool not installed: ${tool}" >&2
        exit 3
    }
done

[[ -f "${TASK_SET}" ]] || {
    echo "ERROR: task set not found: ${TASK_SET}" >&2
    exit 2
}

# Convert YAML task set to JSON once; use jq for all subsequent queries.
TASK_JSON="$(python3 - "${TASK_SET}" <<'PYEOF'
import sys, json, datetime
try:
    import yaml
except ImportError:
    print("ERROR: python3 yaml module not available (pip install pyyaml)", file=sys.stderr)
    sys.exit(3)
class _Enc(json.JSONEncoder):
    def default(self, o):
        if isinstance(o, (datetime.date, datetime.datetime)):
            return o.isoformat()
        return super().default(o)
with open(sys.argv[1]) as f:
    print(json.dumps(yaml.safe_load(f), cls=_Enc))
PYEOF
)"

TASK_COUNT="$(echo "${TASK_JSON}" | jq '.tasks | length')"
PROMOTE_THRESHOLD="$(echo "${TASK_JSON}" | jq '.scoring.promote_threshold')"
FLAG_THRESHOLD="$(echo "${TASK_JSON}" | jq '.scoring.flag_threshold')"
REGRESSION_BLOCK="$(echo "${TASK_JSON}" | jq '.scoring.per_category_regression_block // 0.10')"

echo "[$(date -u +%H:%M:%S)] canary-run: ${TASK_COUNT} tasks against ${ENDPOINT}"
echo "                       adapter_version=${ADAPTER_VERSION}"
echo "                       task_set=${TASK_SET}"
echo "                       out=${OUT_PATH}"

if [[ "${DRY_RUN}" -eq 1 ]]; then
    echo ""
    echo "DRY RUN — first task:"
    echo "${TASK_JSON}" | jq '.tasks[0]'
    echo "(no requests sent; no output written)"
    exit 0
fi

# Health check the Doorman first
if ! curl -sS --max-time 5 "${ENDPOINT}/healthz" >/dev/null 2>&1; then
    echo "ERROR: Doorman unreachable at ${ENDPOINT}/healthz" >&2
    exit 4
fi

mkdir -p "$(dirname "${OUT_PATH}")"

declare -a RESULTS=()
TOTAL_PASS=0
TOTAL_FAIL=0

for i in $(seq 0 $((TASK_COUNT - 1))); do
    TASK_ID="$(echo "${TASK_JSON}" | jq -r ".tasks[${i}].id")"
    CATEGORY="$(echo "${TASK_JSON}" | jq -r ".tasks[${i}].category")"
    PROMPT="$(echo "${TASK_JSON}" | jq -r ".tasks[${i}].prompt")"
    MAX_TOKENS="$(echo "${TASK_JSON}" | jq ".tasks[${i}].max_tokens")"
    TIMEOUT_SEC="$(echo "${TASK_JSON}" | jq ".tasks[${i}].timeout_sec")"
    CONTAINS_JSON="$(echo "${TASK_JSON}" | jq -c ".tasks[${i}].contains")"
    NOT_CONTAINS_JSON="$(echo "${TASK_JSON}" | jq -c ".tasks[${i}].not_contains")"

    REQ_BODY="$(jq -nc \
        --arg model "${ADAPTER_VERSION}" \
        --arg prompt "${PROMPT}" \
        --argjson max_tokens "${MAX_TOKENS}" \
        '{
            model: $model,
            messages: [{role: "user", content: $prompt}],
            max_tokens: $max_tokens,
            stream: false
        }')"

    START_NS="$(date +%s%N)"
    RESPONSE="$(curl -sS --max-time "${TIMEOUT_SEC}" \
        -X POST "${ENDPOINT}/v1/messages" \
        -H 'content-type: application/json' \
        -d "${REQ_BODY}" 2>/dev/null || echo '{"_canary_timeout":true}')"
    END_NS="$(date +%s%N)"
    LATENCY_MS=$(((END_NS - START_NS) / 1000000))

    if echo "${RESPONSE}" | jq -e '._canary_timeout' >/dev/null 2>&1; then
        STATUS="timeout"
        CONTENT=""
    elif echo "${RESPONSE}" | jq -e '.error' >/dev/null 2>&1; then
        STATUS="error"
        CONTENT="$(echo "${RESPONSE}" | jq -rc '.error')"
    else
        STATUS="ok"
        CONTENT="$(echo "${RESPONSE}" | jq -r '.content[0].text // .content // ""' 2>/dev/null || echo "")"
    fi

    PASSED=1
    if [[ "${STATUS}" != "ok" ]]; then
        PASSED=0
    else
        CONTENT_LC="$(echo "${CONTENT}" | tr '[:upper:]' '[:lower:]')"
        while IFS= read -r needle; do
            needle_lc="$(echo "${needle}" | tr '[:upper:]' '[:lower:]')"
            if [[ "${CONTENT_LC}" != *"${needle_lc}"* ]]; then
                PASSED=0
                break
            fi
        done < <(echo "${CONTAINS_JSON}" | jq -r '.[]')
        if [[ "${PASSED}" -eq 1 ]]; then
            while IFS= read -r forbidden; do
                forbidden_lc="$(echo "${forbidden}" | tr '[:upper:]' '[:lower:]')"
                if [[ "${CONTENT_LC}" == *"${forbidden_lc}"* ]]; then
                    PASSED=0
                    break
                fi
            done < <(echo "${NOT_CONTAINS_JSON}" | jq -r '.[]')
        fi
    fi

    if [[ "${PASSED}" -eq 1 ]]; then
        SYMBOL="PASS"
        TOTAL_PASS=$((TOTAL_PASS + 1))
    else
        SYMBOL="FAIL"
        TOTAL_FAIL=$((TOTAL_FAIL + 1))
    fi

    printf "  %-40s %-22s %s (%dms)\n" "${TASK_ID}" "${CATEGORY}" "${SYMBOL}" "${LATENCY_MS}"

    RESULT_LINE="$(jq -nc \
        --arg task_id "${TASK_ID}" \
        --arg category "${CATEGORY}" \
        --arg status "${STATUS}" \
        --arg content "${CONTENT}" \
        --argjson passed "$([ "${PASSED}" -eq 1 ] && echo "true" || echo "false")" \
        --argjson latency_ms "${LATENCY_MS}" \
        '{task_id:$task_id, category:$category, status:$status, passed:$passed, latency_ms:$latency_ms, content:$content}')"
    RESULTS+=("${RESULT_LINE}")
done

# Build results JSON array and compute per-category stats via jq
RESULTS_JSON="$(printf '%s\n' "${RESULTS[@]}" | jq -s '.')"

BY_CATEGORY="$(echo "${RESULTS_JSON}" | jq -c '
    group_by(.category) |
    map({
        key: .[0].category,
        value: {
            pass: ([.[] | select(.passed)] | length),
            fail: ([.[] | select(.passed | not)] | length),
            rate: (([.[] | select(.passed)] | length) / length)
        }
    }) | from_entries')"

PASS_RATE="$(awk -v p="${TOTAL_PASS}" -v t="${TASK_COUNT}" 'BEGIN { printf "%.4f", p/t }')"

REPORT="$(jq -nc \
    --arg adapter "${ADAPTER_VERSION}" \
    --arg ts "${DATE_STAMP}" \
    --arg endpoint "${ENDPOINT}" \
    --argjson task_count "${TASK_COUNT}" \
    --argjson pass "${TOTAL_PASS}" \
    --argjson fail "${TOTAL_FAIL}" \
    --argjson rate "${PASS_RATE}" \
    --argjson promote_threshold "${PROMOTE_THRESHOLD}" \
    --argjson flag_threshold "${FLAG_THRESHOLD}" \
    --argjson by_category "${BY_CATEGORY}" \
    --argjson results "${RESULTS_JSON}" \
    '{
        adapter_version: $adapter,
        ran_at_utc: $ts,
        endpoint: $endpoint,
        task_count: $task_count,
        pass: $pass,
        fail: $fail,
        pass_rate: $rate,
        promote_threshold: $promote_threshold,
        flag_threshold: $flag_threshold,
        by_category: $by_category,
        results: $results
    }')"

echo "${REPORT}" | jq . > "${OUT_PATH}"
echo ""
echo "[$(date -u +%H:%M:%S)] canary-run summary:"
echo "  task_count:        ${TASK_COUNT}"
echo "  pass:              ${TOTAL_PASS} / ${TASK_COUNT}  (rate ${PASS_RATE})"
echo "  fail:              ${TOTAL_FAIL}"
echo "  promote threshold: ${PROMOTE_THRESHOLD}"
echo "  out:               ${OUT_PATH}"

# Per-category regression check: compare against most recent prior result
REGRESSION_FOUND=0
RESULTS_DIR="${FOUNDRY_ROOT}/data/canary/results"
PREV_RESULT="$(ls -t "${RESULTS_DIR}"/*.json 2>/dev/null | grep -Fxv "${OUT_PATH}" | head -1 || echo "")"

if [[ -n "${PREV_RESULT}" && -f "${PREV_RESULT}" ]]; then
    echo ""
    echo "  Regression check vs $(basename "${PREV_RESULT}"):"
    while IFS= read -r cat; do
        new_rate="$(echo "${BY_CATEGORY}" | jq -r --arg c "${cat}" '.[$c].rate')"
        prev_rate="$(jq -r --arg c "${cat}" '.by_category[$c].rate // "null"' "${PREV_RESULT}")"
        [[ "${prev_rate}" == "null" ]] && continue
        drop="$(awk -v n="${new_rate}" -v p="${prev_rate}" 'BEGIN { printf "%.4f", p - n }')"
        block="$(awk -v d="${drop}" -v b="${REGRESSION_BLOCK}" 'BEGIN { print (d+0 > b+0) ? "REGRESSION" : "ok" }')"
        printf "    %-25s prev=%3.0f%%  new=%3.0f%%  drop=%+.0f%%  %s\n" \
            "${cat}" \
            "$(awk -v r="${prev_rate}" 'BEGIN { printf "%.0f", r * 100 }')" \
            "$(awk -v r="${new_rate}" 'BEGIN { printf "%.0f", r * 100 }')" \
            "$(awk -v d="${drop}" 'BEGIN { printf "%.0f", d * 100 }')" \
            "${block}"
        if [[ "${block}" == "REGRESSION" ]]; then
            REGRESSION_FOUND=1
        fi
    done < <(echo "${BY_CATEGORY}" | jq -r 'keys[]')
fi

# Threshold verdict
PASS_RATE_AS_INT="$(awk -v r="${PASS_RATE}" 'BEGIN { print int(r * 100) }')"
PROMOTE_AS_INT="$(awk -v r="${PROMOTE_THRESHOLD}" 'BEGIN { print int(r * 100) }')"
FLAG_AS_INT="$(awk -v r="${FLAG_THRESHOLD}" 'BEGIN { print int(r * 100) }')"

if [[ "${REGRESSION_FOUND}" -eq 1 ]]; then
    echo "  verdict:           REGRESSION BLOCKED (>$(awk -v r="${REGRESSION_BLOCK}" 'BEGIN { print int(r*100) }')% drop in category)"
    [[ "${STRICT}" -eq 1 ]] && exit 5 || exit 0
elif [[ "${PASS_RATE_AS_INT}" -ge "${PROMOTE_AS_INT}" ]]; then
    echo "  verdict:           PROMOTABLE (>= ${PROMOTE_AS_INT}%)"
    exit 0
elif [[ "${PASS_RATE_AS_INT}" -ge "${FLAG_AS_INT}" ]]; then
    echo "  verdict:           OPERATOR REVIEW (${FLAG_AS_INT}%–${PROMOTE_AS_INT}%)"
    [[ "${STRICT}" -eq 1 ]] && exit 5 || exit 0
else
    echo "  verdict:           REJECTED (< ${FLAG_AS_INT}%)"
    [[ "${STRICT}" -eq 1 ]] && exit 5 || exit 0
fi
