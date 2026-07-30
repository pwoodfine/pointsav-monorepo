#!/usr/bin/env bash
# Verifies that nightly-run.sh's two exit conditions fire correctly:
#   Test 1 — idle timeout (no SEMANTIC progress for IDLE_SECONDS)
#   Test 2 — hard-stop wall clock (HARD_STOP_SECONDS reached)
#
# Runs in --no-yoyo --test-mode so no GCE calls and no inference happen.
# Uses a tempdir for JENNIFER_BASE/FOUNDRY_ROOT so the real corpus is not
# touched. Scales the timers down to seconds via env vars.
#
# Pass criteria: each subtest exits 0 with the matching log line.
# Fail criteria: nightly-run does not exit within the per-test wall clock,
# or exits without the expected log line.
#
# Usage:
#   ./scripts/test-nightly-timeouts.sh
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
TEST_TMPDIR="$(mktemp -d -t nightly-run-test.XXXXXX)"
trap 'rm -rf "${TEST_TMPDIR}"' EXIT

mkdir -p "${TEST_TMPDIR}/jennifer/service-fs/data/service-people/ledgers"
mkdir -p "${TEST_TMPDIR}/jennifer/service-fs/data/service-content/ledgers"
mkdir -p "${TEST_TMPDIR}/foundry"

log_test() {
    echo "[test-nightly-timeouts $(date '+%H:%M:%S')] $*"
}

run_subtest() {
    local label="$1"; shift
    local wall_clock_limit="$1"; shift
    local expect_pattern="$1"; shift
    local logfile
    logfile="$(mktemp -t "nightly-run-${label}.XXXXXX")"

    log_test "=== ${label}: running (wall-clock budget ${wall_clock_limit}s) ==="
    set +e
    timeout --kill-after=10 "${wall_clock_limit}" \
        env "$@" \
            JENNIFER_BASE="${TEST_TMPDIR}/jennifer" \
            FOUNDRY_ROOT="${TEST_TMPDIR}/foundry" \
            "${SCRIPT_DIR}/nightly-run.sh" --no-yoyo --test-mode \
            >"${logfile}" 2>&1
    local rc=$?
    set -e

    if [[ "${rc}" -eq 124 ]]; then
        log_test "FAIL ${label}: nightly-run did not exit within ${wall_clock_limit}s wall-clock budget."
        echo "--- last 40 lines of ${logfile} ---"
        tail -40 "${logfile}"
        return 1
    fi
    if [[ "${rc}" -ne 0 ]]; then
        log_test "FAIL ${label}: nightly-run exited with rc=${rc} (expected 0)."
        echo "--- last 40 lines of ${logfile} ---"
        tail -40 "${logfile}"
        return 1
    fi
    if ! grep -qE "${expect_pattern}" "${logfile}"; then
        log_test "FAIL ${label}: log did not contain expected pattern: ${expect_pattern}"
        echo "--- last 40 lines of ${logfile} ---"
        tail -40 "${logfile}"
        return 1
    fi
    log_test "PASS ${label}: matched pattern '${expect_pattern}'."
    rm -f "${logfile}"
    return 0
}

PASSED=0
FAILED=0

# ── Test 1: idle timeout fires ───────────────────────────────────────────────
# IDLE_SECONDS=20 means the first post-sleep idle check triggers exit.
# HARD_STOP_SECONDS=300 keeps the wall-clock above any test runtime.
# Loop pattern: setup (~5s) + foundry-workspace-feeder (~5-25s on tempdir) +
# loop iter 0 (no sleep) + sleep 60s + loop iter 1 (idle elapsed = 60s ≥ 20s) → break.
# Wall-clock budget: 120s (covers feeder plus one full sleep cycle).
if run_subtest "Test1-idle-timeout" 120 "idle timeout" \
        IDLE_SECONDS=20 HARD_STOP_SECONDS=300; then
    PASSED=$((PASSED + 1))
else
    FAILED=$((FAILED + 1))
fi

echo ""

# ── Test 2: hard-stop fires ──────────────────────────────────────────────────
# HARD_STOP_SECONDS=15: at start of loop iter 1 (after 60s sleep), NOW > DEADLINE → break.
# IDLE_SECONDS=99999 to keep idle path from triggering first.
# Wall-clock budget: 120s.
if run_subtest "Test2-hard-stop" 120 "hard.stop reached|hard stop reached" \
        IDLE_SECONDS=99999 HARD_STOP_SECONDS=15; then
    PASSED=$((PASSED + 1))
else
    FAILED=$((FAILED + 1))
fi

echo ""
log_test "Summary: PASSED=${PASSED} FAILED=${FAILED}"
[[ "${FAILED}" -eq 0 ]] && exit 0 || exit 1
