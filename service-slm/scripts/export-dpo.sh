#!/usr/bin/env bash
# export-dpo.sh — Export promoted apprenticeship tuples as DPO training pairs.
#
# Phase 1 (P1-1.9) of learning-loop-master-plan-2026-05-18.md.
#
# Walks the apprenticeship corpus at
#   $FOUNDRY_ROOT/data/training-corpus/apprenticeship/<task-type>/*.jsonl
# selects rows where `verdict != null` (= ssh-signed by operator), and
# emits DPO-format JSONL pairs to
#   $FOUNDRY_ROOT/data/corpus/dpo/<YYYY-MM-DD>.jsonl
#
# Each output line is one training pair:
#   {
#     "prompt":   <full apprentice prompt — brief.body + acceptance_test>,
#     "chosen":   <actual_diff — the operator's committed diff>,
#     "rejected": <attempt.diff — the apprentice's proposed diff>,
#     "task_type": <kebab>,
#     "brief_id":  <ulid>,
#     "attempt_id":<ulid>,
#     "tier_used": "local" | "yoyo",
#     "adapter_version": <id-or-null>,
#     "doctrine_version": "0.0.13"
#   }
#
# DPO-incompatible rows are skipped (verdict=null, tier_used=external,
# missing actual_diff/attempt.diff, etc.) — the gate in slm-doorman
# already prevents these from landing, but this is a second line of
# defense for replay-safety.
#
# Usage:
#   ./export-dpo.sh [--dry-run] [--out=<path>] [--include-feedback]
#
# Exit codes:
#   0 — pairs written (or dry-run summary)
#   2 — corpus dir not found
#   3 — required tool missing (jq)
#   4 — output path exists (--out collision)

set -euo pipefail

FOUNDRY_ROOT="${FOUNDRY_ROOT:-/srv/foundry}"
CORPUS_ROOT="${FOUNDRY_ROOT}/data/training-corpus"
DATE_STAMP="$(date -u +%Y-%m-%d)"
OUT_PATH="${FOUNDRY_ROOT}/data/corpus/dpo/${DATE_STAMP}.jsonl"
DRY_RUN=0
INCLUDE_FEEDBACK=0

while [[ $# -gt 0 ]]; do
    case "$1" in
        --dry-run) DRY_RUN=1; shift ;;
        --out=*)   OUT_PATH="${1#--out=}"; shift ;;
        --include-feedback) INCLUDE_FEEDBACK=1; shift ;;
        --help|-h) sed -n '2,35p' "$0"; exit 0 ;;
        *) echo "Unknown argument: $1" >&2; exit 1 ;;
    esac
done

command -v jq >/dev/null 2>&1 || { echo "ERROR: jq required." >&2; exit 3; }
[[ -d "${CORPUS_ROOT}" ]] || { echo "ERROR: corpus dir not found: ${CORPUS_ROOT}" >&2; exit 2; }

if [[ "${DRY_RUN}" -eq 0 ]] && [[ -e "${OUT_PATH}" ]]; then
    echo "ERROR: output exists: ${OUT_PATH}" >&2
    echo "Pass --out=<different> or rm the existing file." >&2
    exit 4
fi

# ── Find candidate shadow tuples (apprenticeship JSONL with verdict) ────

APPR_DIR="${CORPUS_ROOT}/apprenticeship"

PROCESSED=0
EXPORTED=0
SKIPPED_NO_VERDICT=0
SKIPPED_EXTERNAL_TIER=0
SKIPPED_MISSING_FIELDS=0

