#!/usr/bin/env bash
# eval-prepare.sh — Curate eval-holdout candidates from the training corpus.
#
# Phase 1 (P1-1.2-prep) of learning-loop-master-plan-2026-05-18.md.
#
# Selects 100 candidate tuples from the corpus at
# `$FOUNDRY_ROOT/data/training-corpus/apprenticeship/<task-type>/_review/`
# and `engineering/<cluster>/` distributing evenly across:
#   - task_type (every active type gets representation)
#   - tenant (pointsav + woodfine if both present)
#   - cluster (project-intelligence, project-data, master, ...)
#
# Writes candidates to:
#   $FOUNDRY_ROOT/data/training-corpus/eval/candidates-<YYYY-MM-DD>.jsonl
#
# The operator then reviews and ssh-signs a curated subset as the
# canonical held-out eval set:
#   $FOUNDRY_ROOT/data/training-corpus/eval/holdout-v1.jsonl
# (signing step is operator-only; this script only PREPARES, never signs.)
#
# Usage:
#   ./eval-prepare.sh [--target-count=100] [--out=<path>] [--dry-run]
#
# Exit codes:
#   0  Success — candidates written (or dry-run summary printed)
#   1  Argument error
#   2  FOUNDRY_ROOT/data/training-corpus/ not found or empty
#   3  jq not installed
#   4  Output path already exists (refuse to clobber; pass explicit --out)

set -euo pipefail

# ── Defaults ────────────────────────────────────────────────────────────

FOUNDRY_ROOT="${FOUNDRY_ROOT:-/srv/foundry}"
TARGET_COUNT=100
DRY_RUN=0
DATE_STAMP="$(date -u +%Y-%m-%d)"
OUT_PATH="${FOUNDRY_ROOT}/data/training-corpus/eval/candidates-${DATE_STAMP}.jsonl"

# ── Argument parse ──────────────────────────────────────────────────────

while [[ $# -gt 0 ]]; do
    case "$1" in
        --target-count=*)
            TARGET_COUNT="${1#--target-count=}"
            shift
            ;;
        --out=*)
            OUT_PATH="${1#--out=}"
            shift
            ;;
        --dry-run)
            DRY_RUN=1
            shift
            ;;
        --help|-h)
            sed -n '2,30p' "$0"
            exit 0
            ;;
        *)
            echo "Unknown argument: $1" >&2
            echo "Usage: $0 [--target-count=N] [--out=path] [--dry-run]" >&2
            exit 1
            ;;
    esac
done

# ── Preconditions ───────────────────────────────────────────────────────

if ! command -v jq >/dev/null 2>&1; then
    echo "ERROR: jq is required but not installed." >&2
    exit 3
fi

CORPUS_DIR="${FOUNDRY_ROOT}/data/training-corpus"
if [[ ! -d "${CORPUS_DIR}" ]]; then
    echo "ERROR: corpus directory not found: ${CORPUS_DIR}" >&2
    exit 2
fi

if [[ "${DRY_RUN}" -eq 0 ]] && [[ -e "${OUT_PATH}" ]]; then
    echo "ERROR: output path already exists: ${OUT_PATH}" >&2
    echo "Pass --out=<different-path> to override, or rm the existing file first." >&2
    exit 4
fi

# ── Inventory ───────────────────────────────────────────────────────────

# Source pools — files we can sample from:
#   1. apprenticeship review-stage tuples (post-P1-1.4 land in _review/)
#   2. apprenticeship promoted tuples (post-operator-promote)
#   3. engineering tuples (post-commit hook captures)
#
# Each tuple is a single-line JSONL row.

APPR_DIR="${CORPUS_DIR}/apprenticeship"
ENG_DIR="${CORPUS_DIR}/engineering"

declare -a ALL_FILES=()
if [[ -d "${APPR_DIR}" ]]; then
    while IFS= read -r f; do
        ALL_FILES+=("$f")
    done < <(find "${APPR_DIR}" -type f -name '*.jsonl' -size +0c)
fi
if [[ -d "${ENG_DIR}" ]]; then
    while IFS= read -r f; do
        ALL_FILES+=("$f")
    done < <(find "${ENG_DIR}" -type f -name '*.jsonl' -size +0c)
fi

TOTAL_AVAILABLE="${#ALL_FILES[@]}"
if [[ "${TOTAL_AVAILABLE}" -eq 0 ]]; then
    echo "ERROR: no tuples found under ${CORPUS_DIR}" >&2
    exit 2
fi

# ── Sampling — stratified by tuple_type + task_type ────────────────────

# For each file, extract (tuple_type, task_type, tenant, cluster). Group by
# (tuple_type, task_type) and round-robin across groups up to TARGET_COUNT.

declare -A GROUP_FILES=()       # group_key → newline-separated file list
declare -a GROUP_KEYS=()        # ordered group keys

