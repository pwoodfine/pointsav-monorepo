#!/usr/bin/env bash
# corpus-snapshot.sh — Freeze a snapshot of the training corpus for
# replayable LoRA training runs.
#
# Phase 1 (P1-1.9) of learning-loop-master-plan-2026-05-18.md.
#
# Creates:
#   $FOUNDRY_ROOT/data/training-corpus/snapshots/<YYYY-MM-DD>/
#       ├── corpus.tar.zst         — zstd-compressed tarball
#       ├── manifest.json          — file count, total size, sha256 of tarball
#       └── tuples.shasum.txt      — per-tuple sha256 list (audit replay)
#
# A snapshot is the immutable input to a LoRA training run. The adapter
# registry (data/adapters/registry.yaml) records `corpus_sha` = sha256
# of the tarball, so any adapter can be re-trained from a fixed input.
#
# Usage:
#   ./corpus-snapshot.sh [--dry-run] [--out=<dir>]
#
# Exit codes:
#   0 — snapshot written (or dry-run summary)
#   1 — argument error
#   2 — corpus directory not found
#   3 — required tool missing (tar, zstd, sha256sum, jq)
#   4 — snapshot path already exists (refuse to clobber)

set -euo pipefail

FOUNDRY_ROOT="${FOUNDRY_ROOT:-/srv/foundry}"
CORPUS_ROOT="${FOUNDRY_ROOT}/data/training-corpus"
DATE_STAMP="$(date -u +%Y-%m-%d)"
OUT_DIR=""
DRY_RUN=0

while [[ $# -gt 0 ]]; do
    case "$1" in
        --dry-run) DRY_RUN=1; shift ;;
        --out=*)   OUT_DIR="${1#--out=}"; shift ;;
        --help|-h) sed -n '2,30p' "$0"; exit 0 ;;
        *) echo "Unknown argument: $1" >&2; exit 1 ;;
    esac
done

if [[ -z "${OUT_DIR}" ]]; then
    OUT_DIR="${CORPUS_ROOT}/snapshots/${DATE_STAMP}"
fi

for tool in tar zstd sha256sum jq; do
    command -v "${tool}" >/dev/null 2>&1 || {
        echo "ERROR: required tool not installed: ${tool}" >&2
        exit 3
    }
done

[[ -d "${CORPUS_ROOT}" ]] || {
    echo "ERROR: corpus directory not found: ${CORPUS_ROOT}" >&2
    exit 2
}

if [[ "${DRY_RUN}" -eq 0 ]] && [[ -e "${OUT_DIR}" ]]; then
    echo "ERROR: snapshot path exists: ${OUT_DIR}" >&2
    echo "Pass --out=<different> to override, or rm the existing path first." >&2
    exit 4
fi

# ── Inventory ───────────────────────────────────────────────────────────

# Count files first so we can refuse "snapshot empty corpus" silently.
TUPLE_COUNT="$(find "${CORPUS_ROOT}" -type f -name '*.jsonl' -size +0c -not -path '*/snapshots/*' -not -name '.corpus-index.jsonl' | wc -l | tr -d ' ')"
if [[ "${TUPLE_COUNT}" -eq 0 ]]; then
    echo "WARNING: corpus is empty (no .jsonl files outside snapshots/)" >&2
    echo "Snapshot will be empty. Continue anyway? (--force or rerun with content)" >&2
    [[ "${DRY_RUN}" -eq 0 ]] && exit 2
fi

if [[ "${DRY_RUN}" -eq 1 ]]; then
    echo "corpus-snapshot.sh dry-run summary:"
    echo "  corpus_root:  ${CORPUS_ROOT}"
    echo "  out_dir:      ${OUT_DIR}"
    echo "  tuple_count:  ${TUPLE_COUNT}"
    echo "  (no files written)"
    exit 0
fi

mkdir -p "${OUT_DIR}"

# ── Per-tuple SHA-256 list ──────────────────────────────────────────────

TUPLES_FILE="${OUT_DIR}/tuples.shasum.txt"
echo "[$(date -u +%H:%M:%S)] hashing ${TUPLE_COUNT} tuples → ${TUPLES_FILE}"
(
    cd "${CORPUS_ROOT}"
    find . -type f -name '*.jsonl' -size +0c \
        -not -path './snapshots/*' \
        -not -name '.corpus-index.jsonl' \
        -print0 | sort -z | xargs -0 sha256sum
) > "${TUPLES_FILE}"

# ── Tarball ─────────────────────────────────────────────────────────────

TARBALL="${OUT_DIR}/corpus.tar.zst"
echo "[$(date -u +%H:%M:%S)] writing tarball → ${TARBALL}"
(
    cd "${CORPUS_ROOT}"
    tar --create --file=- \
        --exclude='./snapshots' \
        --exclude='.corpus-index.jsonl' \
        . | zstd -19 --long -T0 -o "${TARBALL}"
)

# ── Manifest ────────────────────────────────────────────────────────────

TARBALL_SHA="$(sha256sum "${TARBALL}" | awk '{print $1}')"
TUPLES_SHA="$(sha256sum "${TUPLES_FILE}" | awk '{print $1}')"
TARBALL_SIZE="$(stat -c %s "${TARBALL}")"

cat > "${OUT_DIR}/manifest.json" <<EOF
{
  "schema": "foundry-corpus-snapshot-v1",
  "date": "${DATE_STAMP}",
  "created_at": "$(date -u +%Y-%m-%dT%H:%M:%SZ)",
  "corpus_root": "${CORPUS_ROOT}",
  "tuple_count": ${TUPLE_COUNT},
  "tarball_path": "corpus.tar.zst",
  "tarball_size_bytes": ${TARBALL_SIZE},
  "tarball_sha256": "${TARBALL_SHA}",
  "tuples_shasum_sha256": "${TUPLES_SHA}",
  "doctrine_version": "0.0.13"
}
EOF

echo "[$(date -u +%H:%M:%S)] snapshot complete:"
echo "  ${OUT_DIR}/manifest.json"
echo "  ${OUT_DIR}/corpus.tar.zst (${TARBALL_SIZE} bytes, sha256: ${TARBALL_SHA:0:16}...)"
echo "  ${OUT_DIR}/tuples.shasum.txt"
echo ""
echo "Adapter-registry corpus_sha for this snapshot: ${TARBALL_SHA}"
