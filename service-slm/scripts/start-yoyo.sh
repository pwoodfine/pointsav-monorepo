#!/usr/bin/env bash
# On-demand Yo-Yo #1 start with two-tier zone cycling and optional vLLM wait-ready.
#
# Mode 1 — Preemption recovery (normal case):
#   The existing VM is TERMINATED in SLM_YOYO_GCP_ZONE due to preemption.
#   Try gcloud instances.start in that zone; if the zone has capacity it
#   comes back in ~60 s. This is the fast, cheap path.
#
# Mode 2 — Zone stockout recovery (fallback):
#   The zone has ZONE_RESOURCE_POOL_EXHAUSTED and can't restart the VM.
#   Try each FALLBACK_ZONES entry in order, provisioning a fresh VM if needed.
#
# Day-time stockout: --retry-cycles=N --retry-wait-seconds=M lets the script
# spend a longer wall-clock budget hunting for L4 capacity (sleep + try again).
#
# Wait-ready: --wait-ready[=SECONDS] polls https://<vm-ip>:9443/health with
# bearer until it returns 200 (vLLM finished loading) or the timeout fires.
#
# Auto-snapshot: --auto-snapshot creates a snapshot the first time vLLM is
# verified ready, so subsequent zone migrations can restore the weights disk.
#
# Exit codes:
#   0 — VM up + (vLLM ready, if --wait-ready)
#   1 — GCE start failure (auth/quota/permission/unknown)
#   2 — vLLM ready-poll timeout (VM up, but model not loaded in time)
#   3 — zone stockout cascade exhausted across all retries
#
# --runtime=Nh / --runtime=Nm: after vLLM is ready, schedule an auto-stop after
#   N hours or N minutes. Requires --wait-ready to have any effect (the timer
#   starts only after the ready-poll succeeds). The stop runs in a background
#   subshell so the script exits immediately; the stop-timer PID is logged.
#   Doorman's idle monitor (30 min) may stop the VM sooner if it goes idle.
#   Example: ./scripts/start-yoyo.sh --wait-ready=120 --runtime=1h
#
# Usage:
#   ./scripts/start-yoyo.sh
#   ./scripts/start-yoyo.sh --wait-ready=300 --auto-snapshot
#   ./scripts/start-yoyo.sh --wait-ready=120 --runtime=1h
#   ./scripts/start-yoyo.sh --retry-cycles=3 --retry-wait-seconds=300
#   SLM_YOYO_GCP_INSTANCE=yoyo-tier-b-2 ./scripts/start-yoyo.sh
set -uo pipefail

PROJECT="${SLM_YOYO_GCP_PROJECT:-woodfine-node-gcp-free}"
PRIMARY_ZONE="${SLM_YOYO_GCP_ZONE:-europe-west4-a}"
INSTANCE="${SLM_YOYO_GCP_INSTANCE:-yoyo-tier-b-1}"
DOORMAN_ENV="${DOORMAN_ENV_FILE:-/etc/local-doorman/local-doorman.env}"
# Zone fallback disabled by default — creating 256 GB disk clones across regions
# to probe capacity costs $2-20/scan. Enable only for explicit operator-initiated
# zone migration: SLM_YOYO_ALLOW_ZONE_FALLBACK=true ./scripts/start-yoyo.sh
ALLOW_ZONE_FALLBACK="${SLM_YOYO_ALLOW_ZONE_FALLBACK:-false}"
BEARER_TOKEN="${SLM_YOYO_BEARER:-}"
IMAGE_FAMILY="${SLM_YOYO_IMAGE_FAMILY:-slm-yoyo}"
IMAGE_PROJECT="${SLM_YOYO_IMAGE_PROJECT:-${PROJECT}}"
WEIGHTS_DISK="${INSTANCE}-weights"
# When set, new weights disks are restored from this snapshot instead of created empty.
# Set this after uploading weights: create-yoyo-snapshot.sh → SLM_YOYO_WEIGHTS_SNAPSHOT
WEIGHTS_SNAPSHOT="${SLM_YOYO_WEIGHTS_SNAPSHOT:-}"
LIFECYCLE_LOG="${SLM_YOYO_LIFECYCLE_LOG:-/srv/foundry/data/yoyo-lifecycle.log}"
KILL_SWITCH="${SLM_YOYO_KILL_SWITCH:-/srv/foundry/data/yoyo-disabled}"

