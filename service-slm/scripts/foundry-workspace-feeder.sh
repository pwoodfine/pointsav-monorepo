#!/usr/bin/env bash
# Foundry workspace DataGraph feeder — module_id: foundry-workspace
#
# Scans known artifact paths in ~/Foundry and emits CORPUS_*.json into
# service-content's ledger dir. service-content processes them and writes
# entities into the 'foundry-workspace' LadybugDB module namespace.
#
# Idempotent: skips artifacts whose CORPUS_*.json or SEMANTIC_*.json
# already exist.
#
# Usage:
#   ./scripts/foundry-workspace-feeder.sh [--batch-size N]
#   FOUNDRY_WORKSPACE_ROOT=/home/user/Foundry ./scripts/foundry-workspace-feeder.sh
#
# Env vars:
#   FOUNDRY_WORKSPACE_ROOT    — root of the Foundry workspace (default ~/Foundry)
#   FOUNDRY_CONTENT_LEDGER    — service-content ledger dir for CORPUS_*.json output
#   FOUNDRY_SEMANTIC_DIR      — where SEMANTIC_*.json are written (idempotency check)
#   BATCH_SIZE                — max files per invocation (default 20)
set -uo pipefail

FOUNDRY_WORKSPACE_ROOT="${FOUNDRY_WORKSPACE_ROOT:-${HOME}/Foundry}"
FOUNDRY_CONTENT_LEDGER="${FOUNDRY_CONTENT_LEDGER:-${HOME}/deployments/woodfine-fleet-deployment/cluster-totebox-jennifer/service-fs/data/service-content/ledgers}"
FOUNDRY_SEMANTIC_DIR="${FOUNDRY_SEMANTIC_DIR:-${HOME}/deployments/woodfine-fleet-deployment/cluster-totebox-jennifer/service-fs/data/service-people/ledgers}"

BATCH_SIZE=20
while [[ $# -gt 0 ]]; do
    case "$1" in
        --batch-size) BATCH_SIZE="$2"; shift 2 ;;
        *) echo "Unknown flag: $1" >&2; exit 1 ;;
    esac
done

mkdir -p "${FOUNDRY_CONTENT_LEDGER}"

echo "[fw-feeder] Foundry root:    ${FOUNDRY_WORKSPACE_ROOT}"
echo "[fw-feeder] Ledger dir:      ${FOUNDRY_CONTENT_LEDGER}"
echo "[fw-feeder] Batch size:      ${BATCH_SIZE}"
echo "================================================================"

COUNT=0
SKIPPED=0

emit_corpus() {
    local filepath="$1"
    local artifact_type="$2"

    [[ "${COUNT}" -ge "${BATCH_SIZE}" ]] && return

    # Stable worm_id from sha1 of relative path
    local rel_path="${filepath#${FOUNDRY_WORKSPACE_ROOT}/}"
    local worm_id="fw-$(echo -n "${rel_path}" | sha1sum | cut -c1-12)"

    local corpus_file="${FOUNDRY_CONTENT_LEDGER}/CORPUS_${worm_id}.json"
    local semantic_file="${FOUNDRY_SEMANTIC_DIR}/SEMANTIC_${worm_id}.json"

    if [[ -f "${corpus_file}" ]] || [[ -f "${semantic_file}" ]]; then
        SKIPPED=$((SKIPPED + 1))
        return
    fi

    # Read the file; strip YAML frontmatter if present, keep prose
    local corpus_text
    corpus_text="$(python3 - "${filepath}" "${artifact_type}" "${rel_path}" <<'PYEOF'
import sys, re

filepath, artifact_type, rel_path = sys.argv[1], sys.argv[2], sys.argv[3]

try:
    with open(filepath, encoding="utf-8", errors="ignore") as f:
        raw = f.read()
except Exception:
    print("")
    sys.exit(0)

# Strip YAML frontmatter (---\n...\n---)
raw = re.sub(r"^---\n.*?\n---\n", "", raw, flags=re.DOTALL)

# Strip markdown headings markers but keep the text
text = re.sub(r"^#{1,6}\s+", "", raw, flags=re.MULTILINE)

# Collapse blank lines
text = re.sub(r"\n{3,}", "\n\n", text).strip()

# Truncate to ~3000 chars for context budget
if len(text) > 3000:
    text = text[:3000] + "..."

title = rel_path.split("/")[-1].replace(".md", "").replace("-", " ").replace("_", " ")

parts = [
    f"Artifact type: {artifact_type}",
    f"Source: {rel_path}",
    f"Title: {title}",
    "",
    text,
]
print("\n".join(parts))
PYEOF
)"

    if [[ -z "${corpus_text}" ]]; then
        echo "  [SKIP] ${rel_path} — empty corpus"
        return
    fi

    python3 -c "
import json, sys
corpus = sys.stdin.read()
d = {'worm_id': sys.argv[1], 'module_id': 'foundry-workspace',
     'corpus': corpus, 'source_path': sys.argv[2], 'artifact_type': sys.argv[3]}
print(json.dumps(d))
" "${worm_id}" "${rel_path}" "${artifact_type}" <<< "${corpus_text}" > "${corpus_file}"

    echo "  [EMIT] CORPUS_${worm_id}.json  ← ${rel_path}"
    COUNT=$((COUNT + 1))

    sleep "$(python3 -c "import random; print(round(random.uniform(0.2, 1.5), 1))")"
}

# ── Artifact scan paths ───────────────────────────────────────────────────────

# 1. Conventions (TOPIC-class)
while IFS= read -r -d '' f; do
    emit_corpus "${f}" "CONVENTION"
done < <(find "${FOUNDRY_WORKSPACE_ROOT}/conventions" -maxdepth 1 -name "*.md" -print0 2>/dev/null)

# 2. GUIDEs from woodfine-fleet-deployment customer catalog
while IFS= read -r -d '' f; do
    emit_corpus "${f}" "GUIDE"
done < <(find "${FOUNDRY_WORKSPACE_ROOT}/customer" -name "guide-*.md" -print0 2>/dev/null)

# 3. Wiki topics from project-* content-wiki-documentation clones
while IFS= read -r -d '' f; do
    emit_corpus "${f}" "TOPIC"
done < <(find "${FOUNDRY_WORKSPACE_ROOT}/clones" -path "*/content-wiki-documentation/topic-*.md" -print0 2>/dev/null)

# ── Summary ───────────────────────────────────────────────────────────────────
echo "================================================================"
echo "[fw-feeder] Done — emitted: ${COUNT}, skipped: ${SKIPPED}."
