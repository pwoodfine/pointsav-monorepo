#!/usr/bin/env bash
# export-sft.sh — Export apprenticeship queue entries as SFT training pairs.
#
# Companion to export-dpo.sh. Where export-dpo.sh needs operator-signed
# verdicts + an apprentice attempt diff (DPO pairs), this script needs only
# the human's real committed diff — the SFT gold label. No OLMo inference,
# no verdict required. Per BRIEF-slm-learning-loop.md §9/§10: SFT-first is
# the correct path at our data scale (<5K samples).
#
# Source: the shadow brief queue + done dirs, where each *.brief.jsonl is a
#   ShadowQueueEntry: {"brief": {...}, "actual_diff": "<real committed diff>"}
#     $FOUNDRY_ROOT/data/apprenticeship/queue/*.brief.jsonl       (pending)
#     $FOUNDRY_ROOT/data/apprenticeship/queue-done/*.brief.jsonl  (drained)
#
# Filter: actual_diff must be non-empty. This automatically EXCLUDES every
# pre-Fix-A entry (all have actual_diff == "") and includes only the real
# post-Fix-A briefs with genuine committed diffs.
#
# Each output line is one SFT instruction/output pair (Alpaca-style):
#   {
#     "instruction": <brief.body + scope + acceptance_test — the task>,
#     "input":       "",
#     "output":      <actual_diff — the human's committed diff>,
#     "task_type":   <kebab>,
#     "brief_id":    <id>,
#     "senior":      <senior_identity>,
#     "doctrine_version": <ver-or-null>
#   }
#
# Usage:
#   ./export-sft.sh [--dry-run] [--out=<path>] [--include-done]
#
# Exit codes:
#   0 — pairs written (or dry-run summary)
#   2 — apprenticeship dir not found
#   3 — required tool missing (jq)
#   4 — output path exists (--out collision)

set -euo pipefail

FOUNDRY_ROOT="${FOUNDRY_ROOT:-/srv/foundry}"
APPR_ROOT="${FOUNDRY_ROOT}/data/apprenticeship"
DATE_STAMP="$(date -u +%Y-%m-%d)"
OUT_PATH="${FOUNDRY_ROOT}/data/corpus/sft/${DATE_STAMP}.jsonl"
DRY_RUN=0
INCLUDE_DONE=0

while [[ $# -gt 0 ]]; do
    case "$1" in
        --dry-run) DRY_RUN=1; shift ;;
        --out=*)   OUT_PATH="${1#--out=}"; shift ;;
        --include-done) INCLUDE_DONE=1; shift ;;
        --help|-h) sed -n '2,38p' "$0"; exit 0 ;;
        *) echo "Unknown argument: $1" >&2; exit 1 ;;
    esac
done

command -v jq >/dev/null 2>&1 || { echo "ERROR: jq required." >&2; exit 3; }
[[ -d "${APPR_ROOT}" ]] || { echo "ERROR: apprenticeship dir not found: ${APPR_ROOT}" >&2; exit 2; }

if [[ "${DRY_RUN}" -eq 0 ]] && [[ -e "${OUT_PATH}" ]]; then
    echo "ERROR: output exists: ${OUT_PATH}" >&2
    echo "Pass --out=<different> or rm the existing file." >&2
    exit 4
fi

PROCESSED=0
EXPORTED=0
SKIPPED_EMPTY_DIFF=0

# jq filter: require a non-empty actual_diff, then build the SFT pair.
# The instruction concatenates brief.body + scope + acceptance_test so the
# model learns task → committed-diff mapping.
SFT_FILTER='
    select((.actual_diff // "") != "") |
    {
        instruction: (
            (.brief.body // "")
            + (if (.brief.scope // "") != "" then "\n\n## Scope\n" + (.brief.scope | tostring) else "" end)
            + (if (.brief.acceptance_test // "") != "" then "\n\n## Acceptance test\n" + (.brief.acceptance_test | tostring) else "" end)
        ),
        input: "",
        output: .actual_diff,
        task_type: (.brief.task_type // "git-commit"),
        brief_id: (.brief.brief_id // .brief.id // null),
        senior: (.brief.senior_identity // null),
        doctrine_version: (.brief.doctrine_version // .doctrine_version // null)
    }
'

if [[ "${DRY_RUN}" -eq 0 ]]; then
    mkdir -p "$(dirname "${OUT_PATH}")"
    : > "${OUT_PATH}"
fi

# Build the search list: always queue/, optionally queue-done/.
SEARCH_DIRS=("${APPR_ROOT}/queue")
if [[ "${INCLUDE_DONE}" -eq 1 ]]; then
    SEARCH_DIRS+=("${APPR_ROOT}/queue-done")
fi

for dir in "${SEARCH_DIRS[@]}"; do
    [[ -d "${dir}" ]] || continue
    while IFS= read -r entry_file; do
        PROCESSED=$((PROCESSED + 1))
        first_line="$(head -n 1 "${entry_file}" 2>/dev/null || true)"
        [[ -z "${first_line}" ]] && continue

        emitted="$(echo "${first_line}" | jq -c "${SFT_FILTER}" 2>/dev/null || true)"
        if [[ -z "${emitted}" ]] || [[ "${emitted}" == "null" ]]; then
            SKIPPED_EMPTY_DIFF=$((SKIPPED_EMPTY_DIFF + 1))
            continue
        fi

        EXPORTED=$((EXPORTED + 1))
        if [[ "${DRY_RUN}" -eq 0 ]]; then
            echo "${emitted}" >> "${OUT_PATH}"
        fi
    done < <(find "${dir}" -type f -name '*.brief.jsonl' -size +0c)
done

# ── Summary ─────────────────────────────────────────────────────────────

echo ""
echo "export-sft.sh summary:"
echo "  processed:               ${PROCESSED}"
echo "  exported:                ${EXPORTED}"
echo "  skipped (empty diff):    ${SKIPPED_EMPTY_DIFF}"
echo "  include_done:            ${INCLUDE_DONE}"
if [[ "${DRY_RUN}" -eq 1 ]]; then
    echo "  (dry run; no output written)"
else
    echo "  out:                     ${OUT_PATH} ($(wc -l < "${OUT_PATH}" 2>/dev/null || echo 0) lines)"
fi

# Soft floor for a meaningful first SFT LoRA run. Research (§9): 1,410 was the
# claimed corpus but all empty; real signal starts with post-Fix-A briefs.
# Even ~100 high-quality SFT pairs justify a first domain-adaptation run.
SFT_FLOOR=100
if [[ "${EXPORTED}" -ge "${SFT_FLOOR}" ]]; then
    echo ""
    echo "✓ ${EXPORTED} SFT pairs — at or above the ${SFT_FLOOR}-pair floor for a first LoRA run."
else
    GAP=$((SFT_FLOOR - EXPORTED))
    echo ""
    echo "→ ${EXPORTED} SFT pairs so far; ${GAP} more recommended before the first LoRA run."
    echo "  (Keep SLM_DRAIN_PAUSED=true; capture continues filling queue/ as you commit.)"
fi
