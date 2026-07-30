#!/usr/bin/env bash
# slm-canary.sh — combined hourly SLM canary: perf/throttle gate + drain health.
#
# Runs perf-bench-llama-server.sh then health-check-drain.sh, tees a timestamped
# summary to $REPORT_DIR/slm-canary-latest.txt (+ dated copy), and exits non-zero
# if either gate failed so a systemd OnFailure= or a watcher can alert.
#
# Wired by foundry-slm-canary.timer (see scripts/systemd/). Env passes through to
# the child scripts (SLM_MIN_TOKS_PER_SEC, SLM_MAX_HIGH_DELTA, SLM_STALE_INFLIGHT_MIN, …).

set -u
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPORT_DIR="${REPORT_DIR:-/srv/foundry/data/reports}"
mkdir -p "$REPORT_DIR" 2>/dev/null || true
TS="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
OUT="$REPORT_DIR/slm-canary-${TS}.txt"

{
  echo "===== SLM canary $TS ====="
  echo
  bash "$HERE/perf-bench-llama-server.sh"; perf_rc=$?
  echo
  bash "$HERE/health-check-drain.sh"; drain_rc=$?
  echo
  echo "===== summary: perf=$([ $perf_rc -eq 0 ] && echo PASS || echo FAIL) drain=$([ $drain_rc -eq 0 ] && echo PASS || echo FAIL) ====="
} 2>&1 | tee "$OUT"

cp -f "$OUT" "$REPORT_DIR/slm-canary-latest.txt" 2>/dev/null || true
# exit non-zero if either gate failed (perf_rc/drain_rc captured in subshell via files)
grep -q "perf=FAIL\|drain=FAIL" "$OUT" && exit 1 || exit 0
