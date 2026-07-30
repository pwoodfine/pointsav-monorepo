#!/usr/bin/env bash
# smoke-test-doorman.sh — advisory smoke-test for the Doorman HTTP server.
#
# Hits every endpoint and reports PASS/FAIL without blocking the deploy.
# Exit 0 if all tests ran (advisory mode); exit 1 only if the script itself crashed.
#
# Usage:
#   DOORMAN_URL=http://127.0.0.1:9080 ./scripts/smoke-test-doorman.sh
#
# Default DOORMAN_URL: http://127.0.0.1:9080

set -euo pipefail

DOORMAN_URL="${DOORMAN_URL:-http://127.0.0.1:9080}"

PASS=0
FAIL=0
TOTAL=0

# ─── helpers ─────────────────────────────────────────────────────────────────

_pass() {
    PASS=$(( PASS + 1 ))
    TOTAL=$(( TOTAL + 1 ))
    echo "  [PASS] $1"
}

_fail() {
    FAIL=$(( FAIL + 1 ))
    TOTAL=$(( TOTAL + 1 ))
    echo "  [FAIL] $1"
}

# Run a single test.
# Args: <test-name> <expected-status> <actual-status> <body-preview>
_check() {
    local name="$1"
    local expected="$2"
    local actual="$3"
    local preview="$4"
    echo ""
    echo "TEST: $name"
    echo "  expected: HTTP $expected  got: HTTP $actual"
    echo "  body: ${preview:0:120}"
    if [[ "$actual" == "$expected" ]]; then
        _pass "$name"
    else
        _fail "$name"
    fi
}

# Perform an HTTP request via curl.
# Outputs "<STATUS_CODE>|<BODY>" to stdout (body may be truncated by caller).
# Curl errors (connection refused, timeout) emit a synthetic "000" status
# with an error message body so the test script can report FAIL gracefully.
_curl() {
    local method="$1"
    local url="$2"
    shift 2
    local extra_args=("$@")

    local tmpfile
    tmpfile="$(mktemp)"
    local http_code
    if http_code="$(curl -s -o "$tmpfile" -w "%{http_code}" \
            --connect-timeout 5 --max-time 15 \
            -X "$method" "$url" "${extra_args[@]}" 2>/dev/null)"; then
        local body
        body="$(cat "$tmpfile")"
        rm -f "$tmpfile"
        echo "${http_code}|${body}"
    else
        local curl_exit="$?"
        rm -f "$tmpfile"
        echo "000|curl error (exit ${curl_exit}) — server may be unreachable"
    fi
}

# ─── tests ───────────────────────────────────────────────────────────────────

echo "=== Doorman smoke-test ==="
echo "URL: $DOORMAN_URL"
echo "Date: $(date -u +%Y-%m-%dT%H:%M:%SZ)"
echo ""

# 1. GET /healthz — expect 200
{
    IFS='|' read -r status body <<< "$(_curl GET "$DOORMAN_URL/healthz")"
    _check "GET /healthz → 200" "200" "$status" "$body"
}

# 2. GET /readyz — expect 200 with tier flags in body
{
    IFS='|' read -r status body <<< "$(_curl GET "$DOORMAN_URL/readyz")"
    _check "GET /readyz → 200 with tier flags" "200" "$status" "$body"
    # Advisory: confirm body contains expected fields
    if [[ "$status" == "200" ]]; then
        for field in "ready" "has_local" "has_yoyo" "has_external"; do
            if ! echo "$body" | grep -q "\"$field\""; then
                echo "    WARNING: readyz body missing field: $field"
            fi
        done
    fi
}

# 3. GET /v1/contract — expect 200 with doorman_version field
{
    IFS='|' read -r status body <<< "$(_curl GET "$DOORMAN_URL/v1/contract")"
    _check "GET /v1/contract → 200 with version" "200" "$status" "$body"
    if [[ "$status" == "200" ]]; then
        if ! echo "$body" | grep -q "doorman_version"; then
            echo "    WARNING: contract body missing field: doorman_version"
        fi
    fi
}

