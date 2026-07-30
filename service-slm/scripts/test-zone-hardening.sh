#!/usr/bin/env bash
# Mock test suite for start-yoyo.sh zone-discipline hardening.
#
# Tests the three behaviours added in commit 00e19718:
#   1. SLM_YOYO_ALLOW_ZONE_FALLBACK env var is ignored (hardcoded false).
#   2. --enable-zone-fallback CLI flag enables Mode 2 with a visible warning.
#   3. Default retry count is 4 cycles (not 1) — script retries on stockout.
#
# No network calls, no GCP credentials required. All gcloud calls and sleep
# are replaced by stubs in a temp PATH directory.
#
# Exit 0 if all tests pass, 1 if any fail.
#
# Usage:
#   ./scripts/test-zone-hardening.sh
set -uo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
START_YOYO="${SCRIPT_DIR}/start-yoyo.sh"

PASSED=0; FAILED=0
TMPDIR_MOCK="$(mktemp -d)"
trap 'rm -rf "${TMPDIR_MOCK}"' EXIT

# ── Result helpers ────────────────────────────────────────────────────────────
pass() { PASSED=$(( PASSED + 1 )); echo "  PASS  $1"; }
fail() { FAILED=$(( FAILED + 1 )); echo "  FAIL  $1"; [[ -n "${2:-}" ]] && echo "        ${2}"; }

# ── run_script: capture output + exit code without aborting on non-zero ───────
run_script() {
    local _out _rc
    _out=$(bash "${START_YOYO}" "$@" 2>&1); _rc=$?
    echo "${_out}"
    return "${_rc}"
}

# ── Mock binaries ─────────────────────────────────────────────────────────────
# mock gcloud: VM found in europe-west4-a; start → stockout; describe → empty
cat > "${TMPDIR_MOCK}/gcloud" <<'GCLOUD_EOF'
#!/usr/bin/env bash
args="$*"
if   [[ "${args}" == *"instances list"* ]]; then
    echo "europe-west4-a"
elif [[ "${args}" == *"instances start"* ]]; then
    echo "ERROR: (gcloud.compute.instances.start) ZONE_RESOURCE_POOL_EXHAUSTED" >&2
    exit 1
elif [[ "${args}" == *"instances describe"* ]]; then
    exit 0
else
    exit 0
fi
GCLOUD_EOF
chmod +x "${TMPDIR_MOCK}/gcloud"

# mock sleep: instant (no actual waiting during tests)
cat > "${TMPDIR_MOCK}/sleep" <<'SLEEP_EOF'
#!/usr/bin/env bash
echo "[MOCK SLEEP ${1}s]"
SLEEP_EOF
chmod +x "${TMPDIR_MOCK}/sleep"

# Shared env: point to mock binaries; disable real lifecycle log writes
export PATH="${TMPDIR_MOCK}:${PATH}"
export LIFECYCLE_LOG="${TMPDIR_MOCK}/lifecycle.log"
export SLM_YOYO_DAILY_LEDGER="${TMPDIR_MOCK}/daily-spend"
export SLM_YOYO_GCP_PROJECT="mock-project"
export SLM_YOYO_GCP_ZONE="europe-west4-a"
export SLM_YOYO_GCP_INSTANCE="yoyo-tier-b-1"

echo ""
echo "start-yoyo.sh zone-hardening mock tests"
echo "════════════════════════════════════════"

# ── Test 1: Env var SLM_YOYO_ALLOW_ZONE_FALLBACK=true is ignored ─────────────
echo ""
echo "Test 1: SLM_YOYO_ALLOW_ZONE_FALLBACK env var is ignored"

output=$(SLM_YOYO_ALLOW_ZONE_FALLBACK=true bash "${START_YOYO}" 2>&1); rc=$?

if [[ "${rc}" -eq 3 ]]; then
    pass "exits code 3 (stockout cascade) when env var is true"
else
    fail "expected exit 3, got ${rc}"
fi

if echo "${output}" | grep -q "waiting out stockout"; then
    pass "logs correct 'waiting out stockout' message"
else
    fail "expected 'waiting out stockout' in output" \
         "$(echo "${output}" | grep -i 'zone\|fallback\|mode' | head -3)"
fi

if echo "${output}" | grep -qi "mode 2\|zone relocation\|provisioning"; then
    fail "output must NOT mention Mode 2 provisioning" \
         "$(echo "${output}" | grep -i 'mode 2\|zone relocation\|provisioning')"
else
    pass "output does NOT attempt Mode 2 provisioning"
fi

# ── Test 2: --enable-zone-fallback prints warning ─────────────────────────────
echo ""
echo "Test 2: --enable-zone-fallback flag shows warning"

output=$(bash "${START_YOYO}" --enable-zone-fallback 2>&1); rc=$?

if echo "${output}" | grep -q "WARNING: Zone fallback ENABLED"; then
    pass "warning block printed when --enable-zone-fallback passed"
else
    fail "expected WARNING block in output" \
         "$(echo "${output}" | head -5)"
fi

if echo "${output}" | grep -q "2-20 per probe"; then
    pass 'cost warning ($2-20 per probe) included in warning block'
else
    fail 'expected cost warning in output' \
         "$(echo "${output}" | grep -i 'cost\|probe\|disk' | head -3)"
fi

if echo "${output}" | grep -q "deliberate zone migration"; then
    pass "warning clarifies: flag is for deliberate zone migration only"
else
    fail "expected migration context in warning" \
         "$(echo "${output}" | grep -i 'migration\|migration\|deliberate' | head -3)"
fi

# ── Test 3: Default retry count is 4 cycles ───────────────────────────────────
echo ""
echo "Test 3: Default 4 retry cycles on stockout"

output=$(bash "${START_YOYO}" 2>&1); rc=$?

if [[ "${rc}" -eq 3 ]]; then
    pass "exits code 3 after exhausting all 4 cycles"
else
    fail "expected exit 3, got ${rc}"
fi

if echo "${output}" | grep -q "retry cycle 1/4"; then
    pass "logs 'retry cycle 1/4'"
else
    fail "expected 'retry cycle 1/4' in output" \
         "$(echo "${output}" | grep -i 'retry\|cycle' | head -5)"
fi

if echo "${output}" | grep -q "retry cycle 3/4"; then
    pass "logs 'retry cycle 3/4' (all cycles reached)"
else
    fail "expected 'retry cycle 3/4' in output" \
         "$(echo "${output}" | grep -i 'retry\|cycle' | head -5)"
fi

retry_count=$(echo "${output}" | grep -c "retry cycle" || true)
if [[ "${retry_count}" -eq 3 ]]; then
    pass "exactly 3 retry log lines (cycles 1, 2, 3 — 4 total attempts)"
else
    fail "expected 3 retry messages, got ${retry_count}" \
         "$(echo "${output}" | grep 'retry cycle')"
fi

# ── Test 4: sleep is mocked (sanity-check test isolation) ────────────────────
echo ""
echo "Test 4: Sleep mocked (test isolation check)"

if echo "${output}" | grep -q "\[MOCK SLEEP"; then
    pass "mock sleep called — test ran without real waits"
else
    fail "mock sleep was not called — may have run real sleep" \
         "$(echo "${output}" | grep -i sleep | head -3)"
fi

# ── Summary ───────────────────────────────────────────────────────────────────
echo ""
echo "════════════════════════════════════════"
echo "Results: ${PASSED} passed, ${FAILED} failed"
echo ""

[[ "${FAILED}" -eq 0 ]]
