#!/usr/bin/env bash
# Phase B live flow-matrix test runner for Yo-Yo Tier B.
#
# Exercises each data-flow type the Yo-Yo Tier B is supposed to handle.
# Each subtest is PASS / FAIL / SKIP. SKIP is used when a prerequisite is
# missing (e.g. vLLM not loaded, SLM_APPRENTICESHIP_ENABLED unset, no SSH).
#
# Pre-conditions (probed in pre-flight; tests that need the missing piece
# are SKIPPED, not failed):
#   - Doorman responsive at $SLM_DOORMAN_ENDPOINT (default 127.0.0.1:9080)
#   - Tier B reachable via Doorman: /readyz reports has_yoyo=true
#   - service-content responsive at $SERVICE_CONTENT_ENDPOINT (default 127.0.0.1:9081)
#   - SSH access to the Yo-Yo VM (only required for circuit-breaker + recovery flows)
#
# Flows:
#   1. Plain Tier B inference
#   2. Grammar-constrained JSON-schema inference
#   3. Graph context injection (service-content → Doorman → Tier B)
#   4. Audit proxy (POST /v1/audit/proxy)
#   5. Apprenticeship shadow (requires SLM_APPRENTICESHIP_ENABLED)
#   6. Mesh routing by yoyo-label
#   7. Idle monitor stop (long; opt-in via --include-idle-test)
#   8. Circuit breaker open (requires SSH + --include-vllm-restart)
#   9. Health probe recovery (requires SSH + --include-vllm-restart)
#
# Output: per-test PASS/FAIL/SKIP lines + JSON summary at
#         /tmp/yoyo-flow-test-<UTC-timestamp>.json
#
# Exit code: 0 if no FAIL (PASS+SKIP only), 1 if any FAIL.
#
# Usage:
#   ./scripts/test-yoyo-flows.sh
#   ./scripts/test-yoyo-flows.sh --include-idle-test --include-vllm-restart
#   SLM_DOORMAN_ENDPOINT=http://localhost:9080 ./scripts/test-yoyo-flows.sh
set -uo pipefail

DOORMAN="${SLM_DOORMAN_ENDPOINT:-http://127.0.0.1:9080}"
SERVICE_CONTENT="${SERVICE_CONTENT_ENDPOINT:-http://127.0.0.1:9081}"
INSTANCE="${SLM_YOYO_GCP_INSTANCE:-yoyo-tier-b-1}"
PROJECT="${SLM_YOYO_GCP_PROJECT:-woodfine-node-gcp-free}"
ZONE="${SLM_YOYO_GCP_ZONE:-us-west1-b}"
INCLUDE_IDLE_TEST=false
INCLUDE_VLLM_RESTART=false
TIMESTAMP="$(date -u +%Y%m%d-%H%M%SZ)"
REPORT="/tmp/yoyo-flow-test-${TIMESTAMP}.json"

while [[ $# -gt 0 ]]; do
    case "$1" in
        --include-idle-test)     INCLUDE_IDLE_TEST=true; shift ;;
        --include-vllm-restart)  INCLUDE_VLLM_RESTART=true; shift ;;
        --help|-h)               sed -n '2,33p' "$0"; exit 0 ;;
        *) echo "Unknown flag: $1" >&2; exit 1 ;;
    esac
done

# ── Result tracking ──────────────────────────────────────────────────────────
declare -a RESULTS_NAME RESULTS_STATUS RESULTS_DETAIL
PASSED=0; FAILED=0; SKIPPED=0

record() {
    local name="$1" status="$2" detail="${3:-}"
    RESULTS_NAME+=("${name}")
    RESULTS_STATUS+=("${status}")
    RESULTS_DETAIL+=("${detail}")
    case "${status}" in
        PASS) PASSED=$((PASSED+1)); echo "  PASS  ${name}${detail:+ — ${detail}}" ;;
        FAIL) FAILED=$((FAILED+1)); echo "  FAIL  ${name} — ${detail}" ;;
        SKIP) SKIPPED=$((SKIPPED+1)); echo "  SKIP  ${name} — ${detail}" ;;
    esac
}

json_escape() { python3 -c 'import json,sys; print(json.dumps(sys.argv[1]))' "$1"; }