for f in "${ALL_FILES[@]}"; do
    # Read first line of the file — single-line JSONL convention.
    first_line="$(head -n 1 "$f" 2>/dev/null || true)"
    [[ -z "${first_line}" ]] && continue

    # Try to extract tuple_type + task_type. Tolerate parse errors silently
    # (we drop malformed rows from the eval set; corpus_gate should have
    # caught them but this is the second line of defense).
    keys="$(echo "${first_line}" | jq -r '
        [(.tuple_type // "unknown"),
         (.task_type // (.scope // "unknown")),
         (.tenant // "unknown")] | join("|")
    ' 2>/dev/null || true)"

    [[ -z "${keys}" ]] && continue

    if [[ -z "${GROUP_FILES[$keys]+_}" ]]; then
        GROUP_FILES[$keys]=""
        GROUP_KEYS+=("$keys")
    fi
    GROUP_FILES[$keys]+="${f}"$'\n'
done

if [[ "${#GROUP_KEYS[@]}" -eq 0 ]]; then
    echo "ERROR: no parseable tuples in ${CORPUS_DIR}" >&2
    exit 2
fi

# Shuffle within each group, then round-robin until we hit TARGET_COUNT.
declare -a SELECTED=()
COUNT=0
while [[ "${COUNT}" -lt "${TARGET_COUNT}" ]]; do
    PROGRESS_THIS_ROUND=0
    for key in "${GROUP_KEYS[@]}"; do
        [[ "${COUNT}" -ge "${TARGET_COUNT}" ]] && break

        # Pop one file from this group's pool.
        group_list="${GROUP_FILES[$key]}"
        [[ -z "${group_list}" ]] && continue

        # Pick a random file from the group via shuf.
        candidate="$(echo "${group_list}" | grep -v '^$' | shuf -n 1 || true)"
        [[ -z "${candidate}" ]] && continue

        # Remove the chosen file from the pool.
        GROUP_FILES[$key]="$(echo "${group_list}" | grep -v '^$' | grep -vFx "${candidate}" || true)"
        GROUP_FILES[$key]="${GROUP_FILES[$key]}"$'\n'

        SELECTED+=("${candidate}")
        COUNT=$((COUNT + 1))
        PROGRESS_THIS_ROUND=1
    done
    # All groups exhausted; stop early.
    [[ "${PROGRESS_THIS_ROUND}" -eq 0 ]] && break
done

# ── Output ──────────────────────────────────────────────────────────────

echo ""
echo "eval-prepare.sh — candidate selection summary"
echo "  corpus_dir:     ${CORPUS_DIR}"
echo "  target_count:   ${TARGET_COUNT}"
echo "  available:      ${TOTAL_AVAILABLE} files"
echo "  groups:         ${#GROUP_KEYS[@]} (tuple_type|task_type|tenant)"
echo "  selected:       ${#SELECTED[@]}"
echo "  out:            ${OUT_PATH}"
echo ""

if [[ "${DRY_RUN}" -eq 1 ]]; then
    echo "DRY RUN — first 10 selected files:"
    printf '  %s\n' "${SELECTED[@]:0:10}"
    echo ""
    echo "(dry run; no output written)"
    exit 0
fi

# Stream selected file CONTENTS (single-line JSONL each) into the output.
# Each input file is one tuple = one JSONL line in output. We add a
# `_eval_source_path` field so the operator-signing step can trace
# back to the source.

mkdir -p "$(dirname "${OUT_PATH}")"

: > "${OUT_PATH}"
for f in "${SELECTED[@]}"; do
    first_line="$(head -n 1 "$f")"
    # Inject _eval_source_path via jq.
    enriched="$(echo "${first_line}" | jq -c --arg p "$f" '. + {_eval_source_path: $p}' 2>/dev/null)"
    if [[ -n "${enriched}" ]]; then
        echo "${enriched}" >> "${OUT_PATH}"
    fi
done

echo "Wrote ${OUT_PATH} ($(wc -l < "${OUT_PATH}") tuples)"
echo ""
echo "Next steps (operator):"
echo "  1. Review candidates: less ${OUT_PATH}"
echo "  2. Optionally curate down by editing the file (keep only the strong examples)"
echo "  3. SSH-sign the file as the canonical holdout:"
echo "     ssh-keygen -Y sign \\"
echo "       -f ~/.ssh/id_<operator> \\"
echo "       -n eval-holdout-v1 \\"
echo "       ${OUT_PATH}"
echo "     mv ${OUT_PATH}.sig ${CORPUS_DIR}/eval/holdout-v1.sig"
echo "     cp ${OUT_PATH} ${CORPUS_DIR}/eval/holdout-v1.jsonl"
echo "  4. bin/eval-adapter.sh refuses to promote any adapter that regresses"
echo "     on the held-out set."
