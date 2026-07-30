#!/bin/bash
# Nightly batch driver for jennifer-2 .md file migration through the LLM inference loop.
#
# Health gate uses /readyz (NOT /health — that endpoint doesn't exist in slm-doorman-server).
# tier_a is a bool (not an object with circuit_state).
# tier_b is a map keyed by node label (e.g. "default", "trainer"); each node has "circuit" field.
# go_no_go is at summary.go_no_go (nested inside "summary"), NOT at top level.
#
# DPO loss guard: if Tier A is alive AND Tier B circuit is open, flush_tier_a() in
# service-content/src/main.rs:778-779 marks CORPUS files as Success permanently → DPO pairs
# permanently skipped. Do NOT run migration in this state.
#
# Cron: 0 23 * * * /srv/foundry/clones/project-data/service-input/scripts/nightly-jennifer-migrate.sh

set -euo pipefail

LOG=/tmp/nightly-jennifer-migrate-$(date -u +%Y%m%d).log
exec >> "$LOG" 2>&1
echo "[$(date -u +%FT%TZ)] nightly-jennifer-migrate start"

# 1. Doorman health gate — /readyz returns JSON; /healthz returns plain "ok"
# Parse via stdin (not inline string interpolation) so Bloomberg-headline filenames
# with apostrophes or triple-quote sequences don't break the parser.
READYZ=$(curl -sf --max-time 5 http://127.0.0.1:9080/readyz 2>/dev/null || echo "{}")

read -r TIER_A TIER_B_CIRCUIT < <(printf '%s' "$READYZ" | python3 -c "
import sys, json
try:
    d = json.load(sys.stdin)
    tier_a = 'true' if d.get('tier_a', False) else 'false'
    tb = d.get('tier_b', {})
    circuit = 'unknown'
    for node, info in tb.items():
        if isinstance(info, dict) and 'circuit' in info:
            circuit = info['circuit']; break
    print(tier_a, circuit)
except Exception:
    print('unknown unknown')
" 2>/dev/null || echo "unknown unknown")

echo "[$(date -u +%FT%TZ)] tier_a=$TIER_A  tier_b_circuit=$TIER_B_CIRCUIT"

# DPO loss guard: Tier A alive + Tier B circuit open → flush_tier_a() permanently loses pairs
if [ "$TIER_A" = "true" ] && [ "$TIER_B_CIRCUIT" = "open" ]; then
    echo "[$(date -u +%FT%TZ)] SKIP: Tier A=alive + Tier B=circuit-open. DPO loss risk (service-content main.rs:778-779). Waiting for Tier B recovery."
    exit 0
fi

# 2. service-input :9106 health
if ! curl -sf --max-time 5 http://127.0.0.1:9106/healthz > /dev/null 2>&1; then
    echo "[$(date -u +%FT%TZ)] ERROR: service-input :9106 not responding. Aborting."
    exit 1
fi

# 3. Calibration gate — go_no_go is nested at summary.go_no_go, NOT top-level
# infrastructure_failure=true means Tier B is down; F1 scores are meaningless
# in that state, so we bypass the go_no_go=stop gate to keep migration running.
CALREP=$(curl -sf --max-time 10 http://127.0.0.1:9106/v1/calibration-report 2>/dev/null || echo "{}")
read -r GO_NO_GO GO_REASON INFRA_FAILURE < <(printf '%s' "$CALREP" | python3 -c "
import sys, json
try:
    d = json.load(sys.stdin)
    s = d.get('summary', {})
    print(s.get('go_no_go', 'stop'), s.get('go_no_go_reason', 'unknown'), str(d.get('infrastructure_failure', False)).lower())
except Exception:
    print('stop unknown false')
" 2>/dev/null || echo "stop unknown false")

# "infrastructure-hold" means all Tier B GPU nodes are down — extraction is systemically
# unavailable, not a data quality problem. Migration continues (CORPUS accumulates);
# LoRA training is still blocked (handled separately by yoyo-daily-cycle.sh).
if [ "$GO_NO_GO" = "stop" ]; then
    if [ "$INFRA_FAILURE" = "true" ]; then
        echo "[$(date -u +%FT%TZ)] WARN: calibration go_no_go=stop but infrastructure_failure=true (Tier B down) — bypassing gate, continuing migration."
    else
        echo "[$(date -u +%FT%TZ)] STOP: calibration go_no_go=stop reason=$GO_REASON. Aborting."
        exit 1
    fi
fi
echo "[$(date -u +%FT%TZ)] calibration go_no_go=$GO_NO_GO infrastructure_failure=$INFRA_FAILURE reason=$GO_REASON — proceeding"

# 4. Batch migration loop
# offset pagination: if docs are added between calls, resume offset may shift.
# Keep ASSET_ROOT frozen during migration runs to avoid this (see BRIEF open decisions).
OFFSET=0
TOTAL=0
BATCH_SIZE=10

while true; do
    RESP=$(curl -sf --max-time 30 -X POST http://127.0.0.1:9106/v1/migrate \
        -H 'Content-Type: application/json' \
        -d "{\"batch_size\":$BATCH_SIZE,\"offset\":$OFFSET}" 2>/dev/null || true)

    # Parse all three fields in one stdin call so apostrophes in filenames can't
    # break interpolation. Emit "ERROR" as processed when the response is empty
    # (curl failed) so we distinguish failure from a legitimate empty batch.
    if [ -z "$RESP" ]; then
        echo "[$(date -u +%FT%TZ)] ERROR: curl to /v1/migrate failed or timed out. Aborting."
        exit 1
    fi

    read -r PROCESSED SKIPPED OFFSET < <(printf '%s' "$RESP" | python3 -c "
import sys, json
try:
    d = json.load(sys.stdin)
    print(d.get('processed', 0), d.get('skipped', 0), d.get('offset_next', 0))
except Exception:
    print(0, 0, 0)
" 2>/dev/null || echo "0 0 0")

    TOTAL=$((TOTAL + PROCESSED))
    echo "[$(date -u +%FT%TZ)] batch: processed=$PROCESSED skipped=$SKIPPED offset_next=$OFFSET total=$TOTAL"

    # Empty batch = all docs processed or skipped
    if [ "$((PROCESSED + SKIPPED))" -eq 0 ]; then
        echo "[$(date -u +%FT%TZ)] Migration complete. Total migrated this run: $TOTAL"
        break
    fi

    # Night window: stop before 05:00 UTC to avoid peak hours
    HOUR=$(date -u +%H)
    if [ "$HOUR" -ge 5 ]; then
        echo "[$(date -u +%FT%TZ)] Night window ended (hour=$HOUR UTC). Resuming tomorrow at offset=$OFFSET."
        break
    fi

    # Rate limit: service-input enforces SERVICE_INPUT_RATE_PER_MIN=6 internally,
    # but add a small sleep between batches to let service-extraction breathe
    sleep 2
done

echo "[$(date -u +%FT%TZ)] nightly-jennifer-migrate done"