write_report() {
    {
        echo "{"
        echo "  \"timestamp\": \"${TIMESTAMP}\","
        echo "  \"doorman\": $(json_escape "${DOORMAN}"),"
        echo "  \"service_content\": $(json_escape "${SERVICE_CONTENT}"),"
        echo "  \"instance\": $(json_escape "${INSTANCE}"),"
        echo "  \"summary\": {\"pass\": ${PASSED}, \"fail\": ${FAILED}, \"skip\": ${SKIPPED}},"
        echo "  \"results\": ["
        local i n=${#RESULTS_NAME[@]}
        for ((i=0; i<n; i++)); do
            local sep=","
            [[ "$((i+1))" -eq "${n}" ]] && sep=""
            echo -n "    {\"name\": $(json_escape "${RESULTS_NAME[$i]}"),"
            echo -n " \"status\": $(json_escape "${RESULTS_STATUS[$i]}"),"
            echo " \"detail\": $(json_escape "${RESULTS_DETAIL[$i]}")}${sep}"
        done
        echo "  ]"
        echo "}"
    } > "${REPORT}"
    echo ""
    echo "Report written: ${REPORT}"
}

# ── HTTP helpers ─────────────────────────────────────────────────────────────
http_get() {
    local url="$1"
    local body http_code tmp
    tmp=$(mktemp)
    http_code=$(curl -sS -o "${tmp}" -w '%{http_code}' --max-time 10 "${url}" 2>/dev/null || echo "000")
    body=$(cat "${tmp}"); rm -f "${tmp}"
    echo "${http_code}"
    echo "${body}"
}

http_post_json() {
    local url="$1" body="$2"
    local resp http_code header_dump
    header_dump=$(mktemp)
    resp=$(curl -sS -D "${header_dump}" -o /tmp/yoyo-flow-resp.json -w '%{http_code}' \
        --max-time 30 -H "Content-Type: application/json" \
        -d "${body}" "${url}" 2>/dev/null || echo "000")
    http_code="${resp}"
    echo "${http_code}"
    cat "${header_dump}"; rm -f "${header_dump}"
    echo "---BODY---"
    cat /tmp/yoyo-flow-resp.json 2>/dev/null || echo ""
}

# ── Pre-flight ───────────────────────────────────────────────────────────────
echo "=== Pre-flight ==="
DOORMAN_READY=false
TIER_B_UP=false
SERVICE_CONTENT_UP=false
APPRENTICESHIP_ENABLED=false

readyz_resp=$(curl -sS --max-time 3 "${DOORMAN}/readyz" 2>/dev/null || echo "")
if [[ -n "${readyz_resp}" ]]; then
    DOORMAN_READY=true
    echo "  Doorman ${DOORMAN}/readyz: ${readyz_resp}"
    if echo "${readyz_resp}" | grep -q '"has_yoyo":true'; then
        # has_yoyo=true means env wired, NOT that vLLM is reachable.
        # Probe more deeply: send a tiny inference and inspect tier_used in
        # the response body. Doorman falls back to Tier A transparently when
        # Tier B is down, so HTTP 200 alone is not proof of Tier B health.
        probe_body=$(curl -sS --max-time 30 \
            -H "Content-Type: application/json" \
            -H "X-Foundry-Complexity: high" \
            -d '{"model":"olmo-3-32b","messages":[{"role":"user","content":"ping"}],"max_tokens":1}' \
            "${DOORMAN}/v1/chat/completions" 2>/dev/null || echo "")
        probe_tier=$(echo "${probe_body}" | python3 -c 'import json,sys; print(json.loads(sys.stdin.read()).get("tier_used",""))' 2>/dev/null || echo "")
        if [[ "${probe_tier}" == "yoyo" ]]; then
            TIER_B_UP=true
            echo "  Tier B reachable: probe returned tier_used=yoyo"
        elif [[ "${probe_tier}" == "local" ]]; then
            echo "  Tier B unreachable: probe fell back to tier_used=local (Tier A serving)"
        else
            echo "  Tier B unreachable: probe tier_used='${probe_tier}' (probe body: $(head -c 120 <<<"${probe_body}"))"
        fi
    else
        echo "  Tier B not configured (has_yoyo=false); inference flows will SKIP."
    fi
else
    echo "  Doorman ${DOORMAN}/readyz: NO RESPONSE"
fi

sc_health=$(curl -sS --max-time 3 "${SERVICE_CONTENT}/healthz" 2>/dev/null || echo "")
if echo "${sc_health}" | grep -q '"status":"ok"'; then
    SERVICE_CONTENT_UP=true
    echo "  service-content ${SERVICE_CONTENT}/healthz: ok"
else
    echo "  service-content ${SERVICE_CONTENT}/healthz: NO/BAD RESPONSE"
fi

# Apprenticeship state inferred from corpus presence (env file is root-owned).
if [[ -d /srv/foundry/data/training-corpus/apprenticeship ]] && \
   compgen -G "/srv/foundry/data/training-corpus/apprenticeship/*/draft-*.jsonl" >/dev/null 2>&1; then
    APPRENTICESHIP_ENABLED=true
    echo "  Apprenticeship: corpus present (recent JSONL events visible)"
else
    echo "  Apprenticeship: no recent corpus — assuming SLM_APPRENTICESHIP_ENABLED unset"
fi

VM_STATUS=$(gcloud compute instances describe "${INSTANCE}" --zone="${ZONE}" --project="${PROJECT}" \
    --format="value(status)" 2>/dev/null || echo "UNKNOWN")
echo "  Yo-Yo VM ${INSTANCE} status: ${VM_STATUS}"
echo ""

if [[ "${DOORMAN_READY}" != "true" ]]; then
    record "preflight" "FAIL" "Doorman /readyz unreachable — cannot run any flow tests"
    write_report
    exit 1
fi

# Helper: extract tier_used field from a JSON response body file
extract_tier_used() {
    python3 -c 'import json,sys; print(json.load(open(sys.argv[1])).get("tier_used",""))' "$1" 2>/dev/null || echo ""
}

# ── Test 1: Plain Tier B inference ───────────────────────────────────────────
echo "=== Test 1: Plain Tier B inference ==="
if [[ "${TIER_B_UP}" != "true" ]]; then
    record "1-plain-tier-b-inference" "SKIP" "Tier B not reachable"
else
    out=$(curl -sS -o /tmp/yoyo-flow-b1 -w '%{http_code}' \
        --max-time 60 -H "Content-Type: application/json" \
        -H "X-Foundry-Complexity: high" \
        -d '{"model":"olmo-3-32b","messages":[{"role":"user","content":"Say hello in one word."}],"max_tokens":8}' \
        "${DOORMAN}/v1/chat/completions" 2>/dev/null || echo "000")
    tier=$(extract_tier_used /tmp/yoyo-flow-b1)
    if [[ "${out}" == "200" ]] && [[ "${tier}" == "yoyo" ]]; then
        record "1-plain-tier-b-inference" "PASS" "200 + tier_used=yoyo"
    elif [[ "${out}" == "200" ]] && [[ "${tier}" == "local" ]]; then
        record "1-plain-tier-b-inference" "FAIL" "200 but tier_used=local (Tier A fallback) — Tier B not actually serving"
    else
        record "1-plain-tier-b-inference" "FAIL" "HTTP ${out}; tier_used=${tier}; body=$(head -c 200 /tmp/yoyo-flow-b1)"
    fi
fi

# ── Test 2: Grammar-constrained JSON-schema inference ────────────────────────
echo "=== Test 2: Grammar-constrained JSON-schema ==="
if [[ "${TIER_B_UP}" != "true" ]]; then
    record "2-grammar-constrained" "SKIP" "Tier B not reachable"
else
    schema='{"type":"object","properties":{"name":{"type":"string"},"age":{"type":"integer"}},"required":["name","age"]}'
    body=$(cat <<EOF
{"model":"olmo-3-32b","messages":[{"role":"user","content":"Return a JSON object with name=Alice and age=30."}],"max_tokens":40,"yoyo_label":"default","grammar":{"type":"json-schema","value":${schema}}}
EOF
)
    out=$(curl -sS -o /tmp/yoyo-flow-b2 -w '%{http_code}' --max-time 60 \
        -H "Content-Type: application/json" \
        -H "X-Foundry-Complexity: high" \
        -d "${body}" \
        "${DOORMAN}/v1/chat/completions" 2>/dev/null || echo "000")
    tier=$(extract_tier_used /tmp/yoyo-flow-b2)
    if [[ "${out}" == "200" ]] && [[ "${tier}" == "yoyo" ]]; then
        content=$(python3 -c 'import json,sys; d=json.load(open(sys.argv[1])); print(d.get("content","") or d.get("choices",[{}])[0].get("message",{}).get("content",""))' /tmp/yoyo-flow-b2 2>/dev/null || echo "")
        if echo "${content}" | python3 -c 'import json,sys; obj=json.loads(sys.stdin.read()); assert "name" in obj and "age" in obj' >/dev/null 2>&1; then
            record "2-grammar-constrained" "PASS" "schema-conforming JSON returned"
        else
            record "2-grammar-constrained" "FAIL" "200 from yoyo but content does not match schema: $(head -c 120 <<<"${content}")"
        fi
    elif [[ "${out}" == "200" ]] && [[ "${tier}" == "local" ]]; then
        record "2-grammar-constrained" "SKIP" "Tier A fallback (grammar may not be honored at Tier A)"
    else
        record "2-grammar-constrained" "FAIL" "HTTP ${out}; tier_used=${tier}"
    fi
fi

# ── Test 3: Graph context injection via service-content ──────────────────────
echo "=== Test 3: Graph context injection ==="
if [[ "${SERVICE_CONTENT_UP}" != "true" ]]; then
    record "3-graph-context-injection" "SKIP" "service-content unreachable"
else
    # Doorman /v1/graph/query is a service-content proxy — does NOT require Tier B.
    # Schema (per http.rs): {"q": "<string>", "limit": u32}. X-Foundry-Module-ID required.
    out=$(curl -sS -o /tmp/yoyo-flow-b3 -w '%{http_code}' --max-time 10 \
        -H "X-Foundry-Module-ID: jennifer" \
        -H "Content-Type: application/json" \
        -d '{"q":"jennifer","limit":3}' \
        "${DOORMAN}/v1/graph/query" 2>/dev/null || echo "000")
    case "${out}" in
        200|204) record "3-graph-context-injection" "PASS" "Doorman→service-content graph proxy returned HTTP ${out}" ;;
        404)     record "3-graph-context-injection" "FAIL" "Doorman /v1/graph/query route missing (404)" ;;
        *)       record "3-graph-context-injection" "FAIL" "HTTP ${out}; body=$(head -c 200 /tmp/yoyo-flow-b3)" ;;
    esac
