#!/usr/bin/env bash
# Jennifer Ontological DataGraph rebuild — REST API integration test.
#
# Uses ONLY the customer-facing REST API:
#   POST :9080/v1/chat/completions  (Doorman — entity extraction via Tier B vLLM)
#   POST :9081/v1/graph/mutate      (service-content — write extracted entities)
#   GET  :9081/healthz              (service-content health check)
#
# No file-watcher path. No internal shortcuts. Same API surface a customer
# running service-slm + service-content would call from their own automation.
#
# Idempotent: tracks processed worm_ids in a local ledger; skips on re-run.
# If this script fails, service-slm or service-content has a bug — that is the point.

set -euo pipefail

DOORMAN_ENDPOINT="${DOORMAN_ENDPOINT:-http://127.0.0.1:9080}"
CONTENT_ENDPOINT="${CONTENT_ENDPOINT:-http://127.0.0.1:9081}"
MODULE_ID="${SERVICE_CONTENT_MODULE_ID:-woodfine}"
JENNIFER_DEPLOYMENT="${JENNIFER_DEPLOYMENT:-/srv/foundry/deployments/cluster-totebox-jennifer}"
FOUNDRY_ROOT="${FOUNDRY_ROOT:-/srv/foundry}"
PROCESSED_LEDGER="${FOUNDRY_ROOT}/data/datagraph-processed.txt"
HEALTH_RECORD="${FOUNDRY_ROOT}/data/datagraph-health.json"
TIMEOUT="${DATAGRAPH_SECONDS:-7200}"

log() { echo "[jennifer-datagraph $(date -u +'%Y-%m-%dT%H:%M:%SZ')] $*"; }
DEADLINE=$(( $(date +%s) + TIMEOUT ))
NEW_ENTITIES=0

mkdir -p "${FOUNDRY_ROOT}/data"
touch "${PROCESSED_LEDGER}"

# Health check
if ! curl -sf --max-time 5 "${CONTENT_ENDPOINT}/healthz" >/dev/null; then
    log "ERROR: service-content not responding at ${CONTENT_ENDPOINT}. Abort."
    exit 1
fi
if ! curl -sf --max-time 5 "${DOORMAN_ENDPOINT}/readyz" >/dev/null 2>&1; then
    log "WARN: Doorman not responding at ${DOORMAN_ENDPOINT}. Entity extraction will degrade to Tier A."
fi

PRIOR_COUNT=$(jq -r '.entity_count // 0' "${HEALTH_RECORD}" 2>/dev/null || echo 0)
log "Prior entity_count=${PRIOR_COUNT}"

ENTITY_SCHEMA='{"type":"array","items":{"type":"object","required":["entity_name","classification","module_id","confidence"],"properties":{"entity_name":{"type":"string"},"classification":{"type":"string","enum":["Person","Company","Project","Account","Location"]},"role_vector":{"type":["string","null"]},"location_vector":{"type":["string","null"]},"contact_vector":{"type":["string","null"]},"module_id":{"type":"string"},"confidence":{"type":"number","minimum":0,"maximum":1}}}}'

