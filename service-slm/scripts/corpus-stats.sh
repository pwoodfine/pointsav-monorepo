#!/usr/bin/env bash
# corpus-stats.sh — survey the local engineering corpus for the project-slm cluster.
#
# Reports:
#   - Total tuple count
#   - Date range (oldest to newest by filename)
#   - Average tuple size in bytes
#   - Schema sanity-check on the 5 most recent events
#   - Any obvious malformations (truncated JSONL, missing required fields)
#
# Corpus directory: $HOME/Foundry/data/training-corpus/engineering/project-slm/
# Override with CORPUS_DIR env var if the corpus lives elsewhere.
#
# Exit 0 on success; exit 1 if the corpus directory does not exist.
#
# Usage:
#   ./scripts/corpus-stats.sh
#   CORPUS_DIR=/custom/path ./scripts/corpus-stats.sh

set -euo pipefail

CORPUS_DIR="${CORPUS_DIR:-${HOME}/Foundry/data/training-corpus/engineering/project-slm}"

# Required top-level fields in engineering corpus tuples.
# Derived from actual tuple schema observed in the corpus.
REQUIRED_FIELDS=(
    "tuple_type"
    "cluster"
    "source_commit"
    "commit_msg"
    "tenant"
    "doctrine_version"
)

# ─── directory check ──────────────────────────────────────────────────────────

if [[ ! -d "$CORPUS_DIR" ]]; then
    echo "ERROR: corpus directory not found: $CORPUS_DIR"
    echo "  Set CORPUS_DIR env var to the correct path, or verify the corpus has been created."
    echo "  The capture-edit hook writes to this directory on each commit."
    exit 1
fi

echo "=== Engineering corpus stats — project-slm ==="
echo "Directory: $CORPUS_DIR"
echo "Date:      $(date -u +%Y-%m-%dT%H:%M:%SZ)"
echo ""

# ─── file inventory ───────────────────────────────────────────────────────────

# Collect all .jsonl files
mapfile -t jsonl_files < <(find "$CORPUS_DIR" -maxdepth 1 -name '*.jsonl' -type f | sort)

total_files="${#jsonl_files[@]}"

if [[ "$total_files" -eq 0 ]]; then
    echo "No .jsonl files found in corpus directory."
    echo "(Corpus is populated by the capture-edit hook on each commit.)"
    exit 0
fi

# ─── tuple count ─────────────────────────────────────────────────────────────
# Each file is one tuple (one git commit = one JSONL file with a single JSON object).

echo "Tuple count: $total_files"

# ─── date range ───────────────────────────────────────────────────────────────
# Files are named by git SHA; sort by modification time for range.

oldest_file="$(ls -t "$CORPUS_DIR"/*.jsonl 2>/dev/null | tail -1)"
newest_file="$(ls -t "$CORPUS_DIR"/*.jsonl 2>/dev/null | head -1)"

oldest_mtime="$(date -r "$oldest_file" +%Y-%m-%dT%H:%M:%SZ 2>/dev/null || echo 'unknown')"
newest_mtime="$(date -r "$newest_file" +%Y-%m-%dT%H:%M:%SZ 2>/dev/null || echo 'unknown')"

echo "Date range: $oldest_mtime → $newest_mtime"

# ─── average tuple size ───────────────────────────────────────────────────────

total_bytes=0
for f in "${jsonl_files[@]}"; do
    sz="$(wc -c < "$f" 2>/dev/null || echo 0)"
    total_bytes=$(( total_bytes + sz ))
done

if [[ "$total_files" -gt 0 ]]; then
    avg_bytes=$(( total_bytes / total_files ))
else
    avg_bytes=0
fi

echo "Total size: $(( total_bytes / 1024 )) KiB"
echo "Avg size:   ${avg_bytes} bytes / tuple"

# ─── schema sanity-check — 5 most recent tuples ──────────────────────────────

echo ""
echo "Schema sanity-check (5 most recent tuples):"

# Get 5 most recent by mtime
mapfile -t recent_files < <(ls -t "$CORPUS_DIR"/*.jsonl 2>/dev/null | head -5)

schema_pass=0
schema_fail=0
malformed=0

for f in "${recent_files[@]}"; do
    fname="$(basename "$f")"
    # Check file is non-empty and contains valid JSON on line 1
    if [[ ! -s "$f" ]]; then
        echo "  [MALFORMED] $fname — file is empty"
        malformed=$(( malformed + 1 ))
        continue
    fi

    # Try to read the file as a JSON object
    if ! python3 -c "import json,sys; json.load(open('$f'))" 2>/dev/null; then
        echo "  [MALFORMED] $fname — invalid JSON (truncated or corrupt)"
        malformed=$(( malformed + 1 ))
        continue
    fi

    # Check required fields
    missing_fields=()
    for field in "${REQUIRED_FIELDS[@]}"; do
        if ! python3 -c "
import json, sys
d = json.load(open('$f'))
if '$field' not in d:
    sys.exit(1)
" 2>/dev/null; then
            missing_fields+=("$field")
        fi
    done

    if [[ "${#missing_fields[@]}" -gt 0 ]]; then
        echo "  [SCHEMA-WARN] $fname — missing fields: ${missing_fields[*]}"
        schema_fail=$(( schema_fail + 1 ))
    else
        echo "  [OK] $fname"
        schema_pass=$(( schema_pass + 1 ))
    fi
done

echo ""
echo "Schema check: $schema_pass OK / ${#recent_files[@]} checked"
if [[ "$malformed" -gt 0 ]]; then
    echo "WARNING: $malformed malformed file(s) found — inspect manually"
fi

# ─── spot-check most recent event ─────────────────────────────────────────────

latest_file="$(ls -t "$CORPUS_DIR"/*.jsonl 2>/dev/null | head -1)"
if [[ -n "$latest_file" && -s "$latest_file" ]]; then
    echo ""
    echo "Most recent tuple: $(basename "$latest_file")"
    python3 -c "
import json, sys
d = json.load(open('$latest_file'))
for k in ['tuple_type', 'cluster', 'tenant', 'doctrine_version', 'source_commit']:
    v = d.get(k, '(missing)')
    print(f'  {k}: {v}')
commit_msg = d.get('commit_msg', '')
first_line = commit_msg.split('\n')[0][:80] if commit_msg else '(missing)'
print(f'  commit_msg: {first_line}')
print(f'  fields present: {len(d)} top-level keys')
" 2>/dev/null || echo "  (could not parse — check file manually)"
fi

# ─── apprenticeship corpus advisory ───────────────────────────────────────────

APPR_DIR="${HOME}/Foundry/data/training-corpus/apprenticeship"
if [[ -d "$APPR_DIR" ]]; then
    appr_count="$(find "$APPR_DIR" -name '*.jsonl' -type f 2>/dev/null | wc -l)"
    echo ""
    echo "Apprenticeship corpus: $appr_count tuple(s) at $APPR_DIR"
    echo "(populated by /v1/verdict and /v1/shadow endpoints when SLM_APPRENTICESHIP_ENABLED=true)"
else
    echo ""
    echo "Apprenticeship corpus: not yet created at $APPR_DIR"
    echo "(will be created on first /v1/verdict call once SLM_APPRENTICESHIP_ENABLED=true)"
fi

echo ""
echo "Done."
exit 0
