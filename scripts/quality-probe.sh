#!/usr/bin/env bash
# quality-probe.sh — Live flow + quality check for service-content and service-slm.
# Run from the archive root: bash scripts/quality-probe.sh
# No Tier B required — Tier A only.

set -euo pipefail
ARCHIVE_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
CONTENT_PORT=9081
DOORMAN_PORT=9080

echo "========================================================"
echo "  Flow + Quality Probe — project-totebox"
echo "  $(date -u '+%Y-%m-%dT%H:%M:%SZ')"
echo "========================================================"

# ── A: Doorman health (service-slm :9080) ────────────────────────────────────
echo ""
echo "── A: Doorman health (service-slm :${DOORMAN_PORT})"
DOORMAN_HEALTH=$(curl -sf --max-time 5 "http://127.0.0.1:${DOORMAN_PORT}/healthz" 2>&1) || {
    echo "  FAIL: Doorman not reachable on :${DOORMAN_PORT}"
    DOORMAN_HEALTH="unreachable"
}
echo "  healthz: ${DOORMAN_HEALTH}"
DOORMAN_READYZ=$(curl -sf --max-time 5 "http://127.0.0.1:${DOORMAN_PORT}/readyz" 2>&1) || {
    DOORMAN_READYZ="not available"
}
echo "  readyz:  ${DOORMAN_READYZ}"

# ── B: DataGraph quality (service-content :9081) ─────────────────────────────
echo ""
echo "── B: DataGraph quality (service-content :${CONTENT_PORT})"
CONTENT_HEALTH=$(curl -sf --max-time 5 "http://127.0.0.1:${CONTENT_PORT}/healthz" 2>&1) || {
    echo "  FAIL: service-content not reachable on :${CONTENT_PORT}"
    CONTENT_HEALTH='{"status":"unreachable","entity_count":0}'
}
echo "  healthz: ${CONTENT_HEALTH}"
ENTITY_COUNT=$(echo "${CONTENT_HEALTH}" | grep -o '"entity_count":[0-9]*' | grep -o '[0-9]*' || echo "0")
echo "  entity_count: ${ENTITY_COUNT}  (baseline 2026-06-21: 7445; baseline 2026-06-26: 11931)"

# Sample entities for jennifer module to check fill-rates
echo ""
echo "  Sample: jennifer module (first 5 entities matching 'Woodfine')"
SAMPLE=$(curl -sf --max-time 5 \
    "http://127.0.0.1:${CONTENT_PORT}/v1/graph/context?module_id=jennifer&q=Woodfine&limit=5" \
    2>&1) || SAMPLE="[]"
echo "${SAMPLE}" | python3 -c "
import json, sys
try:
    ents = json.load(sys.stdin)
    if not ents:
        print('  (no entities returned for module=jennifer q=Woodfine)')
    else:
        filled_role = sum(1 for e in ents if e.get('role_vector'))
        filled_loc  = sum(1 for e in ents if e.get('location_vector'))
        print(f'  {len(ents)} entities returned')
        print(f'  role_vector populated: {filled_role}/{len(ents)}')
        print(f'  location_vector populated: {filled_loc}/{len(ents)}')
        for e in ents[:3]:
            print(f'  - {e[\"entity_name\"]} ({e[\"classification\"]}) rv={e.get(\"role_vector\") or \"NULL\"!r:.40}')
except Exception as ex:
    print(f'  parse error: {ex}')
" 2>/dev/null || echo "  (python3 parse failed — raw: ${SAMPLE})"

# Recent fill-rate log from journalctl
echo ""
echo "  Recent fill-rate telemetry (last 5 log lines):"
journalctl -u local-content.service --no-pager -n 50 2>/dev/null \
    | grep "\[graph\] upserted" | tail -5 \
    | sed 's/^/  /' \
    || echo "  (no [graph] upserted log lines found)"

# ── C: Tier A inference quality ───────────────────────────────────────────────
echo ""
echo "── C: Tier A inference quality (POST :${DOORMAN_PORT}/v1/chat/completions)"
echo "  (wall timeout: 150s — Tier A CPU inference; under extraction load can exceed 90s)"
INFER_RESULT=$(curl -sf --max-time 150 \
    -X POST "http://127.0.0.1:${DOORMAN_PORT}/v1/chat/completions" \
    -H "Content-Type: application/json" \
    -H "X-Foundry-Module-ID: jennifer" \
    -d '{"model":"local","messages":[{"role":"user","content":"Who is Jennifer Woodfine and what is her role at Woodfine Management Corp?"}],"max_tokens":80}' \
    -D /tmp/infer-headers.txt \
    2>&1) || {
    echo "  FAIL or TIMEOUT: ${INFER_RESULT}"
    INFER_RESULT=""
}
if [ -n "${INFER_RESULT}" ]; then
    TIER_USED=$(grep -i "x-foundry-tier-used" /tmp/infer-headers.txt 2>/dev/null | tr -d '\r' | awk '{print $2}' || echo "unknown")
    echo "  X-Foundry-Tier-Used: ${TIER_USED}"
    echo "${INFER_RESULT}" | python3 -c "
import json, sys
try:
    r = json.load(sys.stdin)
    content = r.get('choices', [{}])[0].get('message', {}).get('content', '')
    print(f'  response: {content[:200]!r}')
    grounded = any(w in content.lower() for w in ['woodfine', 'jennifer', 'management'])
    print(f'  entity-grounded: {grounded}')
except Exception as ex:
    print(f'  parse error: {ex}')
    print(f'  raw: {sys.stdin.read()[:300]}')
" 2>/dev/null || echo "  raw: ${INFER_RESULT:0:300}"
fi
rm -f /tmp/infer-headers.txt

# ── D: Corpus quality snapshot ────────────────────────────────────────────────
echo ""
echo "── D: Corpus quality snapshot"
CORPUS_ROOT="/srv/foundry/data/training-corpus"
if [ -d "${CORPUS_ROOT}" ]; then
    SFT_COUNT=$(find "${CORPUS_ROOT}/apprenticeship" -name "*.jsonl" 2>/dev/null | wc -l || echo "0")
    DPO_COUNT=$(find "${CORPUS_ROOT}/feedback" -name "*.jsonl" 2>/dev/null | wc -l || echo "0")
    echo "  SFT records (apprenticeship/): ${SFT_COUNT}"
    echo "  DPO pairs   (feedback/):        ${DPO_COUNT}"
else
    echo "  training-corpus dir not found at ${CORPUS_ROOT}"
fi
STAGES_FILE="/srv/foundry/data/apprenticeship/stages.json"
if [ -f "${STAGES_FILE}" ]; then
    echo "  apprenticeship stages: $(cat "${STAGES_FILE}" | tr '\n' ' ')"
else
    echo "  stages.json: not found at ${STAGES_FILE}"
fi

# ── E: Summary ────────────────────────────────────────────────────────────────
echo ""
echo "── E: Summary vs BRIEF-flow-quality-audit baselines"
echo "  entity_count:  ${ENTITY_COUNT}  (baseline 2026-06-21: 7445; delta: +$((ENTITY_COUNT - 7445)) if positive)"
echo "  fill-rates:    see section B above (baseline role=4.9% loc=7.7% contact=3.5%)"
echo "  edge count:    check journalctl -u local-content.service | grep upsert_edges"
echo "  alias count:   check journalctl -u local-content.service | grep entity_aliases"
echo ""
echo "========================================================"
echo "  Probe complete. Run cargo test -p service-content to verify unit + pipeline tests."
echo "========================================================"
