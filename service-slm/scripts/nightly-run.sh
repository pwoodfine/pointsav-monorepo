#!/usr/bin/env bash
# Yo-Yo #1 nightly pipeline — two mandatory phases.
#
# Phase 1 (DataGraph, 2h): vLLM UP → jennifer-datagraph-rebuild.sh → REST API extraction
# Phase 2 (Training, 2h):  vLLM DOWN → corpus-threshold.py → QLoRA on 7B model
#
# Both phases run every night regardless of data volume.
# Split configurable via DATAGRAPH_SECONDS (default 7200) / TRAINING_SECONDS (default 7200).
#
# Run AFTER local-content.service is live.
#
# Usage:
#   ./scripts/nightly-run.sh
#   ./scripts/nightly-run.sh --no-yoyo   (skip VM lifecycle; use local Tier A only)
#   ./scripts/nightly-run.sh --test-mode  (short timers: 60s DataGraph + 60s Training)
#
# Env vars:
#   DATAGRAPH_SECONDS   — DataGraph phase budget (default: 7200)
#   TRAINING_SECONDS    — Training phase budget (default: 7200)
#   JENNIFER_DEPLOYMENT — jennifer deployment root
#   FOUNDRY_ROOT        — Foundry workspace root (default: /srv/foundry)

set -uo pipefail

FOUNDRY_ROOT="${FOUNDRY_ROOT:-/srv/foundry}"
JENNIFER_DEPLOYMENT="${JENNIFER_DEPLOYMENT:-/srv/foundry/deployments/cluster-totebox-jennifer}"
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"

DATAGRAPH_SECONDS_DEFAULT=7200
TRAINING_SECONDS_DEFAULT=7200

NO_YOYO=false
TEST_MODE=false
while [[ $# -gt 0 ]]; do
    case "$1" in
        --no-yoyo)    NO_YOYO=true; shift ;;
        --test-mode)  TEST_MODE=true; shift ;;
        --help|-h)    sed -n '2,24p' "$0"; exit 0 ;;
        *) echo "Unknown flag: $1" >&2; exit 1 ;;
    esac
done

if [[ "${TEST_MODE}" == "true" ]]; then
    DATAGRAPH_SECONDS_DEFAULT=60
    TRAINING_SECONDS_DEFAULT=60
fi

DATAGRAPH_SECONDS="${DATAGRAPH_SECONDS:-${DATAGRAPH_SECONDS_DEFAULT}}"
TRAINING_SECONDS="${TRAINING_SECONDS:-${TRAINING_SECONDS_DEFAULT}}"

log() { echo "[nightly-run $(date -u +'%Y-%m-%dT%H:%M:%SZ')] $*"; }

log "Session start. DataGraph=${DATAGRAPH_SECONDS}s Training=${TRAINING_SECONDS}s no_yoyo=${NO_YOYO} test_mode=${TEST_MODE}"

# ── Phase 1: DataGraph rebuild ────────────────────────────────────────────────
log "=== Phase 1: DataGraph rebuild (${DATAGRAPH_SECONDS}s budget) ==="

if [[ "${NO_YOYO}" != "true" ]]; then
    log "Starting Yo-Yo #1 (vLLM Tier B for extraction)..."
    if "${SCRIPT_DIR}/start-yoyo.sh" --wait-ready=5400 --auto-snapshot 2>&1 | sed 's/^/  /'; then
        log "Yo-Yo #1 ready — Tier B available."
    else
        rc=${PIPESTATUS[0]}
        log "WARN: start-yoyo.sh exited rc=${rc}. Proceeding with Tier A fallback."
    fi
else
    log "Skipping Yo-Yo lifecycle (--no-yoyo)."
fi