# ── Flag parsing ─────────────────────────────────────────────────────────────
WAIT_READY=0       # 0 = no wait, >0 = poll seconds before exiting
AUTO_SNAPSHOT=false
RETRY_CYCLES=1
RETRY_WAIT=300
RETRY_UNTIL_EPOCH=0   # 0 = disabled; >0 = Unix epoch deadline, exit before this
RUNTIME_SECS=0     # 0 = no auto-stop; >0 = stop VM this many seconds after ready
WEIGHTS_GCS_BUCKET="${SLM_YOYO_WEIGHTS_GCS_BUCKET:-woodfine-node-gcp-free-foundry-substrate}"
while [[ $# -gt 0 ]]; do
    case "$1" in
        --wait-ready=*)         WAIT_READY="${1#*=}"; shift ;;
        --wait-ready)           WAIT_READY=5400; shift ;;
        --auto-snapshot)        AUTO_SNAPSHOT=true; shift ;;
        --retry-cycles=*)       RETRY_CYCLES="${1#*=}"; shift ;;
        --retry-wait-seconds=*) RETRY_WAIT="${1#*=}"; shift ;;
        --retry-until-epoch=*)  RETRY_UNTIL_EPOCH="${1#*=}"; shift ;;
        --runtime=*)
            _rt="${1#*=}"
            if   [[ "${_rt}" =~ ^([0-9]+)h$ ]]; then RUNTIME_SECS=$(( ${BASH_REMATCH[1]} * 3600 ))
            elif [[ "${_rt}" =~ ^([0-9]+)m$ ]]; then RUNTIME_SECS=$(( ${BASH_REMATCH[1]} * 60 ))
            elif [[ "${_rt}" =~ ^([0-9]+)s?$ ]]; then RUNTIME_SECS="${BASH_REMATCH[1]}"
            else echo "Unknown --runtime format '${_rt}'. Use Nh, Nm, or Ns." >&2; exit 1
            fi
            shift ;;
        --help|-h)
            sed -n '2,37p' "$0"
            exit 0
            ;;
        *) echo "Unknown flag: $1" >&2; exit 1 ;;
    esac
done

# ── Lifecycle logging ────────────────────────────────────────────────────────
log() {
    local ts msg
    ts="$(date -u +'%Y-%m-%dT%H:%M:%SZ')"
    msg="[start-yoyo ${ts}] $*"
    echo "${msg}"
    if [[ -w "$(dirname "${LIFECYCLE_LOG}")" ]] || [[ -w "${LIFECYCLE_LOG}" ]] 2>/dev/null; then
        echo "${msg}" >> "${LIFECYCLE_LOG}" 2>/dev/null || true
    fi
}

# Interruptible sleep: checks kill switch every 30s. If deadline epoch is passed as
# $2 (>0), returns 1 when deadline is reached so caller can exit before timer fires.
kill_switch_aware_sleep() {
    local total="$1" deadline="${2:-0}" elapsed=0
    while [[ "${elapsed}" -lt "${total}" ]]; do
        if [[ -e "${KILL_SWITCH}" ]]; then
            log "KILL SWITCH ACTIVE (${KILL_SWITCH}) — aborting during sleep."
            exit 0
        fi
        if [[ "${deadline}" -gt 0 ]] && [[ "$(date +%s)" -ge "${deadline}" ]]; then
            return 1
        fi
        sleep 30
        elapsed=$(( elapsed + 30 ))
    done
    return 0
}

