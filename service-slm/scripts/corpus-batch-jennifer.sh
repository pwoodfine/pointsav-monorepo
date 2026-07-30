#!/usr/bin/env bash
# Batch CORPUS ingestion for cluster-totebox-jennifer DataGraph rebuild.
#
# Scans jennifer's service-content and service-research source dirs, emits
# CORPUS_*.json into service-content's ledger dir for LadybugDB extraction
# during the nightly Yo-Yo #1 window.
#
# Idempotent: skips any worm_id that already has a SEMANTIC_*.json in the
# CRM output dir.
#
# Usage:
#   ./scripts/corpus-batch-jennifer.sh [--batch-size N]
#   JENNIFER_BASE=/path/to/deployment ./scripts/corpus-batch-jennifer.sh
#
# Env vars (with defaults):
#   JENNIFER_BASE          — jennifer deployment root
#   SERVICE_CONTENT_LEDGER — service-content ledger dir (CORPUS_*.json goes here)
#   JENNIFER_CRM_DIR       — where SEMANTIC_*.json files are written (idempotency check)
#   BATCH_SIZE             — max files to process per invocation (default 50)
set -uo pipefail

JENNIFER_BASE="${JENNIFER_BASE:-/home/mathew/deployments/woodfine-fleet-deployment/cluster-totebox-jennifer}"
SERVICE_CONTENT_LEDGER="${SERVICE_CONTENT_LEDGER:-${JENNIFER_BASE}/service-fs/data/service-content/ledgers}"
JENNIFER_CRM_DIR="${JENNIFER_CRM_DIR:-${JENNIFER_BASE}/service-fs/data/service-people/ledgers}"

# Parse --batch-size flag
BATCH_SIZE=50
while [[ $# -gt 0 ]]; do
    case "$1" in
        --batch-size) BATCH_SIZE="$2"; shift 2 ;;
        *) echo "Unknown flag: $1" >&2; exit 1 ;;
    esac
done

mkdir -p "${SERVICE_CONTENT_LEDGER}"

echo "[corpus-batch] Jennifer base: ${JENNIFER_BASE}"
echo "[corpus-batch] Ledger dir:    ${SERVICE_CONTENT_LEDGER}"
echo "[corpus-batch] CRM dir:       ${JENNIFER_CRM_DIR}"
echo "[corpus-batch] Batch size:    ${BATCH_SIZE}"
echo "================================================================"

COUNT=0
SKIPPED=0

# Scan source JSON payloads in service-people/source
SOURCE_DIR="${JENNIFER_BASE}/service-fs/data/service-people/source"
if [[ -d "${SOURCE_DIR}" ]]; then
    while IFS= read -r -d '' payload_file; do
        [[ "${COUNT}" -ge "${BATCH_SIZE}" ]] && break

        worm_id="$(basename "${payload_file}" .json)"
        semantic_file="${JENNIFER_CRM_DIR}/SEMANTIC_${worm_id}.json"
        corpus_file="${SERVICE_CONTENT_LEDGER}/CORPUS_${worm_id}.json"

        # Skip if already processed
        if [[ -f "${semantic_file}" ]]; then
            SKIPPED=$((SKIPPED + 1))
            continue
        fi
        # Skip if CORPUS already dropped (service-content may still be processing)
        if [[ -f "${corpus_file}" ]]; then
            SKIPPED=$((SKIPPED + 1))
            continue
        fi

        # Extract corpus text from the source payload
        # The payload has file.filename + edge_entities; build a text summary.
        corpus_text="$(python3 - "${payload_file}" <<'PYEOF'
import sys, json, base64, re

try:
    with open(sys.argv[1]) as f:
        p = json.load(f)
except Exception as e:
    print("")
    sys.exit(0)

parts = []
fn = p.get("file", {}).get("filename", "")
if fn:
    parts.append(f"Document: {fn}")

dest = p.get("destination_archive", "")
if dest:
    parts.append(f"Archive: {dest}")

# Try to decode the email body for richer text
b64 = p.get("file", {}).get("data", "")
if b64:
    try:
        if "," in b64:
            b64 = b64.split(",", 1)[1]
        raw = base64.b64decode(b64).decode("utf-8", errors="ignore")
        # Grab up to 1000 chars of printable text
        text = re.sub(r"<[^>]+>", " ", raw)  # strip HTML tags
        text = re.sub(r"\s+", " ", text).strip()
        if text:
            parts.append(text[:1000])
    except Exception:
        pass

# Add edge entities
for ent in p.get("edge_entities", []):
    name = ent.get("entity_name", "").strip()
    cls = ent.get("classification", "UNKNOWN")
    if name:
        parts.append(f"{cls}: {name}")

print("\n".join(parts))
PYEOF
)"

        if [[ -z "${corpus_text}" ]]; then
            echo "  [SKIP] ${worm_id} — empty corpus"
            continue
        fi

        # Emit CORPUS JSON with module_id=jennifer
        python3 -c "
import json, sys
corpus = sys.stdin.read()
print(json.dumps({'worm_id': sys.argv[1], 'module_id': 'jennifer', 'corpus': corpus}))
" "${worm_id}" <<< "${corpus_text}" > "${corpus_file}"

        echo "  [EMIT] CORPUS_${worm_id}.json"
        COUNT=$((COUNT + 1))

        # Jitter to avoid thundering herd if called concurrently
        sleep "$(python3 -c "import random; print(round(random.uniform(0.5, 3.0), 1))")"

    done < <(find "${SOURCE_DIR}" -maxdepth 1 -name "*.json" -print0 2>/dev/null)
fi

echo "================================================================"
echo "[corpus-batch] Done — emitted: ${COUNT}, skipped: ${SKIPPED}."
echo "Next run: service-content will extract entities and write SEMANTIC_*.json."
