#!/usr/bin/env bash
# health-check-drain.sh — apprenticeship drain liveness + stall detection.
#
# Catches the operational-stall class. Two real stalls this layer would have
# caught: (1) the 2.5 h empty-diff OLMo hang; (2) a persistently-failing brief
# stuck in an infinite ~30-min retry loop, blocking the queue.
#
# Checks:
#   1. Stale in-flight: any leased brief held longer than SLM_STALE_INFLIGHT_MIN.
#      Age is taken from the nanosecond timestamp EMBEDDED IN THE LEASE FILENAME
#      (`*.lease.<worker>.<ns>`), NOT file mtime — the lease file inherits the
#      brief's original (old) mtime, so mtime would massively over-report age.
#   2. Poison growth: queue-poison/ count grew since last run (advisory).
#   3. Drain liveness: if queue/ non-empty AND drain not paused, the drain is
#      alive iff queue-done/ advanced in the window OR a FRESH lease (< stale
#      floor) is being worked. (A single real-diff brief takes ~2 min, and
#      memory-bound generation CPU fluctuates, so neither done-movement-in-60s
#      nor a CPU sample is reliable alone — fresh-lease presence is the signal.)
#
# Exit 0 = healthy. Exit 1 = stall detected. Poison-growth alone warns, not fails.
#
# Env: APPR (/srv/foundry/data/apprenticeship), DOORMAN_ENV
#      (/etc/local-doorman/local-doorman.env), SLM_STALE_INFLIGHT_MIN (30),
#      LIVENESS_SAMPLE_SEC (60), STATE_FILE (/srv/foundry/data/slm-canary-last.json)

set -u
APPR="${APPR:-/srv/foundry/data/apprenticeship}"
DOORMAN_ENV="${DOORMAN_ENV:-/etc/local-doorman/local-doorman.env}"
STALE_MIN="${SLM_STALE_INFLIGHT_MIN:-30}"
SAMPLE="${LIVENESS_SAMPLE_SEC:-60}"
STATE_FILE="${STATE_FILE:-/srv/foundry/data/slm-canary-last.json}"
STALE_SEC=$((STALE_MIN * 60))

cnt() { ls "$1" 2>/dev/null | wc -l | tr -d ' '; }
# Age of a lease file in seconds, from the ns timestamp in its filename.
lease_age_sec() {
  local ns; ns="$(printf '%s' "$1" | sed -n 's/.*\.\([0-9]\{10,\}\)$/\1/p')"
  [ -z "$ns" ] && { echo -1; return; }
  echo $(( ( $(date +%s%N) - ns ) / 1000000000 ))
}

fail=0
echo "== health-check-drain =="
echo "appr=$APPR stale_floor=${STALE_MIN}min sample=${SAMPLE}s"

Q_PEND="$(cnt "$APPR/queue")"; Q_DONE="$(cnt "$APPR/queue-done")"; Q_POISON="$(cnt "$APPR/queue-poison")"
echo "queue: pending=$Q_PEND done=$Q_DONE poison=$Q_POISON"

# ── 1. stale in-flight (age from filename ns timestamp) ─────────────────────
max_age=0; stale_name=""; fresh_lease=0
if [ -d "$APPR/queue-in-flight" ]; then
  for f in "$APPR/queue-in-flight"/*; do
    [ -e "$f" ] || continue
    a="$(lease_age_sec "$(basename "$f")")"
    [ "$a" -lt 0 ] 2>/dev/null && continue
    [ "$a" -lt "$STALE_SEC" ] && fresh_lease=1
    if [ "$a" -gt "$max_age" ]; then max_age="$a"; stale_name="$(basename "$f")"; fi
  done
fi
if [ "$max_age" -gt "$STALE_SEC" ]; then
  echo "  [FAIL] stale in-flight lease $((max_age/60))min old (> ${STALE_MIN}): $stale_name"
  fail=1
else
  echo "  [PASS] max in-flight lease age $((max_age/60))min (<= ${STALE_MIN})"
fi

# ── 2. poison growth (advisory) ─────────────────────────────────────────────
prev_poison=""
[ -r "$STATE_FILE" ] && prev_poison="$(python3 -c 'import json,sys
try: print(json.load(open(sys.argv[1])).get("poison",""))
except Exception: print("")' "$STATE_FILE" 2>/dev/null)"
if [ -n "$prev_poison" ] && [ "$Q_POISON" -gt "$prev_poison" ] 2>/dev/null; then
  echo "  [warn] poison grew ${prev_poison} -> ${Q_POISON} since last run"
else
  echo "  [PASS] poison not growing (prev=${prev_poison:-n/a} now=${Q_POISON})"
fi

# ── 3. drain liveness ───────────────────────────────────────────────────────
DRAIN_PAUSED="unknown"
if [ -r "$DOORMAN_ENV" ]; then
  grep -qE '^SLM_DRAIN_PAUSED=(true|1)[[:space:]]*$' "$DOORMAN_ENV" && DRAIN_PAUSED="true" || DRAIN_PAUSED="false"
fi
if [ "$Q_PEND" -gt 0 ] && [ "$DRAIN_PAUSED" = "false" ]; then
  before="$Q_DONE"; sleep "$SAMPLE"; after="$(cnt "$APPR/queue-done")"
  # re-scan for a fresh lease after the window too
  fresh_after=0
  for f in "$APPR/queue-in-flight"/*; do
    [ -e "$f" ] || continue
    a="$(lease_age_sec "$(basename "$f")")"
    [ "$a" -ge 0 ] 2>/dev/null && [ "$a" -lt "$STALE_SEC" ] && fresh_after=1
  done
  if [ "$after" -gt "$before" ]; then
    echo "  [PASS] drain live: done advanced $before -> $after over ${SAMPLE}s"
  elif [ "$fresh_after" -eq 1 ]; then
    echo "  [PASS] drain live: a fresh brief is in-flight (being worked; real-diff briefs take ~2min)"
  else
    echo "  [FAIL] drain stalled: queue=$Q_PEND pending, done frozen at $before over ${SAMPLE}s, no fresh in-flight lease"
    fail=1
  fi
else
  echo "  [skip] liveness (queue empty=$( [ "$Q_PEND" -eq 0 ] && echo yes || echo no ) / drain_paused=$DRAIN_PAUSED)"
fi

printf '{"poison":%s,"done":%s,"ts":%s}\n' "$Q_POISON" "$Q_DONE" "$(date +%s)" > "$STATE_FILE" 2>/dev/null || true
echo "result: $([ "$fail" -eq 0 ] && echo PASS || echo FAIL)"
exit "$fail"