# UTC offsets for L4-capable GCP zones (integer hours; DST ignored — 1h precision
# is sufficient for demand-pattern scoring). Spot capacity correlates with commercial
# compute demand, which follows business hours in each zone's local market.
# Scoring: local hour 01-07 = deep night (5), 20-01 = late evening (4),
#          07-09 = early morning (3), daytime = 1.
# Zones where it is currently night float to the top of the fallback list,
# maximising the chance of finding available L4 Spot capacity.
# us-east4 omitted — does not stock g2-standard-4.
declare -A ZONE_UTC_OFFSET=(
    ["us-west1-a"]=-8    ["us-west1-b"]=-8
    ["us-west4-a"]=-8
    ["us-central1-a"]=-6 ["us-central1-b"]=-6 ["us-central1-c"]=-6
    ["us-east1-b"]=-5    ["us-east1-c"]=-5    ["us-east1-d"]=-5
    ["northamerica-northeast1-b"]=-5 ["northamerica-northeast1-c"]=-5
    ["europe-west1-b"]=1 ["europe-west1-c"]=1 ["europe-west4-a"]=1
    ["europe-west2-a"]=0 ["europe-west2-b"]=0
    ["asia-east1-a"]=8   ["asia-east1-b"]=8
    ["asia-southeast1-a"]=8 ["asia-southeast1-b"]=8
)

# Returns zone names one-per-line, sorted by night-score descending.
# Excludes the zone passed as $1 (already tried in Mode 1).
# Ties broken by $RANDOM so repeated runs don't hammer the same zone.
sorted_fallback_zones() {
    local skip_zone="${1:-}"
    local utc_hour
    utc_hour=$(date -u +%-H)   # %-H strips leading zero for bash arithmetic
    local -a scored=()
    local zone offset local_hour score
    for zone in "${!ZONE_UTC_OFFSET[@]}"; do
        [[ "${zone}" == "${skip_zone}" ]] && continue
        offset="${ZONE_UTC_OFFSET[$zone]}"
        local_hour=$(( (utc_hour + offset + 24) % 24 ))
        if   (( local_hour >= 1  && local_hour <  7 )); then score=5
        elif (( local_hour >= 20 || local_hour <  1 )); then score=4
        elif (( local_hour >= 7  && local_hour <  9 )); then score=3
        else score=1
        fi
        scored+=("${score}.${RANDOM}:${zone}")
    done
    printf '%s\n' "${scored[@]}" | sort -t: -k1 -rn | sed 's/^[^:]*://'
}

# ── Helper: check if gcloud error output indicates zone stockout ──────────────
is_stockout() {
    local stderr_output="$1"
    echo "${stderr_output}" | grep -q "ZONE_RESOURCE_POOL_EXHAUSTED\|does not have enough resources\|stockout"
}

# ── Helper: get the VM's current zone ────────────────────────────────────────
# If PRIMARY_ZONE is set and the VM exists there, return that zone (preferred).
# Otherwise return the first zone from the project-wide list (handles name
# collisions when the same instance name exists in multiple zones).
current_vm_zone() {
    if [[ -n "${PRIMARY_ZONE}" ]]; then
        local z
        z=$(gcloud compute instances list \
                --project="${PROJECT}" \
                --filter="name=${INSTANCE} AND zone:${PRIMARY_ZONE}" \
                --format="value(zone.basename())" 2>/dev/null | head -1)
        if [[ -n "${z}" ]]; then
            echo "${z}"
            return
        fi
    fi
    gcloud compute instances list \
        --project="${PROJECT}" \
        --filter="name=${INSTANCE}" \
        --format="value(zone.basename())" 2>/dev/null | head -1
}