# ── Phase 5b: adapter pull from previous training cycle ──────────────────────
# Pull LoRA adapter from yoyo-batch persistent disk if training completed.
# Runs after start-yoyo.sh (VM is up) so we have SSH access.
# Warn-only: a failed pull defers to the next cycle without aborting extraction.
if [[ "${NO_YOYO}" != "true" ]]; then
    YOYO_INSTANCE="${SLM_YOYO_GCP_INSTANCE:-yoyo-batch}"
    YOYO_ZONE="${SLM_YOYO_GCP_ZONE:-us-central1-a}"
    ADAPTER_SRC="${YOYO_INSTANCE}:/data/weights/adapters/apprenticeship-pointsav-wip/"
    ADAPTER_DEST="${FOUNDRY_ROOT}/data/adapters/apprenticeship-pointsav-incremental/"
    log "Phase 5b: checking for adapter on ${YOYO_INSTANCE} (${YOYO_ZONE})..."
    if gcloud compute ssh "${YOYO_INSTANCE}" --zone="${YOYO_ZONE}" \
        --command 'test -f /data/weights/adapters/apprenticeship-pointsav-wip/adapter_config.json' \
        --quiet 2>/dev/null; then
        log "Adapter found — pulling to ${ADAPTER_DEST}"
        mkdir -p "${ADAPTER_DEST}"
        if gcloud compute scp --recurse \
            "${ADAPTER_SRC}" "${ADAPTER_DEST}" \
            --zone="${YOYO_ZONE}" 2>&1 | sed 's/^/  /'; then
            log "Phase 5b: adapter pull succeeded."
        else
            log "WARN: Phase 5b adapter pull failed — will retry next cycle."
        fi
    else
        log "Phase 5b: no completed adapter on ${YOYO_INSTANCE} — skipping pull."
    fi
fi

# Workspace feeder warmup (20 workspace artifacts → DataGraph via REST)
log "Workspace feeder warmup (batch-size 20)..."
"${SCRIPT_DIR}/foundry-workspace-feeder.sh" --batch-size 20 2>&1 | sed 's/^/  /' || true

log "Running jennifer-datagraph-rebuild.sh (budget=${DATAGRAPH_SECONDS}s)..."
DATAGRAPH_SECONDS="${DATAGRAPH_SECONDS}" \
JENNIFER_DEPLOYMENT="${JENNIFER_DEPLOYMENT}" \
FOUNDRY_ROOT="${FOUNDRY_ROOT}" \
    "${SCRIPT_DIR}/jennifer-datagraph-rebuild.sh" \
    || log "WARN: jennifer-datagraph-rebuild returned non-zero — check service-content + Doorman"

log "Phase 1 complete. Stopping vLLM to free L4 GPU for training..."
if [[ "${NO_YOYO}" != "true" ]]; then
    "${SCRIPT_DIR}/stop-yoyo.sh" --drain-timeout=60 2>&1 | sed 's/^/  /' || true
fi

# ── Phase 2: Training ─────────────────────────────────────────────────────────
log "=== Phase 2: Training (${TRAINING_SECONDS}s budget) ==="
log "Running corpus-threshold.py (dispatches GCS marker if threshold met)..."
TRAINING_SECONDS="${TRAINING_SECONDS}" \
FOUNDRY_ROOT="${FOUNDRY_ROOT}" \
    python3 "${SCRIPT_DIR}/corpus-threshold.py" 2>&1 | sed 's/^/  /' || true

# ── Nightly summary ───────────────────────────────────────────────────────────
log "=== Nightly run complete ==="
HEALTH=$(cat "${FOUNDRY_ROOT}/data/datagraph-health.json" 2>/dev/null || echo '{}')
log "DataGraph: $(echo "${HEALTH}" | jq -r '"entity_count=\(.entity_count // 0) delta=\(.delta // 0) new_entities=\(.new_entities_this_run // 0)"')"
MARKER_COUNT=$(find "${FOUNDRY_ROOT}/data/training-pending" -name "*.json" -not -name "*.claimed" -not -name "*.completed" 2>/dev/null | wc -l)
log "Training: ${MARKER_COUNT} pending marker(s) dispatched this run."
