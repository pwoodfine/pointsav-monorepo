#!/usr/bin/env bash
# Weekly DataGraph cleanup — re-seeds taxonomy entities from current CSVs.
#
# Calls service-content HTTP API to reload topics and guides from disk,
# removing stale entities and replacing them with the current CSV state.
# Run weekly on Sunday (after corpus-threshold.py training check).
#
# Usage:
#   ./scripts/graph-cleanup.sh
#   SERVICE_CONTENT_URL=http://127.0.0.1:9081 ./scripts/graph-cleanup.sh
set -uo pipefail

SERVICE_CONTENT_URL="${SERVICE_CONTENT_URL:-http://127.0.0.1:9081}"
ONTOLOGY_DIR="${ONTOLOGY_DIR:-/srv/foundry/clones/project-intelligence/service-content/ontology}"

echo "[graph-cleanup] Targeting service-content at: ${SERVICE_CONTENT_URL}"

# Check service-content is up
if ! curl -sf "${SERVICE_CONTENT_URL}/healthz" > /dev/null 2>&1; then
    echo "[graph-cleanup] ERROR: service-content not reachable at ${SERVICE_CONTENT_URL}" >&2
    exit 1
fi

echo "[graph-cleanup] Reloading taxonomy from CSVs..."

# Reload guides via dedicated reload endpoint (re-reads disk, no body needed)
echo "[graph-cleanup] Reloading guides..."
guides_resp=$(curl -sf -X POST "${SERVICE_CONTENT_URL}/v1/config/guides/reload" \
    -H "Content-Type: application/json" 2>&1) && \
    echo "  guides: OK — ${guides_resp}" || \
    echo "  guides: FAIL — ${guides_resp}"

# Reload topics for each domain by POSTing the current CSV body
for domain in corporate documentation projects; do
    csv_path="${ONTOLOGY_DIR}/topics/topics_${domain}.csv"
    if [[ ! -f "${csv_path}" ]]; then
        echo "  topics/${domain}: SKIP (no CSV at ${csv_path})"
        continue
    fi
    resp=$(curl -sf -X POST "${SERVICE_CONTENT_URL}/v1/config/topics/${domain}" \
        -H "Content-Type: text/csv" \
        --data-binary "@${csv_path}" 2>&1) && \
        echo "  topics/${domain}: OK — ${resp}" || \
        echo "  topics/${domain}: FAIL — ${resp}"
done

echo "[graph-cleanup] Done."