# ── Helper: create a fresh VM in a zone ──────────────────────────────────────
provision_vm_in_zone() {
    local zone="$1"
    echo "  [PROVISION] Creating ${INSTANCE} in ${PROJECT}/${zone}..."

    # Create weights disk — restore from snapshot if one exists, otherwise blank.
    # 256GB pd-balanced fits the first-boot bootstrap peak (safetensors 64GB + intermediate
    # fp16 GGUF 64GB during convert step, before cleanup) PLUS steady-state (base 20GB +
    # LoRA adapters 3GB + tokenizer + checkpoints + headroom). pd-balanced is much cheaper
    # than pd-ssd; LoRA I/O is fine on balanced. ~$26/mo always-attached.
    echo "  [PROVISION] Creating weights disk ${WEIGHTS_DISK} (256GB pd-balanced) in ${zone}..."
    local disk_create_args=(
        "${WEIGHTS_DISK}"
        --project="${PROJECT}"
        --zone="${zone}"
        --type=pd-balanced
        --labels=role=yoyo-weights
    )
    if [[ -n "${WEIGHTS_SNAPSHOT}" ]]; then
        echo "  [PROVISION] Restoring from snapshot ${WEIGHTS_SNAPSHOT} (weights preserved)."
        disk_create_args+=(--source-snapshot="${WEIGHTS_SNAPSHOT}")
    else
        echo "  [PROVISION] No snapshot set — empty disk; vllm-weights-prep.service will bootstrap from GCS or AllenAI."
        disk_create_args+=(--size=256GB)
    fi
    if ! gcloud compute disks create "${disk_create_args[@]}" 2>&1; then
        echo "  [PROVISION] Disk creation failed in ${zone} — trying next zone."
        return 1
    fi

    # Build metadata arg — bearer-token (nginx auth) + weights-gcs-bucket
    # (consumed by vllm-weights-prep.service to know where to fetch/upload).
    local meta_kv=()
    [[ -n "${BEARER_TOKEN}" ]] && meta_kv+=("bearer-token=${BEARER_TOKEN}")
    [[ -n "${WEIGHTS_GCS_BUCKET}" ]] && meta_kv+=("weights-gcs-bucket=${WEIGHTS_GCS_BUCKET}")
    local meta_arg=""
    if [[ "${#meta_kv[@]}" -gt 0 ]]; then
        meta_arg="--metadata=$(IFS=','; printf '%s' "${meta_kv[*]}")"
    fi

    # Create the instance.
    # Ephemeral external IP is allocated by default (no --no-address flag) so:
    #   (a) wait_for_vllm_ready can probe https://<ip>:9443/health from outside
    #   (b) Doorman's existing SLM_YOYO_ENDPOINT pattern (https://<ip>:9443) works
    #   (c) the VM has internet egress for HF download during first-boot bootstrap
    # Scope is restricted by VPC firewall rule (only the workspace VM IP allowed
    # through to port 9443; SSH is via IAP). This is the existing operational pattern.
    local err_output
    err_output=$(gcloud compute instances create "${INSTANCE}" \
        --project="${PROJECT}" \
        --zone="${zone}" \
        --machine-type=g2-standard-4 \
        --accelerator=type=nvidia-l4,count=1 \
        --maintenance-policy=TERMINATE \
        --provisioning-model=SPOT \
        --instance-termination-action=STOP \
        --image-family="${IMAGE_FAMILY}" \
        --image-project="${IMAGE_PROJECT}" \
        --boot-disk-size=50GB \
        --boot-disk-type=pd-balanced \
        --disk=name="${WEIGHTS_DISK}",device-name=yoyo-weights,auto-delete=no \
        --tags=yoyo-tier-b \
        --scopes=cloud-platform \
        ${meta_arg} 2>&1)

    if [[ $? -ne 0 ]]; then
        if is_stockout "${err_output}"; then
            echo "  [PROVISION] Stockout in ${zone} — deleting disk, trying next."
            gcloud compute disks delete "${WEIGHTS_DISK}" --project="${PROJECT}" --zone="${zone}" --quiet 2>/dev/null || true
            return 1
        else
            echo "  [PROVISION] VM creation failed in ${zone}: ${err_output}"
            gcloud compute disks delete "${WEIGHTS_DISK}" --project="${PROJECT}" --zone="${zone}" --quiet 2>/dev/null || true
            return 1
        fi
    fi

    echo "${zone}"
    return 0
}

