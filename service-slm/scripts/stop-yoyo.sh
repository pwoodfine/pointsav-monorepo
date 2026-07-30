#!/usr/bin/env bash
# On-demand Yo-Yo #1 stop with optional drain + snapshot.
#
# Under normal operation the Doorman idle monitor stops the VM automatically
# after SLM_YOYO_IDLE_MINUTES (default 30) of vLLM-reachable-zero-active-slots.
# This script is for explicit lifecycle control (e.g. nightly-run.sh end-of-
# session, cost emergency, maintenance).
#
# Drain: --drain-timeout=N sleeps N seconds before stopping so in-flight Tier B
# requests have time to complete. Simple wait; no Doorman control surface
# required.
#
# Snapshot: --snapshot-before-stop calls create-yoyo-snapshot.sh prior to stop,
# preserving the weights disk's current state for zone-migration recovery.
#
# Exit codes:
#   0 — VM stopped successfully (or already stopped)
#   1 — gcloud stop failure (auth/permission/unknown)
#
# Usage:
#   ./scripts/stop-yoyo.sh
#   ./scripts/stop-yoyo.sh --drain-timeout=60
#   ./scripts/stop-yoyo.sh --drain-timeout=60 --snapshot-before-stop
#   SLM_YOYO_GCP_INSTANCE=yoyo-tier-b-2 ./scripts/stop-yoyo.sh
set -uo pipefail

PROJECT="${SLM_YOYO_GCP_PROJECT:-woodfine-node-gcp-free}"
ZONE="${SLM_YOYO_GCP_ZONE:-us-west1-b}"
INSTANCE="${SLM_YOYO_GCP_INSTANCE:-yoyo-tier-b-1}"
LIFECYCLE_LOG="${SLM_YOYO_LIFECYCLE_LOG:-/srv/foundry/data/yoyo-lifecycle.log}"
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"

# ── Flag parsing ─────────────────────────────────────────────────────────────
DRAIN_TIMEOUT=0
SNAPSHOT_BEFORE_STOP=false
while [[ $# -gt 0 ]]; do
    case "$1" in
        --drain-timeout=*)         DRAIN_TIMEOUT="${1#*=}"; shift ;;
        --drain-timeout)           DRAIN_TIMEOUT="$2"; shift 2 ;;
        --snapshot-before-stop)    SNAPSHOT_BEFORE_STOP=true; shift ;;
        --help|-h)
            sed -n '2,25p' "$0"
            exit 0
            ;;
        *) echo "Unknown flag: $1" >&2; exit 1 ;;
    esac
done

# ── Lifecycle logging ────────────────────────────────────────────────────────
log() {
    local ts msg
    ts="$(date -u +'%Y-%m-%dT%H:%M:%SZ')"
    msg="[stop-yoyo ${ts}] $*"
    echo "${msg}"
    if [[ -w "$(dirname "${LIFECYCLE_LOG}")" ]] || [[ -w "${LIFECYCLE_LOG}" ]] 2>/dev/null; then
        echo "${msg}" >> "${LIFECYCLE_LOG}" 2>/dev/null || true
    fi
}

# ── Drain ────────────────────────────────────────────────────────────────────
if [[ "${DRAIN_TIMEOUT}" -gt 0 ]]; then
    log "Drain: waiting ${DRAIN_TIMEOUT}s for in-flight Tier B requests to settle..."
    sleep "${DRAIN_TIMEOUT}"
fi

# ── Optional pre-stop snapshot ───────────────────────────────────────────────
if [[ "${SNAPSHOT_BEFORE_STOP}" == "true" ]]; then
    log "Pre-stop snapshot: invoking create-yoyo-snapshot.sh..."
    if [[ -x "${SCRIPT_DIR}/create-yoyo-snapshot.sh" ]]; then
        "${SCRIPT_DIR}/create-yoyo-snapshot.sh" 2>&1 | sed 's/^/  /' || \
            log "WARN: snapshot creation reported failure — continuing with stop."
    else
        log "WARN: ${SCRIPT_DIR}/create-yoyo-snapshot.sh not found or not executable — skipping."
    fi
fi

# ── Stop ─────────────────────────────────────────────────────────────────────
log "Stopping ${INSTANCE} in ${PROJECT}/${ZONE} ..."
err=$(gcloud compute instances stop "${INSTANCE}" \
    --project="${PROJECT}" --zone="${ZONE}" 2>&1)
rc=$?
if [[ "${rc}" -ne 0 ]]; then
    if echo "${err}" | grep -qiE "already.*(stopped|terminated)|in state TERMINATED"; then
        log "VM already stopped/terminated — treating as success."
        exit 0
    fi
    log "ERROR: gcloud stop failed (rc=${rc}): ${err}"
    exit 1
fi

log "VM stopped."
exit 0