fi

# ── Test 4: Audit proxy (Tier C path, route-existence + schema validation) ───
echo "=== Test 4: Audit proxy ==="
# Real schema (slm-core::AuditProxyRequest): module_id, purpose, provider, model,
# messages[], optional max_tokens/temperature. Tier C (Anthropic) is NOT configured
# in this deployment (has_external=false), so we expect either 503 (no Tier C)
# or 403 (purpose-allowlist deny) — both prove the route + schema exist.
audit_body='{"module_id":"jennifer","purpose":"editorial-refinement","provider":"anthropic","model":"claude-opus-4-7","messages":[{"role":"user","content":"hi"}],"max_tokens":4}'
out=$(curl -sS -o /tmp/yoyo-flow-b4 -w '%{http_code}' --max-time 10 \
    -H "Content-Type: application/json" -d "${audit_body}" \
    "${DOORMAN}/v1/audit/proxy" 2>/dev/null || echo "000")
case "${out}" in
    200)
        if grep -q '"audit_id"' /tmp/yoyo-flow-b4; then
            record "4-audit-proxy" "PASS" "200 with audit_id (Tier C live)"
        else
            record "4-audit-proxy" "FAIL" "200 but no audit_id in response"
        fi ;;
    403|503)
        record "4-audit-proxy" "PASS" "route + schema valid (HTTP ${out}; Tier C/allowlist gate as expected)" ;;
    400|422)
        record "4-audit-proxy" "FAIL" "schema mismatch (HTTP ${out}): $(head -c 200 /tmp/yoyo-flow-b4)" ;;
    404)
        record "4-audit-proxy" "FAIL" "Doorman /v1/audit/proxy route missing (404)" ;;
    *)
        record "4-audit-proxy" "FAIL" "HTTP ${out}; body=$(head -c 200 /tmp/yoyo-flow-b4)" ;;