# Emit jq filter that:
# 1. Reads one JSONL row.
# 2. Asserts verdict != null AND tier_used != "external".
# 3. Asserts both diffs are present.
# 4. Builds the DPO pair object.
DPO_FILTER='
    select(.verdict != null and .tier_used != "external") |
    select((.actual_diff // "") != "" and ((.attempt.diff // "") != "")) |
    {
        prompt: ((.brief.body // "") + "\n\n## Acceptance test\n" + (.brief.acceptance_test // "")),
        chosen: .actual_diff,
        rejected: .attempt.diff,
        task_type: .task_type,
        brief_id: (.brief.brief_id // .brief.id // null),
        attempt_id: (.attempt.attempt_id // null),
        tier_used: .tier_used,
        adapter_version: (.attempt.adapter_version // null),
        doctrine_version: .doctrine_version
    }
'

if [[ "${DRY_RUN}" -eq 0 ]]; then
    mkdir -p "$(dirname "${OUT_PATH}")"
    : > "${OUT_PATH}"
fi

# Find every shadow JSONL in the apprenticeship tree.
if [[ -d "${APPR_DIR}" ]]; then
    while IFS= read -r tuple_file; do
        PROCESSED=$((PROCESSED + 1))
        first_line="$(head -n 1 "${tuple_file}" 2>/dev/null || true)"
        [[ -z "${first_line}" ]] && continue

        # Try to emit a DPO pair via the filter. If filter returns empty, skip.
        emitted="$(echo "${first_line}" | jq -c "${DPO_FILTER}" 2>/dev/null || true)"
        if [[ -z "${emitted}" ]] || [[ "${emitted}" == "null" ]]; then
            # Categorise the skip — useful operator signal.
            verdict_val="$(echo "${first_line}" | jq -r '.verdict' 2>/dev/null || echo "null")"
            tier_val="$(echo "${first_line}" | jq -r '.tier_used' 2>/dev/null || echo "unknown")"
            if [[ "${verdict_val}" == "null" ]]; then
                SKIPPED_NO_VERDICT=$((SKIPPED_NO_VERDICT + 1))
            elif [[ "${tier_val}" == "external" ]]; then
                SKIPPED_EXTERNAL_TIER=$((SKIPPED_EXTERNAL_TIER + 1))
            else
                SKIPPED_MISSING_FIELDS=$((SKIPPED_MISSING_FIELDS + 1))
            fi
            continue
        fi

        EXPORTED=$((EXPORTED + 1))
        if [[ "${DRY_RUN}" -eq 0 ]]; then
            echo "${emitted}" >> "${OUT_PATH}"
        fi
    done < <(find "${APPR_DIR}" -type f -name 'shadow-*.jsonl' -size +0c)
fi

# ── Optional: include feedback/ rows (already-DPO-shaped) ───────────────

if [[ "${INCLUDE_FEEDBACK}" -eq 1 ]]; then
    FEEDBACK_DIR="${CORPUS_ROOT}/feedback"
    if [[ -d "${FEEDBACK_DIR}" ]]; then
        while IFS= read -r fb_file; do
            first_line="$(head -n 1 "${fb_file}" 2>/dev/null || true)"
            [[ -z "${first_line}" ]] && continue
            # Feedback rows have rejected_diff + corrected_diff; reshape.
            reshaped="$(echo "${first_line}" | jq -c '
                select(.rejected_diff != null and .corrected_diff != null) |
                {
                    prompt: ("Doctrine violation tag: " + (.doctrine_violation_tag // "unspecified")),
                    chosen: .corrected_diff,
                    rejected: .rejected_diff,
                    task_type: .task_type,
                    brief_id: .brief_id,
                    attempt_id: .attempt_id,
                    tier_used: "feedback",
                    adapter_version: null,
                    doctrine_version: (.doctrine_version // "0.0.13")
                }
            ' 2>/dev/null || true)"
            if [[ -n "${reshaped}" ]] && [[ "${reshaped}" != "null" ]]; then
                EXPORTED=$((EXPORTED + 1))
                if [[ "${DRY_RUN}" -eq 0 ]]; then
                    echo "${reshaped}" >> "${OUT_PATH}"
                fi
            fi
        done < <(find "${FEEDBACK_DIR}" -type f -name '*.jsonl' -size +0c)
    fi
fi

# ── Summary ─────────────────────────────────────────────────────────────

echo ""
echo "export-dpo.sh summary:"
echo "  processed:                ${PROCESSED}"
echo "  exported:                 ${EXPORTED}"
echo "  skipped (no verdict):     ${SKIPPED_NO_VERDICT}"
echo "  skipped (Tier C):         ${SKIPPED_EXTERNAL_TIER}"
echo "  skipped (missing fields): ${SKIPPED_MISSING_FIELDS}"
echo "  include_feedback:         ${INCLUDE_FEEDBACK}"
if [[ "${DRY_RUN}" -eq 1 ]]; then
    echo "  (dry run; no output written)"
else
    echo "  out:                      ${OUT_PATH} ($(wc -l < "${OUT_PATH}" 2>/dev/null || echo 0) lines)"
fi

# LIMA threshold per master plan: 1000 DPO pairs is the soft floor for
# meaningful LoRA training. Surface this so operator knows when to
# trigger lora-update.
LIMA_THRESHOLD=1000
if [[ "${EXPORTED}" -ge "${LIMA_THRESHOLD}" ]]; then
    echo ""
    echo "✓ At or above LIMA threshold (${LIMA_THRESHOLD} pairs) — ready for adapter training."
else
    GAP=$((LIMA_THRESHOLD - EXPORTED))
    echo ""
    echo "→ ${GAP} more pairs needed to reach LIMA threshold (${LIMA_THRESHOLD})."
fi