# ── Helper: update Doorman env with new zone, IP, snapshot ──────────────────
# After every successful provision/start, the VM may have a new external IP.
# Doorman's SLM_YOYO_ENDPOINT must reflect this for the health probe to land.
# Best-effort: writes if the env file is writable; otherwise emits the new
# values to stdout so an operator can apply them via sudo.
update_doorman_env() {
    local new_zone="$1"
    local new_ip
    new_ip=$(gcloud compute instances describe "${INSTANCE}" \
            --project="${PROJECT}" --zone="${new_zone}" \
            --format='value(networkInterfaces[0].accessConfigs[0].natIP)' 2>/dev/null || echo "")
    local new_endpoint=""
    [[ -n "${new_ip}" ]] && new_endpoint="https://${new_ip}:9443"

    if [[ ! -w "${DOORMAN_ENV}" ]]; then
        echo "Note: ${DOORMAN_ENV} not writable by this process. Apply these as root:"
        echo "  SLM_YOYO_GCP_ZONE=${new_zone}"
        [[ -n "${new_endpoint}" ]] && echo "  SLM_YOYO_ENDPOINT=${new_endpoint}"
        [[ -n "${WEIGHTS_SNAPSHOT}" ]] && echo "  SLM_YOYO_WEIGHTS_SNAPSHOT=${WEIGHTS_SNAPSHOT}"
        echo "Then: sudo systemctl restart local-doorman.service"
        return 0
    fi

    sed -i "s|^SLM_YOYO_GCP_ZONE=.*|SLM_YOYO_GCP_ZONE=${new_zone}|" "${DOORMAN_ENV}"
    echo "Updated SLM_YOYO_GCP_ZONE=${new_zone} in ${DOORMAN_ENV}."

    if [[ -n "${new_endpoint}" ]]; then
        if grep -q "^SLM_YOYO_ENDPOINT=" "${DOORMAN_ENV}"; then
            sed -i "s|^SLM_YOYO_ENDPOINT=.*|SLM_YOYO_ENDPOINT=${new_endpoint}|" "${DOORMAN_ENV}"
        else
            echo "SLM_YOYO_ENDPOINT=${new_endpoint}" >> "${DOORMAN_ENV}"
        fi
        echo "Updated SLM_YOYO_ENDPOINT=${new_endpoint} in ${DOORMAN_ENV}."
    fi

    if [[ -n "${WEIGHTS_SNAPSHOT}" ]]; then
        if grep -q "^SLM_YOYO_WEIGHTS_SNAPSHOT=" "${DOORMAN_ENV}"; then
            sed -i "s|^SLM_YOYO_WEIGHTS_SNAPSHOT=.*|SLM_YOYO_WEIGHTS_SNAPSHOT=${WEIGHTS_SNAPSHOT}|" "${DOORMAN_ENV}"
        else
            echo "SLM_YOYO_WEIGHTS_SNAPSHOT=${WEIGHTS_SNAPSHOT}" >> "${DOORMAN_ENV}"
        fi
        echo "Updated SLM_YOYO_WEIGHTS_SNAPSHOT=${WEIGHTS_SNAPSHOT} in ${DOORMAN_ENV}."
    fi

    # Restart Doorman so it picks up the new SLM_YOYO_ENDPOINT from the env file.
    # Systemd services load env vars at start — editing the file is not enough.
    echo "Restarting local-doorman.service to apply new endpoint..."
    if sudo systemctl restart local-doorman.service 2>/dev/null; then
        echo "Doorman restarted."
    else
        echo "Note: could not auto-restart Doorman — run: sudo systemctl restart local-doorman.service"
    fi
}