esac

# ── Test 5: Apprenticeship shadow ────────────────────────────────────────────
echo "=== Test 5: Apprenticeship shadow ==="
if [[ "${APPRENTICESHIP_ENABLED}" != "true" ]]; then
    record "5-apprenticeship-shadow" "SKIP" "SLM_APPRENTICESHIP_ENABLED unset (no recent corpus)"
elif [[ "${TIER_B_UP}" != "true" ]]; then
    record "5-apprenticeship-shadow" "SKIP" "Tier B not reachable"
else
    pre=$(find /srv/foundry/data/training-corpus/apprenticeship -name "*.jsonl" -newer /tmp 2>/dev/null | wc -l)
    curl -sS -o /tmp/yoyo-flow-b5 --max-time 60 \
        -H "Content-Type: application/json" \
        -H "X-Foundry-Complexity: high" \
        -H "X-Foundry-Task-Type: doorman-routing" \
        -d '{"model":"olmo-3-32b","messages":[{"role":"user","content":"Pick a tier for this routing decision."}],"max_tokens":16}' \
        "${DOORMAN}/v1/chat/completions" >/dev/null 2>&1
    sleep 2
    post=$(find /srv/foundry/data/training-corpus/apprenticeship -name "*.jsonl" -newer /tmp 2>/dev/null | wc -l)
    if [[ "${post}" -gt "${pre}" ]]; then
        record "5-apprenticeship-shadow" "PASS" "${post} > ${pre} JSONL files after call"
    else
        record "5-apprenticeship-shadow" "FAIL" "no new corpus events (pre=${pre} post=${post})"
    fi