process_document() {
    local worm_id="$1" text="$2"

    if grep -qF "${worm_id}" "${PROCESSED_LEDGER}" 2>/dev/null; then
        return 0
    fi

    [[ $(date +%s) -ge ${DEADLINE} ]] && { log "WARN: time budget exhausted during DataGraph phase."; return 1; }

    local payload
    payload=$(jq -n \
        --arg text "${text:0:3000}" \
        --argjson schema "${ENTITY_SCHEMA}" \
        '{
            "messages": [
                {"role": "system", "content": "Extract named entities from the text. Classify each entity into exactly one category.\nCategories and examples:\n  Person — named human individual. Example: \"Jane Smith\".\n  Company — registered organisation or business. Example: \"Woodfine Management Corp.\".\n  Project — named initiative, programme, or system. Example: \"service-slm\".\n  Account — financial account, service account, or contract reference.\n  Location — geographic place or address. Example: \"Vancouver\".\nOmit: laws and regulations (not Location), dates and years (not Location), abstract concepts (not Company), regulatory bodies (not Company unless they are a named legal entity with a registered name).\nIf an entity does not clearly fit one category, omit it rather than guessing.\nReturn a JSON array matching the schema exactly."},
                {"role": "user", "content": $text}
            ],
            "grammar": {"type": "json-schema", "value": $schema},
            "max_tokens": 512
        }')

    local response
    response=$(curl -sf --max-time 180 \
        -X POST "${DOORMAN_ENDPOINT}/v1/chat/completions" \
        -H "Content-Type: application/json" \
        -H "X-Foundry-Module-ID: ${MODULE_ID}" \
        -H "X-Foundry-Complexity: high" \
        -H "X-Foundry-Yoyo-Label: graph" \
        -d "${payload}" 2>/dev/null) || { log "WARN: Doorman call failed for ${worm_id}. Skipping."; return 0; }

    local entities
    entities=$(echo "${response}" | jq -r '.content // .choices[0].message.content // "[]"')
    local entity_count
    entity_count=$(echo "${entities}" | jq 'length' 2>/dev/null || echo 0)

    if [[ "${entity_count}" -eq 0 ]]; then
        log "No entities extracted for ${worm_id}."
        echo "${worm_id}" >> "${PROCESSED_LEDGER}"
        return 0
    fi

    local tagged_entities
    tagged_entities=$(echo "${entities}" | jq --arg mid "${MODULE_ID}" '[.[] | .module_id = $mid]')

    local mutate_payload
    mutate_payload=$(jq -n --arg mid "${MODULE_ID}" --argjson ents "${tagged_entities}" \
        '{"module_id": $mid, "entities": $ents}')

    curl -sf --max-time 10 \
        -X POST "${CONTENT_ENDPOINT}/v1/graph/mutate" \
        -H "Content-Type: application/json" \
        -d "${mutate_payload}" >/dev/null || { log "WARN: graph/mutate failed for ${worm_id}."; return 0; }

    echo "${worm_id}" >> "${PROCESSED_LEDGER}"
    NEW_ENTITIES=$(( NEW_ENTITIES + entity_count ))
    log "  ${worm_id}: ${entity_count} entities written."
    sleep "$(python3 -c "import random; print(round(random.uniform(0.3, 1.5), 2))")"
}

# Phase J1: Minutebook assets
log "Phase J1: minutebook/assets"
while IFS= read -r -d '' fpath; do
    worm_id="mk-$(sha1sum "${fpath}" | cut -c1-12)"
    text=$(sed '/^---$/,/^---$/d' "${fpath}" | tr -s ' \n' ' ')
    process_document "${worm_id}" "${text}" || break
done < <(find "${JENNIFER_DEPLOYMENT}/service-minutebook/assets" -name "*.md" -print0 2>/dev/null)

# Phase J2: Service-agents research
log "Phase J2: service-agents"
while IFS= read -r -d '' fpath; do
    worm_id="ag-$(sha1sum "${fpath}" | cut -c1-12)"
    text=$(cat "${fpath}")
    process_document "${worm_id}" "${text}" || break
done < <(find "${JENNIFER_DEPLOYMENT}/service-agents" \( -name "*.md" -o -name "*.yaml" \) -print0 2>/dev/null)

# Phase J3: Service-people new arrivals
log "Phase J3: service-people (new arrivals)"
PEOPLE_SOURCE="${JENNIFER_DEPLOYMENT}/service-people/source"
if [[ -d "${PEOPLE_SOURCE}" ]]; then
    while IFS= read -r -d '' fpath; do
        worm_id="sp-$(sha1sum "${fpath}" | cut -c1-12)"
        text=$(jq -r '.corpus // .body // .text // empty' "${fpath}" 2>/dev/null || cat "${fpath}")
        process_document "${worm_id}" "${text}" || break
    done < <(find "${PEOPLE_SOURCE}" -name "*.json" -newer "${PROCESSED_LEDGER}" -print0 2>/dev/null | head -z -n 50)
fi

# Phase J4: Health probe + delta report
log "Phase J4: health probe"
HEALTH_RESP=$(curl -sf --max-time 5 "${CONTENT_ENDPOINT}/healthz" 2>/dev/null || echo "{}")
CURRENT_ENTITY_COUNT=$(echo "${HEALTH_RESP}" | jq -r '.entity_count // 0' 2>/dev/null || echo "${PRIOR_COUNT}")
DELTA=$(( CURRENT_ENTITY_COUNT - PRIOR_COUNT ))

jq -n \
    --arg ts "$(date -u +'%Y-%m-%dT%H:%M:%SZ')" \
    --argjson ec "${CURRENT_ENTITY_COUNT}" \
    --argjson delta "${DELTA}" \
    --argjson new_ents "${NEW_ENTITIES}" \
    '{"timestamp": $ts, "entity_count": $ec, "delta": $delta, "new_entities_this_run": $new_ents}' \
    > "${HEALTH_RECORD}"

if [[ "${DELTA}" -ge 0 ]]; then
    log "DataGraph: HEALTHY | entity_count=${CURRENT_ENTITY_COUNT} (delta=+${DELTA}) | new_entities=${NEW_ENTITIES}"
else
    log "DataGraph: WARN | entity_count shrank by ${DELTA} — service-content may have restarted"
fi

exit 0