# ── Helper: poll vLLM /health endpoint until 200 or timeout ──────────────────
# Returns 0 on ready, 1 on timeout. Uses bearer auth via nginx (port 9443).
wait_for_vllm_ready() {
    local zone="$1"
    local ip endpoint deadline http_code
    ip=$(gcloud compute instances describe "${INSTANCE}" \
            --project="${PROJECT}" --zone="${zone}" \
            --format='value(networkInterfaces[0].accessConfigs[0].natIP)' 2>/dev/null)
    if [[ -z "${ip}" ]]; then
        log "ERROR: could not determine VM external IP in ${zone} for wait-ready."
        return 1
    fi
    endpoint="https://${ip}:9443/health"
    deadline=$(( $(date +%s) + WAIT_READY ))
    log "Waiting for vLLM at ${endpoint} (timeout ${WAIT_READY}s)..."
    while [[ $(date +%s) -lt ${deadline} ]]; do
        http_code=$(curl -k -sS -o /tmp/yoyo-health.json -w '%{http_code}' \
            --max-time 5 -H "Authorization: Bearer ${BEARER_TOKEN}" \
            "${endpoint}" 2>/dev/null || echo "000")
        if [[ "${http_code}" == "200" ]]; then
            log "vLLM ready (HTTP 200 from ${endpoint})."
            return 0
        fi
        sleep 10
    done
    log "ERROR: vLLM did not become ready at ${endpoint} within ${WAIT_READY}s (last HTTP ${http_code:-???})."
    return 1
}

# ── Helper: trigger weights snapshot via create-yoyo-snapshot.sh ─────────────
maybe_create_snapshot() {
    local zone="$1"
    local snap_script
    snap_script="$(dirname "$0")/create-yoyo-snapshot.sh"
    if [[ ! -x "${snap_script}" ]]; then
        log "WARN: ${snap_script} not found or not executable — skipping auto-snapshot."
        return 0
    fi
    log "Auto-snapshot: creating snapshot of weights disk in ${zone}..."
    SLM_YOYO_GCP_ZONE="${zone}" "${snap_script}" 2>&1 | sed 's/^/  /' || \
        log "WARN: snapshot creation reported failure — continuing."
}

# ── Helper: print operator post-provisioning steps after Mode 2 ──────────────
print_post_provision_steps() {
    local zone="$1"
    cat <<EOF

IMPORTANT — post-provisioning steps:

  0. Add an external IP (if IAP is not available):
     gcloud compute instances add-access-config ${INSTANCE} --zone=${zone} --project=${PROJECT}
     NEW_IP=\$(gcloud compute instances describe ${INSTANCE} --zone=${zone} --project=${PROJECT} --format='value(networkInterfaces[0].accessConfigs[0].natIP)')

  1. Set bearer token in instance metadata (if not set before provisioning):
     gcloud compute instances add-metadata ${INSTANCE} --zone=${zone} --project=${PROJECT} --metadata=bearer-token=\${SLM_YOYO_BEARER}

  2. rc.local auto-mounts the weights disk at /data/weights on first boot.
     Verify: gcloud compute ssh ${INSTANCE} --zone=${zone} --project=${PROJECT} --command='mountpoint /data/weights'

  3. Upload weights:
     gcloud compute scp <weights.gguf> ${INSTANCE}:/data/weights/olmo-3-32b-think-q4.gguf --zone=${zone} --project=${PROJECT}

  4. Start vllm: gcloud compute ssh ${INSTANCE} --zone=${zone} --project=${PROJECT} --command='sudo systemctl start vllm.service'

  5. Update SLM_YOYO_ENDPOINT in ${DOORMAN_ENV} with new external IP:
     sudo sed -i "s|SLM_YOYO_ENDPOINT=.*|SLM_YOYO_ENDPOINT=https://\${NEW_IP}:9443|" ${DOORMAN_ENV}

  6. Restart Doorman: sudo systemctl restart local-doorman.service
EOF
}