# 4. POST /v1/chat/completions — expect 200 (Tier A) or 503 if Tier A unreachable
# This test is advisory: 503 with a Tier A connection error is not a deploy failure,
# it means local-slm.service is not running. The test reports PASS on 200 only.
{
    local_payload='{"messages":[{"role":"user","content":"Say hello in one word."}],"max_tokens":5}'
    IFS='|' read -r status body <<< "$(
        _curl POST "$DOORMAN_URL/v1/chat/completions" \
            -H 'Content-Type: application/json' \
            -H 'X-Foundry-Module-ID: foundry' \
            -d "$local_payload"
    )"
    echo ""
    echo "TEST: POST /v1/chat/completions → 200 (Tier A round-trip)"
    echo "  expected: HTTP 200  got: HTTP $status"
    echo "  body: ${body:0:120}"
    if [[ "$status" == "200" ]]; then
        _pass "POST /v1/chat/completions → 200"
    elif [[ "$status" == "502" ]] || [[ "$status" == "503" ]]; then
        FAIL=$(( FAIL + 1 ))
        TOTAL=$(( TOTAL + 1 ))
        echo "  [FAIL] Tier A unreachable — check local-slm.service status"
    else
        _fail "POST /v1/chat/completions → 200"
    fi
}

# 5. POST /v1/audit/proxy with valid purpose → expect 503 unconfigured
# 503 is CORRECT when SLM_TIER_C_* env vars are not set. This confirms the
# endpoint is live and returning the expected "unconfigured" message.
{
    proxy_payload='{"module_id":"foundry","provider":"anthropic","purpose":"editorial-refinement","messages":[{"role":"user","content":"test"}],"model":"claude-3-haiku-20240307"}'
    IFS='|' read -r status body <<< "$(
        _curl POST "$DOORMAN_URL/v1/audit/proxy" \
            -H 'Content-Type: application/json' \
            -d "$proxy_payload"
    )"
    _check "POST /v1/audit/proxy (valid, unconfigured) → 503" "503" "$status" "$body"
}

# 6. POST /v1/audit/capture with valid prose-edit event → expect 200
{
    audit_id="test-$(date +%s)-$(( RANDOM % 9999 ))"
    capture_payload="{\"module_id\":\"foundry\",\"audit_id\":\"${audit_id}\",\"event_type\":\"prose-edit\",\"source\":\"smoke-test-doorman.sh\",\"status\":\"completed\",\"event_at\":\"$(date -u +%Y-%m-%dT%H:%M:%SZ)\",\"payload\":{\"note\":\"smoke-test tuple\"},\"caller_request_id\":\"smoke-test-1\"}"
    IFS='|' read -r status body <<< "$(
        _curl POST "$DOORMAN_URL/v1/audit/capture" \
            -H 'Content-Type: application/json' \
            -d "$capture_payload"
    )"
    _check "POST /v1/audit/capture (valid prose-edit) → 200" "200" "$status" "$body"
    if [[ "$status" == "200" ]]; then
        if ! echo "$body" | grep -q "captured"; then
            echo "    WARNING: audit/capture response missing 'captured' status field"
        fi
    fi
}

# 7. POST /v1/audit/proxy with unallowlisted purpose → expect 403
{
    bad_purpose_payload='{"module_id":"foundry","provider":"anthropic","purpose":"unauthorized-use-case","messages":[{"role":"user","content":"test"}],"model":"claude-3-haiku-20240307"}'
    IFS='|' read -r status body <<< "$(
        _curl POST "$DOORMAN_URL/v1/audit/proxy" \
            -H 'Content-Type: application/json' \
            -d "$bad_purpose_payload"
    )"
    _check "POST /v1/audit/proxy (unallowlisted purpose) → 403" "403" "$status" "$body"
}

# 8. POST /v1/audit/capture with unknown event_type → expect 400
{
    bad_event_payload="{\"module_id\":\"foundry\",\"audit_id\":\"test-bad-event\",\"event_type\":\"not-a-real-event-type\",\"source\":\"smoke-test\",\"status\":\"ok\",\"event_at\":\"$(date -u +%Y-%m-%dT%H:%M:%SZ)\",\"payload\":{},\"caller_request_id\":\"smoke-test-2\"}"
    IFS='|' read -r status body <<< "$(
        _curl POST "$DOORMAN_URL/v1/audit/capture" \
            -H 'Content-Type: application/json' \
            -d "$bad_event_payload"
    )"
    _check "POST /v1/audit/capture (unknown event_type) → 400" "400" "$status" "$body"
}

# ─── summary ─────────────────────────────────────────────────────────────────

echo ""
echo "========================================"
echo "  $PASS tests passed / $TOTAL total"
if [[ $FAIL -gt 0 ]]; then
    echo "  $FAIL tests FAILED — review output above"
fi
echo "========================================"

# Advisory mode: always exit 0 so the script does not block a deploy.
exit 0