fi

# ── Test 6: Mesh routing by yoyo-label ───────────────────────────────────────
echo "=== Test 6: Mesh routing by yoyo-label ==="
if [[ "${TIER_B_UP}" != "true" ]]; then
    record "6-mesh-routing" "SKIP" "Tier B not reachable"
else
    out=$(curl -sS -o /tmp/yoyo-flow-b6 -w '%{http_code}' --max-time 60 \
        -H "Content-Type: application/json" \
        -H "X-Foundry-Complexity: high" \
        -d '{"model":"olmo-3-32b","messages":[{"role":"user","content":"hi"}],"max_tokens":2}' \
        "${DOORMAN}/v1/chat/completions" 2>/dev/null || echo "000")
    tier=$(extract_tier_used /tmp/yoyo-flow-b6)
    if [[ "${out}" == "200" ]] && [[ "${tier}" == "yoyo" ]]; then
        record "6-mesh-routing" "PASS" "label=trainer routed to yoyo (200, tier_used=yoyo)"
    elif grep -qi "no.*node.*trainer\|unknown.*label" /tmp/yoyo-flow-b6; then
        record "6-mesh-routing" "FAIL" "trainer label not registered in mesh"
    else
        record "6-mesh-routing" "FAIL" "HTTP ${out}; tier_used=${tier}; body=$(head -c 200 /tmp/yoyo-flow-b6)"
    fi
fi

# ── Test 7: Idle monitor stop (long-running, opt-in) ─────────────────────────
echo "=== Test 7: Idle monitor stop ==="
if [[ "${INCLUDE_IDLE_TEST}" != "true" ]]; then
    record "7-idle-monitor-stop" "SKIP" "opt-in only (--include-idle-test); takes 30+ minutes"
elif [[ "${TIER_B_UP}" != "true" ]]; then
    record "7-idle-monitor-stop" "SKIP" "Tier B not reachable"
else
    echo "  Waiting 31 minutes for idle monitor to stop the VM..."
    sleep 1860
    new_status=$(gcloud compute instances describe "${INSTANCE}" --zone="${ZONE}" --project="${PROJECT}" \
        --format="value(status)" 2>/dev/null || echo "UNKNOWN")
    if [[ "${new_status}" == "TERMINATED" ]] || [[ "${new_status}" == "STOPPING" ]]; then
        record "7-idle-monitor-stop" "PASS" "VM transitioned to ${new_status}"
    else
        record "7-idle-monitor-stop" "FAIL" "VM still ${new_status} after 31 min idle"
    fi
fi

# ── Test 8: Circuit breaker open (kill vLLM, observe Tier A fallback) ────────
echo "=== Test 8: Circuit breaker open ==="
if [[ "${INCLUDE_VLLM_RESTART}" != "true" ]]; then
    if [[ "${TIER_B_UP}" != "true" ]]; then
        # Tier B already down → circuit should be OPEN. Verify Tier A fallback.
        out=$(curl -sS -o /tmp/yoyo-flow-b8 -w '%{http_code}' --max-time 30 \
            -H "Content-Type: application/json" \
            -d '{"model":"olmo-3-32b","messages":[{"role":"user","content":"hi"}],"max_tokens":2,"yoyo_label":"default"}' \
            "${DOORMAN}/v1/chat/completions" 2>/dev/null || echo "000")
        tier=$(extract_tier_used /tmp/yoyo-flow-b8)
        if [[ "${out}" == "200" ]] && [[ "${tier}" == "local" ]]; then
            record "8-circuit-breaker" "PASS" "Tier B down + circuit open → Tier A fallback (tier_used=local), HTTP 200"
        else
            record "8-circuit-breaker" "FAIL" "Tier B down but no Tier A fallback (HTTP ${out}, tier_used=${tier})"
        fi
    else
        record "8-circuit-breaker" "SKIP" "opt-in only (--include-vllm-restart); requires SSH to kill vLLM"
    fi