# ── attempt_start_once: one full Mode-1+Mode-2 pass ──────────────────────────
# Sets STARTED_ZONE on success.
# Returns: 0 success, 1 hard failure (auth/permission), 3 stockout in all zones.
attempt_start_once() {
    local known_zone err
    known_zone=$(current_vm_zone)

    if [[ -n "${known_zone}" ]]; then
        # Mode 1: VM exists — try to start it in its current zone
        log "Found ${INSTANCE} in ${PROJECT}/${known_zone}. Attempting start (Mode 1)..."
        err=$(gcloud compute instances start "${INSTANCE}" \
            --project="${PROJECT}" --zone="${known_zone}" 2>&1)
        if [[ $? -eq 0 ]]; then
            log "VM started in ${known_zone} (Mode 1: preemption recovery)."
            STARTED_ZONE="${known_zone}"
            update_doorman_env "${known_zone}"
            return 0
        fi
        if is_stockout "${err}"; then
            log "Zone ${known_zone} has no L4 capacity."
            if [[ "${ALLOW_ZONE_FALLBACK}" != "true" ]]; then
                log "Zone fallback disabled — skipping (set SLM_YOYO_ALLOW_ZONE_FALLBACK=true to scan other zones)."
                return 3
            fi
            log "Falling through to Mode 2 (ALLOW_ZONE_FALLBACK=true)."
        else
            log "ERROR: failed to start ${INSTANCE} in ${known_zone}: ${err}"
            return 1
        fi
    else
        log "No existing ${INSTANCE} in project ${PROJECT}."
        if [[ "${ALLOW_ZONE_FALLBACK}" != "true" ]]; then
            log "Zone fallback disabled — cannot provision without an existing VM (set SLM_YOYO_ALLOW_ZONE_FALLBACK=true to enable)."
            return 3
        fi
        log "Entering Mode 2 (provision) — ALLOW_ZONE_FALLBACK=true."
    fi

    # Mode 2: provision a new VM in a time-scored fallback zone.
    # sorted_fallback_zones() ranks zones where it is currently night first —
    # lower commercial GPU demand means more L4 Spot capacity available.
    log "Zone order (top 3 by night-score): $(sorted_fallback_zones "${known_zone:-}" | head -3 | tr '\n' ' ')"
    local zone
    while IFS= read -r zone; do
        log "Trying to provision ${INSTANCE} in ${PROJECT}/${zone} ..."
        if provision_vm_in_zone "${zone}" >&2; then
            log "VM provisioned in ${zone} (Mode 2: zone relocation)."
            STARTED_ZONE="${zone}"
            update_doorman_env "${zone}"
            print_post_provision_steps "${zone}"
            return 0
        fi
    done < <(sorted_fallback_zones "${known_zone:-}")

    log "All fallback zones exhausted in this cycle."
    return 3
}

# ─────────────────────────────────────────────────────────────────────────────
# Main — retry-cycle loop wraps Mode 1 + Mode 2; then optional wait-ready + snapshot
# ─────────────────────────────────────────────────────────────────────────────

log "Session start. instance=${INSTANCE} primary_zone=${PRIMARY_ZONE} retry_cycles=${RETRY_CYCLES} retry_wait=${RETRY_WAIT}s wait_ready=${WAIT_READY}s auto_snapshot=${AUTO_SNAPSHOT} retry_until_epoch=${RETRY_UNTIL_EPOCH}"

