#!/usr/bin/env bash
# perf-bench-llama-server.sh — Tier A (local llama-server) throughput / throttle gate.
#
# Catches the throughput-regression class. On 2026-06-01 a cgroup MemoryMax +
# repack misconfig silently dropped llama-server from ~4 tok/s to 0.3 tok/s and
# went unnoticed for 10 days because no test measured token rate or throttling.
#
# Two gates:
#   (A) PRIMARY, always runs, needs no inference slot:
#       cgroup `memory.events high` must NOT climb during a sample window. This is
#       the exact regression signature — 517,907 while broken, 0 when healthy. The
#       active drain (or a bench request below) provides the generation load.
#   (B) BEST-EFFORT tok/s spot-check:
#       one request at max_tokens=N; tok/s from server-side timings if present,
#       else wall-clock. Gated against SLM_MIN_TOKS_PER_SEC. SKIPPED (non-fatal) if
#       the slot is busy (drain mid-brief) and the request times out. For a clean
#       tok/s reading run with the drain paused so the slot is free.
#
# Exit 0 = gates pass (or tok/s skipped). Exit 1 = a gate FAILED. Exit 2 = server down.
#
# The throttle gate fires on the CATASTROPHE (the repack-overflow thrash drove
# `high` up by hundreds of thousands and tok/s to 0.3), not on mild noise. A
# healthy box under sustained drain still shows a small `high` trickle (~3/s)
# because the working set sits near the memory.high watermark — that is sub-gate
# and expected; raising MemoryMax (Stage 2) eliminates it. The gate threshold
# SLM_MAX_HIGH_DELTA separates the two.
#
# Env: SLM_LOCAL_ENDPOINT (http://127.0.0.1:8080), SLM_MIN_TOKS_PER_SEC (3.5),
#      SLM_MAX_HIGH_DELTA (500), BENCH_MAX_TOKENS (64), BENCH_TIMEOUT (90), SAMPLE_SEC (45)

set -u
ENDPOINT="${SLM_LOCAL_ENDPOINT:-http://127.0.0.1:8080}"
MIN_TPS="${SLM_MIN_TOKS_PER_SEC:-3.5}"
MAX_HIGH_DELTA="${SLM_MAX_HIGH_DELTA:-500}"
MAXTOK="${BENCH_MAX_TOKENS:-64}"
BTIMEOUT="${BENCH_TIMEOUT:-90}"
SAMPLE="${SAMPLE_SEC:-45}"
fail=0

echo "== perf-bench-llama-server =="
echo "endpoint=$ENDPOINT floor=${MIN_TPS} tok/s"

if ! curl -s --max-time 8 "$ENDPOINT/health" 2>/dev/null | grep -q '"status":"ok"'; then
  echo "  [FAIL] llama-server /health not ok"; exit 2
fi

# ── locate cgroup ───────────────────────────────────────────────────────────
CG_EVENTS=""; CG_CUR=""; CG_MAX=""
PID="$(pgrep -f 'llama-server --host' | head -1)"
if [ -n "${PID:-}" ] && [ -r "/proc/$PID/cgroup" ]; then
  CG="$(sed 's/^0:://' "/proc/$PID/cgroup" | head -1)"
  CG_EVENTS="/sys/fs/cgroup${CG}/memory.events"
  CG_CUR="/sys/fs/cgroup${CG}/memory.current"
  CG_MAX="/sys/fs/cgroup${CG}/memory.max"
fi
high_before=""
[ -r "$CG_EVENTS" ] && high_before="$(awk '/^high /{print $2}' "$CG_EVENTS")"
if [ -r "$CG_CUR" ] && [ -r "$CG_MAX" ]; then
  cur="$(cat "$CG_CUR")"; max="$(cat "$CG_MAX")"
  printf "  cgroup mem: %.2f / %.2f GiB\n" "$(echo "$cur/1073741824"|bc -l)" "$(echo "$max/1073741824"|bc -l)"
fi

# ── (B) best-effort tok/s spot-check (also provides load for gate A) ────────
tps="" ; resp=""
start="$(date +%s.%N)"
resp="$(curl -s --max-time "$BTIMEOUT" "$ENDPOINT/v1/chat/completions" -H "Content-Type: application/json" \
  -d "{\"messages\":[{\"role\":\"user\",\"content\":\"Write a paragraph about software testing.\"}],\"max_tokens\":$MAXTOK,\"temperature\":0}" 2>/dev/null)"
end="$(date +%s.%N)"
if [ -n "$resp" ]; then
  tps="$(printf '%s' "$resp" | python3 -c '
import json,sys
try:
    d=json.load(sys.stdin)
except Exception:
    print(""); sys.exit()
t=d.get("timings") or {}
if "predicted_per_second" in t and t["predicted_per_second"]:
    print(f"{t[\"predicted_per_second\"]:.2f}")
else:
    ct=(d.get("usage") or {}).get("completion_tokens",0)
    print(f"server-timings-absent:{ct}")
' 2>/dev/null)"
fi
if [ -z "$resp" ]; then
  echo "  [skip] tok/s spot-check — slot busy (drain mid-brief); request timed out at ${BTIMEOUT}s"
elif printf '%s' "$tps" | grep -q '^server-timings-absent'; then
  ct="${tps#server-timings-absent:}"; wall="$(echo "$end-$start"|bc)"
  if [ "${ct:-0}" -gt 0 ] 2>/dev/null && [ "$(echo "$wall>0"|bc)" = "1" ]; then
    tps="$(echo "scale=2;$ct/$wall"|bc)"
    echo "  tok/s (wall-clock, no server timings): $tps"
  else tps=""; echo "  [skip] tok/s — no usable completion"; fi
elif [ -n "$tps" ]; then
  echo "  tok/s (server timings): $tps"
fi
if [ -n "$tps" ]; then
  if [ "$(echo "$tps < $MIN_TPS"|bc)" = "1" ]; then
    echo "  [FAIL] ${tps} tok/s < floor ${MIN_TPS}"; fail=1
  else echo "  [PASS] ${tps} tok/s >= floor ${MIN_TPS}"; fi
fi

# ── (A) cgroup throttle gate — sample over window (drain provides load) ─────
if [ -n "$high_before" ] && [ -r "$CG_EVENTS" ]; then
  # ensure some generation load exists during the sample (drain in-flight or just-ran request)
  sleep "$SAMPLE"
  high_after="$(awk '/^high /{print $2}' "$CG_EVENTS")"
  delta=$((high_after - high_before))
  if [ "$delta" -gt "$MAX_HIGH_DELTA" ]; then
    echo "  [FAIL] cgroup memory.events high climbed by $delta over ${SAMPLE}s (> ${MAX_HIGH_DELTA}) — catastrophic thrash (weights reclaimed per token)"
    fail=1
  elif [ "$delta" -gt 0 ]; then
    echo "  [PASS] cgroup high Δ=$delta over ${SAMPLE}s (mild, < ${MAX_HIGH_DELTA} floor — working set near memory.high; Stage 2 MemoryMax raise would zero it)"
  else
    echo "  [PASS] cgroup high Δ=0 over ${SAMPLE}s (no throttle)"
  fi
else
  echo "  [warn] cgroup memory.events unreadable — throttle gate skipped"
fi

echo "result: $([ "$fail" -eq 0 ] && echo PASS || echo FAIL) | tok/s=${tps:-skipped}"
exit "$fail"