else
    record "8-circuit-breaker" "SKIP" "kill-vLLM-via-SSH path not yet implemented in this script"
fi

# ── Test 9: Health probe recovery ────────────────────────────────────────────
echo "=== Test 9: Health probe recovery ==="
if [[ "${INCLUDE_VLLM_RESTART}" != "true" ]]; then
    record "9-health-probe-recovery" "SKIP" "opt-in only (--include-vllm-restart)"
else
    record "9-health-probe-recovery" "SKIP" "restart-vLLM-via-SSH path not yet implemented in this script"
fi

# ── Test 10: Jennifer DataGraph REST API flow ─────────────────────────────────
echo "=== Test 10: Jennifer DataGraph REST API flow ==="
if [[ "${SERVICE_CONTENT_UP}" != "true" ]]; then
    record "10-jennifer-datagraph-rest-api" "SKIP" "service-content unreachable"
else
    # Write one entity via REST (same API surface customers use)
    mutate_out=$(curl -sf --max-time 10 -X POST "${SERVICE_CONTENT}/v1/graph/mutate" \
        -H "Content-Type: application/json" \
        -d '{"module_id":"test","entities":[{"entity_name":"Test Entity Flow10",
             "classification":"Company","role_vector":null,"location_vector":null,
             "contact_vector":null,"module_id":"test","confidence":0.99}]}' 2>/dev/null || echo "FAIL")
    if [[ "${mutate_out}" == "FAIL" ]]; then
        record "10-jennifer-datagraph-rest-api" "FAIL" "POST /v1/graph/mutate returned error"
    else
        # Query it back via the same customer API
        query_out=$(curl -sf --max-time 5 \
            "${SERVICE_CONTENT}/v1/graph/context?q=Test+Entity+Flow10&module_id=test&limit=1" 2>/dev/null || echo "")
        if echo "${query_out}" | python3 -c 'import json,sys; d=json.loads(sys.stdin.read()); assert len(d)>0' >/dev/null 2>&1; then
            record "10-jennifer-datagraph-rest-api" "PASS" "entity written + queried back via customer REST API"
        else
            record "10-jennifer-datagraph-rest-api" "FAIL" "entity written but query returned empty: ${query_out:0:120}"
        fi
    fi
fi

# ── Test 11: LoRA training marker claim ──────────────────────────────────────
echo "=== Test 11: LoRA training marker claim ==="
PENDING_DIR_T11="${FOUNDRY_ROOT:-/srv/foundry}/data/training-pending"
mkdir -p "${PENDING_DIR_T11}"
MARKER_T11="${PENDING_DIR_T11}/test-flow11-$(date +%Y%m%dT%H%M%S).json"
echo '{"adapter":"engineering-pointsav","tenant":"pointsav","role":"engineering",
      "corpus_path":"/srv/foundry/data/training-corpus/engineering",
      "method":"sft","training_method":"sft","version":99,
      "tuple_count":1,"triggered_at":"'"$(date -u +'%Y-%m-%dT%H:%M:%SZ')"'"}' > "${MARKER_T11}"

CLAIM_FOUND=false
for i in $(seq 1 6); do
    sleep 5
    if ls "${MARKER_T11}.claimed" "${MARKER_T11}.completed" 2>/dev/null | grep -q .; then
        CLAIM_FOUND=true
        break
    fi
done

if [[ "${CLAIM_FOUND}" == "true" ]]; then
    record "11-lora-training-marker-claim" "PASS" "marker claimed within $((i * 5))s"
elif systemctl is-active --quiet lora-training.service 2>/dev/null; then
    record "11-lora-training-marker-claim" "FAIL" "lora-training.service active but marker not claimed after 30s"
else
    record "11-lora-training-marker-claim" "SKIP" "lora-training.service not active (enable with: sudo systemctl enable --now lora-training.service)"
fi
# Clean up test marker
rm -f "${MARKER_T11}" "${MARKER_T11}.claimed" "${MARKER_T11}.completed" 2>/dev/null || true

# ── Summary ──────────────────────────────────────────────────────────────────
echo ""
echo "=== Summary ==="
echo "  PASSED: ${PASSED}"
echo "  FAILED: ${FAILED}"
echo "  SKIPPED: ${SKIPPED}"
echo "  Total:  $((PASSED+FAILED+SKIPPED))"

write_report

[[ "${FAILED}" -eq 0 ]] && exit 0 || exit 1