STARTED_ZONE=""
_SCRIPT_START=$(date +%s)
cycle=0
while [[ "${RETRY_UNTIL_EPOCH}" -gt 0 ]] || [[ "${cycle}" -lt "${RETRY_CYCLES}" ]]; do
    if [[ -e "${KILL_SWITCH}" ]]; then
        log "KILL SWITCH ACTIVE (${KILL_SWITCH}) — aborting retry loop."
        exit 0
    fi
    if [[ "${RETRY_UNTIL_EPOCH}" -gt 0 ]] && [[ "$(date +%s)" -ge "${RETRY_UNTIL_EPOCH}" ]]; then
        log "[RETRY:DEADLINE] Approaching systemd timer (epoch ${RETRY_UNTIL_EPOCH}) — exiting before it fires. VM not started."
        exit 3
    fi
    if [[ "${cycle}" -gt 0 ]]; then
        log "Stockout retry cycle ${cycle} — sleeping ${RETRY_WAIT}s before next attempt..."
        if ! kill_switch_aware_sleep "${RETRY_WAIT}" "${RETRY_UNTIL_EPOCH}"; then
            log "[RETRY:DEADLINE] Timer deadline reached mid-sleep — exiting before timer fires. VM not started."
            exit 3
        fi
    fi

    attempt_start_once
    rc=$?
    _attempt_elapsed=$(( $(date +%s) - _SCRIPT_START ))
    if   [[ "${rc}" -eq 0 ]]; then _result="SUCCESS"
    elif [[ "${rc}" -eq 1 ]]; then _result="HARD_FAIL"
    else                           _result="STOCKOUT"
    fi
    log "[RETRY:ATTEMPT cycle=$(( cycle + 1 ))/${RETRY_CYCLES} elapsed=${_attempt_elapsed}s result=${_result}]"
    if [[ "${rc}" -eq 0 ]]; then
        # Re-check kill switch — may have been activated while gcloud start was in-flight
        if [[ -e "${KILL_SWITCH}" ]]; then
            log "KILL SWITCH ACTIVE (${KILL_SWITCH}) — stopping VM that was just started."
            gcloud compute instances stop "${INSTANCE}" --zone "${STARTED_ZONE:-${PRIMARY_ZONE}}" --quiet || true
            exit 0
        fi
        break
    elif [[ "${rc}" -eq 1 ]]; then
        log "Hard failure during start attempt — aborting (exit 1)."
        exit 1
    fi
    # rc == 3: stockout cascade exhausted in this cycle — retry if cycles remain
    cycle=$(( cycle + 1 ))
done

if [[ -z "${STARTED_ZONE}" ]]; then
    log "ERROR: could not start or provision ${INSTANCE} in any zone after ${RETRY_CYCLES} cycle(s). Exit 3."
    exit 3
fi

# ── Optional wait-ready + auto-snapshot ──────────────────────────────────────
if [[ "${WAIT_READY}" -gt 0 ]]; then
    if ! wait_for_vllm_ready "${STARTED_ZONE}"; then
        exit 2
    fi
    if [[ "${AUTO_SNAPSHOT}" == "true" ]] && [[ -z "${WEIGHTS_SNAPSHOT}" ]]; then
        maybe_create_snapshot "${STARTED_ZONE}"
    fi
else
    log "Allow ~2 minutes for vLLM to finish loading the model."
    log "Doorman health probe will detect readiness within 30 seconds."
fi

# ── Optional auto-stop timer ─────────────────────────────────────────────────
# Spawned only when --runtime=Nh/Nm is set AND the VM reached ready state.
# Runs in a background subshell so this script exits immediately.
# Doorman's 30-minute idle monitor may stop the VM sooner if it goes idle.
if [[ "${RUNTIME_SECS}" -gt 0 ]]; then
    if [[ "${WAIT_READY}" -le 0 ]]; then
        log "WARN: --runtime=${RUNTIME_SECS}s has no effect without --wait-ready (timer starts from ready confirmation, not VM start)."
    else
        _zone="${STARTED_ZONE}"
        (
            sleep "${RUNTIME_SECS}"
            _ts="$(date -u +'%Y-%m-%dT%H:%M:%SZ')"
            echo "[start-yoyo ${_ts}] runtime-stop: stopping ${INSTANCE} in ${PROJECT}/${_zone} after ${RUNTIME_SECS}s"
            gcloud compute instances stop "${INSTANCE}" \
                --project="${PROJECT}" --zone="${_zone}" --quiet \
                && echo "[start-yoyo $(date -u +'%Y-%m-%dT%H:%M:%SZ')] runtime-stop: VM stopped." \
                || echo "[start-yoyo $(date -u +'%Y-%m-%dT%H:%M:%SZ')] runtime-stop: WARNING: gcloud stop returned non-zero"
        ) &
        STOP_TIMER_PID=$!
        log "runtime-stop: auto-stop scheduled in ${RUNTIME_SECS}s (background PID=${STOP_TIMER_PID})."
        log "  To cancel: kill ${STOP_TIMER_PID}"
    fi
fi

log "Session done. Exit 0."
exit 0
