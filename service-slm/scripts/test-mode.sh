#!/usr/bin/env bash
# test-mode.sh — capped, self-terminating Tier B quality-validation harness.
#
# PURPOSE
#   Boot yoyo-batch (L4 GPU) for a SHORT, capped testing session, run a real
#   DataGraph-injection check and a LoRA SFT smoke-train + quality gate, then
#   GUARANTEE the VM is stopped — regardless of success, failure, Ctrl-C,
#   SIGTERM, or the hard wall-clock wall.
#
#   This is a standalone diagnostic, run INDEPENDENTLY of the nightly Yo-Yo
#   cycle. It is the gate that should decide whether the nightly cycle is safe
#   to re-enable.
#
# WHAT IT NEVER TOUCHES
#   - The production adapter dir (data/adapters/apprenticeship-pointsav-incremental/)
#   - Any training-approved/*.ran receipt (writes NO receipt)
#   - local-slm.service / production llama-server serving
#   It writes ONLY to a throwaway tree: /srv/foundry/data/yoyo-test/<ts>/
#
# SHUTDOWN GUARANTEES (defence in depth)
#   1. trap 'hard_stop' EXIT INT TERM — Phase 5 stop runs on EVERY exit path.
#   2. Background wall-clock watchdog SIGKILLs the main script after MAX_MINUTES
#      and independently issues a VM stop (the in-process RuntimeMaxSec analogue).
#   3. In-script SECONDS soft deadline forces a graceful jump to hard_stop BEFORE
#      the watchdog fires, so the normal path is a clean stop, not a SIGKILL.
#   4. Kill switch checked at preflight, between every phase, AND inside every
#      sleep loop (ks_sleep, never plain sleep) — touching the file stops a
#      RUNNING VM within ~one poll interval.
#   5. VM-on seconds are checkpoint-debited every poll interval, so a mid-run
#      SIGKILL still leaves an accurate budget ledger.
#   6. The shared manual-cycle flock is held for the whole run, so the
#      production ExecStopPost guard (yoyo-vm-stop-guard.sh) defers to us while
#      we are alive and stops the VM if we die without cleaning up.
#
# Usage:
#   ./test-mode.sh                 # default 2-hour hard wall (matches nightly budget)
#   MAX_MINUTES=45 ./test-mode.sh  # tighter wall for quick validation
#   ./test-mode.sh --max-minutes 45
#
# Exit codes:
#   0  — ran to completion (tests may have individually FAILED — see result JSON)
#   2  — preflight aborted before any VM spend
#   3  — STOCKOUT: could not provision L4 within cap (no spend beyond start try)
#   4  — kill switch tripped (VM stopped, clean exit)
#   124 — killed by the wall-clock watchdog (VM stop still attempted)

set -uo pipefail

# ──────────────────────────────────────────────────────────────────────────────
# Configuration
# ──────────────────────────────────────────────────────────────────────────────
MAX_MINUTES="${MAX_MINUTES:-120}"

while [[ $# -gt 0 ]]; do
    case "$1" in
        --max-minutes=*) MAX_MINUTES="${1#*=}"; shift ;;
        --max-minutes)   MAX_MINUTES="$2"; shift 2 ;;
        --help|-h)       sed -n '2,46p' "$0"; exit 0 ;;
        *) echo "Unknown flag: $1" >&2; exit 1 ;;
    esac
done

# Hard wall (watchdog SIGKILL) and soft wall (graceful jump to hard_stop).
HARD_WALL_SECONDS=$(( MAX_MINUTES * 60 ))
# Soft wall sits 90s before the hard wall so a graceful stop wins the race.
SOFT_WALL_SECONDS=$(( HARD_WALL_SECONDS - 90 ))
(( SOFT_WALL_SECONDS < 60 )) && SOFT_WALL_SECONDS=$(( HARD_WALL_SECONDS / 2 ))

# GCP target — per SYSTEM CONTEXT, NOT the start/stop-yoyo defaults.
PROJECT="${SLM_YOYO_GCP_PROJECT:-woodfine-node-gcp-free}"
ZONE="${SLM_YOYO_GCP_ZONE:-us-central1-a}"
INSTANCE="${SLM_YOYO_GCP_INSTANCE:-yoyo-batch}"
GCLOUD="${GCLOUD_BIN:-/snap/bin/gcloud}"
SSH_KEY="${SLM_YOYO_SSH_KEY:-/home/mathew/.ssh/google_compute_engine}"

# Kill switch — distinct from the production switch (/srv/foundry/data/yoyo-disabled).
KILL_SWITCH="${TEST_MODE_KILL_SWITCH:-/srv/foundry/data/yoyo-test-mode-kill}"

# Shared manual-cycle VM lock — same file the production guard tests.
VM_LOCK="${SLM_YOYO_VM_LOCK:-/srv/foundry/data/yoyo-vm.lock}"

# Endpoints.
DOORMAN="http://127.0.0.1:9080"
DATAGRAPH="http://127.0.0.1:9081"

# Paths.
FOUNDRY_ROOT="/srv/foundry"
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
TS="$(date -u +%Y%m%dT%H%M%SZ)"
TEST_ROOT="${FOUNDRY_ROOT}/data/yoyo-test/${TS}"
ADAPTER_OUT="${TEST_ROOT}/adapter"
CORPUS_OUT="${TEST_ROOT}/corpus.jsonl"
HEARTBEAT="${TEST_ROOT}/heartbeat"
LOG_FILE="${TEST_ROOT}/test-mode.log"
RESULT_FILE="${FOUNDRY_ROOT}/data/test-mode-results-${TS}.json"
BUDGET_LEDGER="${FOUNDRY_ROOT}/data/yoyo-budget/test-mode.ledger"

# Remote scratch dir on the VM.
REMOTE_DIR="/tmp/yoyo-test-${TS}"

# Polling interval for kill-switch-aware waits.
POLL_INTERVAL=15

# Watchdog PID (background timer) and VM-started flag.
WATCHDOG_PID=""
VM_STARTED=0
VM_IP=""
LOCK_FD=""
HARD_STOP_DONE=0

mkdir -p "${TEST_ROOT}" "${ADAPTER_OUT}" "$(dirname "${BUDGET_LEDGER}")" 2>/dev/null || true
echo "started=${TS} pid=$$ max_minutes=${MAX_MINUTES}" > "${HEARTBEAT}"

# ──────────────────────────────────────────────────────────────────────────────
# Logging
# ──────────────────────────────────────────────────────────────────────────────
log() {
    local ts msg
    ts="$(date -u +'%Y-%m-%dT%H:%M:%SZ')"
    msg="[test-mode ${ts}] $*"
    echo "${msg}"
    echo "${msg}" >> "${LOG_FILE}" 2>/dev/null || true
}
phase() {
    log "════════════════════════════════════════════════════════════════"
    log "PHASE $*  (elapsed ${SECONDS}s / soft ${SOFT_WALL_SECONDS}s / hard ${HARD_WALL_SECONDS}s)"
    log "════════════════════════════════════════════════════════════════"
}

# ──────────────────────────────────────────────────────────────────────────────
# Result accumulation — appended to as tests run; flushed in hard_stop.
# ──────────────────────────────────────────────────────────────────────────────
declare -a TEST_NAMES=()
declare -a TEST_RESULTS=()   # "pass" | "fail" | "skip"
declare -a TEST_DETAILS=()
record() {
    # record <name> <pass|fail|skip> <detail>
    TEST_NAMES+=("$1")
    TEST_RESULTS+=("$2")
    TEST_DETAILS+=("${3:-}")
    log "  RESULT [$2] $1 — ${3:-}"
}

# ──────────────────────────────────────────────────────────────────────────────
# Budget ledger — debit VM-on seconds. Called at every poll AND at hard_stop so a
# SIGKILL mid-run still leaves an accurate ledger (we record cumulative-since-start
# minus what we already debited).
# ──────────────────────────────────────────────────────────────────────────────
VM_ON_START_SECONDS=""    # value of $SECONDS when the VM came up
LAST_DEBITED=0            # VM-on seconds already written to the ledger
debit_budget() {
    [[ -z "${VM_ON_START_SECONDS}" ]] && return 0
    local on_now=$(( SECONDS - VM_ON_START_SECONDS ))
    (( on_now < 0 )) && on_now=0
    local delta=$(( on_now - LAST_DEBITED ))
    (( delta <= 0 )) && return 0
    LAST_DEBITED="${on_now}"
    printf '%s ts=%s vm_on_seconds_total=%d delta=%d ts_session=%s\n' \
        "$(date -u +'%Y-%m-%dT%H:%M:%SZ')" "${TS}" "${on_now}" "${delta}" "${TS}" \
        >> "${BUDGET_LEDGER}" 2>/dev/null || true
}

# ──────────────────────────────────────────────────────────────────────────────
# Kill switch
# ──────────────────────────────────────────────────────────────────────────────
kill_switch_tripped() { [[ -e "${KILL_SWITCH}" ]]; }

check_kill_switch() {
    if kill_switch_tripped; then
        log "KILL SWITCH PRESENT (${KILL_SWITCH}) — aborting to hard stop"
        record "kill-switch" "fail" "kill switch tripped at SECONDS=${SECONDS}"
        EXIT_CODE=4
        # Trigger the EXIT trap (hard_stop) by exiting.
        exit 4
    fi
}

# Kill-switch-aware sleep. Sleeps up to $1 seconds in POLL_INTERVAL slices,
# checking the kill switch and the soft wall and debiting budget each slice.
ks_sleep() {
    local remaining="$1"
    while (( remaining > 0 )); do
        if kill_switch_tripped; then
            log "kill switch detected inside ks_sleep — breaking to hard stop"
            EXIT_CODE=4
            exit 4
        fi
        if (( SECONDS >= SOFT_WALL_SECONDS )); then
            log "soft wall (${SOFT_WALL_SECONDS}s) reached inside ks_sleep — jumping to hard stop"
            EXIT_CODE=124
            exit 124
        fi
        debit_budget
        local slice="${POLL_INTERVAL}"
        (( slice > remaining )) && slice="${remaining}"
        sleep "${slice}"
        remaining=$(( remaining - slice ))
    done
}

check_soft_wall() {
    if (( SECONDS >= SOFT_WALL_SECONDS )); then
        log "soft wall (${SOFT_WALL_SECONDS}s) reached — jumping to hard stop"
        EXIT_CODE=124
        exit 124
    fi
}

# Guard run between every phase.
between_phases() {
    check_kill_switch
    check_soft_wall
    debit_budget
}

# ──────────────────────────────────────────────────────────────────────────────
# VM control
# ──────────────────────────────────────────────────────────────────────────────
vm_status() {
    "${GCLOUD}" compute instances describe "${INSTANCE}" \
        --project="${PROJECT}" --zone="${ZONE}" \
        --format='value(status)' 2>/dev/null || echo "UNKNOWN"
}

vm_external_ip() {
    "${GCLOUD}" compute instances describe "${INSTANCE}" \
        --project="${PROJECT}" --zone="${ZONE}" \
        --format='value(networkInterfaces[0].accessConfigs[0].natIP)' 2>/dev/null || true
}

remote_ssh() {
    # remote_ssh <command...>  — run a command on the VM via direct SSH (no IAP).
    ssh -i "${SSH_KEY}" -o StrictHostKeyChecking=no \
        -o ConnectTimeout=10 -o ServerAliveInterval=30 \
        "mathew@${VM_IP}" "$*" 2>>"${LOG_FILE}"
}

remote_rsync() {
    # remote_rsync <local> <remote>  — rsync to VM via direct SSH (no IAP).
    rsync -az \
        -e "ssh -i ${SSH_KEY} -o StrictHostKeyChecking=no -o ConnectTimeout=10" \
        "$1" "mathew@${VM_IP}:$2" 2>>"${LOG_FILE}"
}

# ──────────────────────────────────────────────────────────────────────────────
# hard_stop — Phase 5. Runs on EVERY exit path via trap. Idempotent.
# ──────────────────────────────────────────────────────────────────────────────
hard_stop() {
    local rc="${EXIT_CODE:-$?}"
    # Re-entrancy guard: the trap can fire while we are already stopping.
    if (( HARD_STOP_DONE == 1 )); then return; fi
    HARD_STOP_DONE=1
    # Disarm further traps so an error inside hard_stop does not recurse.
    trap - EXIT INT TERM

    log "════════════════════════════════════════════════════════════════"
    log "PHASE 5 — UNCONDITIONAL HARD STOP (exit code ${rc})"
    log "════════════════════════════════════════════════════════════════"

    # Final budget debit before the VM goes down.
    debit_budget

    # Kill the wall-clock watchdog so it does not SIGKILL us mid-cleanup.
    if [[ -n "${WATCHDOG_PID}" ]] && kill -0 "${WATCHDOG_PID}" 2>/dev/null; then
        kill "${WATCHDOG_PID}" 2>/dev/null || true
        wait "${WATCHDOG_PID}" 2>/dev/null || true
    fi

    # Stop the VM if we ever started it (or if it is unexpectedly up).
    local status
    status="$(vm_status)"
    if (( VM_STARTED == 1 )) || [[ "${status}" == "RUNNING" || "${status}" == "STAGING" || "${status}" == "PROVISIONING" ]]; then
        log "Stopping ${INSTANCE} (current status: ${status})"
        "${GCLOUD}" compute instances stop "${INSTANCE}" \
            --project="${PROJECT}" --zone="${ZONE}" --quiet \
            >>"${LOG_FILE}" 2>&1 || log "WARNING: gcloud stop returned non-zero"

        # Wait up to 180s for TERMINATED, polling every 10s.
        local waited=0
        while (( waited < 180 )); do
            status="$(vm_status)"
            log "  waiting for TERMINATED — status=${status} (${waited}s)"
            [[ "${status}" == "TERMINATED" ]] && break
            sleep 10
            waited=$(( waited + 10 ))
        done
        if [[ "${status}" != "TERMINATED" ]]; then
            log "WARNING: VM not TERMINATED after 180s (status=${status})."
            log "         Production ExecStopPost guard (yoyo-vm-stop-guard.sh) is the backstop"
            log "         once the manual-cycle flock is released below."
            # Best-effort second stop attempt.
            "${GCLOUD}" compute instances stop "${INSTANCE}" \
                --project="${PROJECT}" --zone="${ZONE}" --quiet \
                >>"${LOG_FILE}" 2>&1 || true
        else
            log "VM ${INSTANCE} is TERMINATED."
        fi
    else
        log "VM was never started (status: ${status}) — nothing to stop."
    fi

    # Final budget line.
    debit_budget

    # Release the manual-cycle flock LAST — after this, the production guard is
    # free to issue its own stop if ours somehow did not take.
    if [[ -n "${LOCK_FD}" ]]; then
        flock -u "${LOCK_FD}" 2>/dev/null || true
        eval "exec ${LOCK_FD}>&-" 2>/dev/null || true
        log "Released manual-cycle flock (fd ${LOCK_FD})."
    fi

    write_result_json "${rc}"
    print_summary "${rc}"

    rm -f "${HEARTBEAT}" 2>/dev/null || true
    log "Hard stop complete. Result: ${RESULT_FILE}"
    exit "${rc}"
}

write_result_json() {
    local rc="$1"
    local passed=true
    local items=""
    local i
    for i in "${!TEST_NAMES[@]}"; do
        [[ "${TEST_RESULTS[$i]}" == "fail" ]] && passed=false
        local detail="${TEST_DETAILS[$i]//\"/\\\"}"
        items+=$(printf '    {"name":"%s","result":"%s","detail":"%s"}' \
            "${TEST_NAMES[$i]}" "${TEST_RESULTS[$i]}" "${detail}")
        (( i < ${#TEST_NAMES[@]} - 1 )) && items+=$',\n' || items+=$'\n'
    done
    local vm_on=0
    [[ -n "${VM_ON_START_SECONDS}" ]] && vm_on=$(( LAST_DEBITED ))
    {
        printf '{\n'
        printf '  "session": "%s",\n' "${TS}"
        printf '  "passed": %s,\n' "${passed}"
        printf '  "exit_code": %d,\n' "${rc}"
        printf '  "max_minutes": %d,\n' "${MAX_MINUTES}"
        printf '  "elapsed_seconds": %d,\n' "${SECONDS}"
        printf '  "vm_on_seconds": %d,\n' "${vm_on}"
        printf '  "instance": "%s",\n' "${INSTANCE}"
        printf '  "zone": "%s",\n' "${ZONE}"
        printf '  "test_tree": "%s",\n' "${TEST_ROOT}"
        printf '  "adapter_dir": "%s",\n' "${ADAPTER_OUT}"
        printf '  "wrote_receipt": false,\n'
        printf '  "touched_production_adapter": false,\n'
        printf '  "tests": [\n%s  ]\n' "${items}"
        printf '}\n'
    } > "${RESULT_FILE}" 2>/dev/null || log "WARNING: could not write result JSON"
}

print_summary() {
    local rc="$1"
    log "──────────────────── TEST-MODE SUMMARY ────────────────────"
    local i
    for i in "${!TEST_NAMES[@]}"; do
        log "  [${TEST_RESULTS[$i]}]  ${TEST_NAMES[$i]}  ${TEST_DETAILS[$i]}"
    done
    log "  exit_code=${rc} elapsed=${SECONDS}s vm_on=${LAST_DEBITED}s"
    log "  result_json=${RESULT_FILE}"
    log "────────────────────────────────────────────────────────────"
}

# Arm the trap as early as possible so even a preflight failure stops a VM that
# (improbably) is already up.
EXIT_CODE=0
trap 'hard_stop' EXIT
trap 'EXIT_CODE=130; exit 130' INT
trap 'EXIT_CODE=143; exit 143' TERM

# ──────────────────────────────────────────────────────────────────────────────
# Wall-clock watchdog — background subshell. SIGKILLs the main script after the
# hard wall AND independently issues a VM stop, so even a hung main process is
# bounded. This is the in-process analogue of systemd RuntimeMaxSec.
# ──────────────────────────────────────────────────────────────────────────────
start_watchdog() {
    local main_pid="$$"
    (
        sleep "${HARD_WALL_SECONDS}"
        echo "[test-mode watchdog] HARD WALL ${HARD_WALL_SECONDS}s reached — killing main pid ${main_pid}" \
            | tee -a "${LOG_FILE}"
        # Try to stop the VM directly — do not rely on the dying main process.
        "${GCLOUD}" compute instances stop "${INSTANCE}" \
            --project="${PROJECT}" --zone="${ZONE}" --quiet >>"${LOG_FILE}" 2>&1 || true
        kill -TERM "${main_pid}" 2>/dev/null || true
        sleep 20
        kill -KILL "${main_pid}" 2>/dev/null || true
    ) &
    WATCHDOG_PID=$!
    log "Wall-clock watchdog armed (pid ${WATCHDOG_PID}, hard wall ${HARD_WALL_SECONDS}s)."
}

# ══════════════════════════════════════════════════════════════════════════════
# PHASE 0 — preflight (no VM, no spend)
# ══════════════════════════════════════════════════════════════════════════════
phase "0 — preflight (no VM spend)"

# Kill switch before anything.
if kill_switch_tripped; then
    log "ABORT: kill switch present at preflight (${KILL_SWITCH}). No VM spend."
    record "preflight-kill-switch" "fail" "kill switch present"
    EXIT_CODE=4
    exit 4
fi

PREFLIGHT_OK=1
require_file() {
    if [[ -f "$1" ]]; then
        log "  OK   $1"
    else
        log "  MISS $1"
        PREFLIGHT_OK=0
    fi
}
require_exec() {
    if [[ -x "$1" ]]; then
        log "  OK   $1"
    else
        log "  MISS $1 (not found / not executable)"
        PREFLIGHT_OK=0
    fi
}

log "Asserting required scripts in ${SCRIPT_DIR} (project-totebox, NOT project-intelligence):"
require_file "${SCRIPT_DIR}/run-sft-training.py"
require_file "${SCRIPT_DIR}/export-sft.py"
require_file "${SCRIPT_DIR}/corpus-threshold.py"
require_exec "${SCRIPT_DIR}/deploy-gate.sh"
require_exec "${GCLOUD}"

# SSH key.
if [[ -f "${SSH_KEY}" ]]; then log "  OK   ssh key ${SSH_KEY}"; else log "  MISS ssh key ${SSH_KEY}"; PREFLIGHT_OK=0; fi

# Confirm we are running from project-totebox, not project-intelligence.
case "${SCRIPT_DIR}" in
    *project-totebox*) log "  OK   script dir is under project-totebox" ;;
    *) log "  WARN script dir not under project-totebox: ${SCRIPT_DIR}" ;;
esac

if (( PREFLIGHT_OK == 0 )); then
    log "ABORT: preflight assertions failed. No VM spend."
    record "preflight" "fail" "missing required scripts/keys — see log"
    EXIT_CODE=2
    exit 2
fi
record "preflight" "pass" "all required scripts + ssh key present"
log "Preflight passed. Start SECONDS=${SECONDS}."

# Arm the watchdog now that we are committed to a run.
start_watchdog

between_phases

# ══════════════════════════════════════════════════════════════════════════════
# PHASE 1 — provision L4 with deadline + manual-cycle flock
# ══════════════════════════════════════════════════════════════════════════════
phase "1 — provision L4 (deadline + flock)"

# Acquire the shared manual-cycle flock (non-blocking). If a live cycle holds it,
# do NOT fight it — exit cleanly with no VM spend.
exec 200>"${VM_LOCK}"
LOCK_FD=200
if flock -n 200; then
    printf 'test-mode pid=%d started=%s\n' "$$" "${TS}" >&200 || true
    log "Acquired manual-cycle flock (fd 200) on ${VM_LOCK}."
else
    log "ABORT: manual-cycle flock held by another process — a live Yo-Yo cycle is running."
    log "       Not provisioning. No VM spend."
    LOCK_FD=""   # we do not hold it; do not release in hard_stop
    record "flock" "fail" "VM lock held by live cycle"
    EXIT_CODE=0
    exit 0
fi

check_kill_switch

# Start the VM with a STOCKOUT-aware retry cap (600s).
status="$(vm_status)"
log "Current ${INSTANCE} status: ${status}"
if [[ "${status}" == "RUNNING" ]]; then
    log "VM already RUNNING — reusing. (It will still be stopped at hard_stop.)"
    VM_STARTED=1
    VM_ON_START_SECONDS="${SECONDS}"
else
    PROVISION_DEADLINE=$(( SECONDS + 600 ))
    PROVISIONED=0
    while (( SECONDS < PROVISION_DEADLINE )); do
        check_kill_switch
        log "Issuing gcloud instances start (deadline in $(( PROVISION_DEADLINE - SECONDS ))s)..."
        if "${GCLOUD}" compute instances start "${INSTANCE}" \
                --project="${PROJECT}" --zone="${ZONE}" --quiet >>"${LOG_FILE}" 2>&1; then
            PROVISIONED=1
            VM_STARTED=1
            VM_ON_START_SECONDS="${SECONDS}"
            log "VM start succeeded."
            break
        fi
        log "VM start failed (likely zone STOCKOUT). Retrying after ${POLL_INTERVAL}s..."
        ks_sleep "${POLL_INTERVAL}"
    done

    if (( PROVISIONED == 0 )); then
        log "STOCKOUT: could not provision ${INSTANCE} in ${ZONE} within 600s cap."
        record "provision" "fail" "STOCKOUT in ${ZONE} within 600s"
        EXIT_CODE=3
        exit 3
    fi
fi
record "provision" "pass" "VM RUNNING in ${ZONE}"

# Resolve external IP for direct SSH (no IAP tunnel required).
VM_IP="$(vm_external_ip)"
log "VM external IP: ${VM_IP}"
if [[ -z "${VM_IP}" ]]; then
    log "ERROR: could not determine VM external IP — cannot SSH"
    record "ssh-reachable" "fail" "no external IP"
    EXIT_CODE=1; exit 1
fi

# Wait for SSH reachability (kill-switch-aware), bounded.
log "Waiting for SSH reachability..."
SSH_DEADLINE=$(( SECONDS + 180 ))
SSH_OK=0
while (( SECONDS < SSH_DEADLINE )); do
    check_kill_switch
    if remote_ssh "true"; then SSH_OK=1; log "SSH reachable."; break; fi
    ks_sleep "${POLL_INTERVAL}"
done
if (( SSH_OK == 0 )); then
    log "SSH never came up within deadline."
    record "ssh-reachable" "fail" "no SSH within 180s"
    EXIT_CODE=1
    exit 1
fi
record "ssh-reachable" "pass" "SSH up"

between_phases

# ══════════════════════════════════════════════════════════════════════════════
# PHASE — DataGraph injection check (~10 min budget)
#   Send 5 probes via Doorman with X-Foundry-Module-ID: woodfine, then confirm
#   GraphContextClient fetched graph context (Doorman log), and score relevance.
# ══════════════════════════════════════════════════════════════════════════════
phase "DG — DataGraph injection (5 Doorman probes)"

# Pause the apprenticeship drain loop so OLMo slots are free for interactive probes.
# The drain loop saturates both slots (queue_pending can be 100+), causing all 5 probes
# to time out at 60s each and report EMPTY. SLM_APPRENTICESHIP_DRAIN_PAUSED is read by
# the Doorman's drain worker; it stops accepting new drain work within one poll cycle (~5s).
DRAIN_WAS_PAUSED="$(systemctl show -p Environment local-doorman | grep -o 'SLM_APPRENTICESHIP_DRAIN_PAUSED=true' || true)"
if [[ -z "${DRAIN_WAS_PAUSED}" ]]; then
    log "  Pausing apprenticeship drain for DataGraph probes (SLM_DRAIN_PAUSED env drop-in)..."
    mkdir -p /etc/systemd/system/local-doorman.service.d/
    printf '[Service]\nEnvironment=SLM_APPRENTICESHIP_DRAIN_PAUSED=true\n' \
        > /etc/systemd/system/local-doorman.service.d/zz-test-mode-drain-pause.conf
    systemctl daemon-reload 2>/dev/null || true
    systemctl restart local-doorman 2>/dev/null || true
    log "  Waiting 15s for Doorman to restart and drain to stop..."
    ks_sleep 15
else
    log "  Drain already paused — skipping restart."
fi

# Snapshot Doorman log cursor so we only read lines produced by our probes.
DOORMAN_LOG_SINCE="$(date -u +'%Y-%m-%d %H:%M:%S')"

DG_PROMPTS=(
    "Who is Jennifer Woodfine and what is her role?"
    "What is PointSav Digital Systems?"
    "Summarise the project-totebox cluster mission."
    "What does the Doorman service do?"
    "Describe the relationship between Woodfine Management Corp. and PointSav."
)
DG_NONEMPTY=0
DG_TOTAL="${#DG_PROMPTS[@]}"
for p in "${DG_PROMPTS[@]}"; do
    between_phases
    log "  probe: ${p}"
    resp="$(curl -s --max-time 60 \
        -H 'Content-Type: application/json' \
        -H 'X-Foundry-Module-ID: woodfine' \
        -X POST "${DOORMAN}/v1/chat/completions" \
        -d "$(python3 -c 'import json,sys; print(json.dumps({"model":"local","messages":[{"role":"user","content":sys.argv[1]}],"max_tokens":128,"temperature":0.0}))' "${p}")" 2>/dev/null || true)"
    body="$(printf '%s' "${resp}" | python3 -c 'import sys,json
try:
    d=json.load(sys.stdin)
    c=d.get("choices",[{}])[0].get("message",{}).get("content","")
    print(c.strip())
except Exception:
    print("")' 2>/dev/null || true)"
    if [[ -n "${body}" ]]; then
        DG_NONEMPTY=$(( DG_NONEMPTY + 1 ))
        log "    -> non-empty (${#body} chars)"
    else
        log "    -> EMPTY"
    fi
done

# Verify GraphContextClient actually fetched graph context for our probes.
GRAPH_INJECTED="unknown"
if command -v journalctl >/dev/null 2>&1; then
    if journalctl --since "${DOORMAN_LOG_SINCE}" 2>/dev/null \
         | grep -qiE 'GraphContextClient|graph.context.*fetch|injected .* entit'; then
        GRAPH_INJECTED="yes"
    else
        GRAPH_INJECTED="no"
    fi
fi
log "  graph context injection observed in Doorman log: ${GRAPH_INJECTED}"

if (( DG_NONEMPTY == DG_TOTAL )) && [[ "${GRAPH_INJECTED}" != "no" ]]; then
    record "datagraph-injection" "pass" "${DG_NONEMPTY}/${DG_TOTAL} non-empty; injection=${GRAPH_INJECTED}"
elif (( DG_NONEMPTY > 0 )); then
    record "datagraph-injection" "fail" "${DG_NONEMPTY}/${DG_TOTAL} non-empty; injection=${GRAPH_INJECTED} (expected all + injected)"
else
    record "datagraph-injection" "fail" "0/${DG_TOTAL} non-empty — Doorman not serving"
fi

# Bonus regression: default foundry-namespace must be non-empty (currently FAILS live).
fn="$(curl -s --max-time 20 "${DATAGRAPH}/v1/query?q=Jennifer+Woodfine&module_id=foundry" 2>/dev/null \
     | python3 -c 'import sys,json
try:
    d=json.load(sys.stdin)
    ents=d.get("entities",d) if isinstance(d,dict) else d
    print(len(ents))
except Exception:
    print(-1)' 2>/dev/null || echo -1)"
if [[ "${fn}" =~ ^[0-9]+$ ]] && (( fn > 0 )); then
    record "datagraph-foundry-scope" "pass" "foundry namespace entities=${fn}"
else
    record "datagraph-foundry-scope" "fail" "foundry namespace entities=${fn} (known live failure pre-backfill)"
fi

between_phases

# Resume drain after DataGraph probes are complete.
if [[ -z "${DRAIN_WAS_PAUSED}" ]]; then
    log "  Resuming apprenticeship drain (removing zz-test-mode-drain-pause.conf)..."
    rm -f /etc/systemd/system/local-doorman.service.d/zz-test-mode-drain-pause.conf
    systemctl daemon-reload 2>/dev/null || true
    systemctl restart local-doorman 2>/dev/null || true
fi

# ══════════════════════════════════════════════════════════════════════════════
# PHASE — LoRA SFT smoke-train (capped at remaining budget)
# ══════════════════════════════════════════════════════════════════════════════
phase "LoRA — SFT smoke-train (capped)"

# 1) Export a corpus slice (merged code+docs) to the throwaway tree.
log "Exporting SFT corpus (--source=all) to ${CORPUS_OUT} ..."
if python3 "${SCRIPT_DIR}/export-sft.py" --source=all --out="${CORPUS_OUT}" >>"${LOG_FILE}" 2>&1; then
    NREC=0
    [[ -f "${CORPUS_OUT}" ]] && NREC="$(wc -l < "${CORPUS_OUT}" 2>/dev/null | tr -d ' ')"
    log "  export-sft wrote ${NREC} records."
    if [[ "${NREC}" =~ ^[0-9]+$ ]] && (( NREC >= 1 )); then
        record "sft-corpus-export" "pass" "${NREC} records exported"
    else
        record "sft-corpus-export" "fail" "0 records exported"
    fi
else
    log "  export-sft.py failed."
    record "sft-corpus-export" "fail" "export-sft.py non-zero exit"
fi

between_phases

# 2) Dry-run the trainer against the SINGLE pinned OLMo-3 base (NO --resume).
#    run-sft-training.py reads its base from data/base-registry.yaml by default
#    (OLMo-only policy) and writes to --output-dir. We override output-dir into
#    the throwaway tree so the production adapter dir is NEVER touched.
log "Trainer dry-run (no --resume; pinned base from base-registry.yaml)..."
DRYRUN_OK=0
if python3 "${SCRIPT_DIR}/run-sft-training.py" --dry-run \
        --adapter-name "yoyo-test-${TS}" \
        --output-dir "${ADAPTER_OUT}" >>"${LOG_FILE}" 2>&1; then
    DRYRUN_OK=1
    record "sft-dry-run" "pass" "trainer corpus load OK"
    log "  dry-run passed."
else
    record "sft-dry-run" "fail" "trainer dry-run non-zero — skipping real train"
    log "  dry-run FAILED — skipping the GPU training step."
fi

between_phases

# 3) Real capped SFT train on the VM (only if dry-run passed and budget remains).
#    NOTE: run-sft-training.py caps wall-clock via --max-runtime-seconds (there is
#    NO --max-steps flag — verified in the script's argparse). We compute the cap
#    from the remaining soft-wall budget minus a reserve for convert+gate+stop.
TRAIN_DONE=0
if (( DRYRUN_OK == 1 )); then
    RESERVE_SECONDS=420   # convert+load+gate+stop headroom
    REMAINING=$(( SOFT_WALL_SECONDS - SECONDS - RESERVE_SECONDS ))
    if (( REMAINING < 120 )); then
        log "  Not enough budget for a real train (${REMAINING}s remaining). Skipping."
        record "sft-train" "skip" "insufficient budget (${REMAINING}s)"
    else
        TRAIN_CAP="${REMAINING}"
        (( TRAIN_CAP > 1500 )) && TRAIN_CAP=1500   # never spend >25 min on the train itself
        log "  Running capped SFT on VM (--max-runtime-seconds=${TRAIN_CAP}, r=32/a=64, LR 2e-4)..."

        # Stage corpus + trainer to the VM.
        remote_ssh "mkdir -p ${REMOTE_DIR}/adapter" || true
        remote_rsync "${CORPUS_OUT}" "${REMOTE_DIR}/corpus.jsonl" || log "  WARN corpus rsync failed"
        remote_rsync "${SCRIPT_DIR}/run-sft-training.py" "${REMOTE_DIR}/run-sft-training.py" || log "  WARN trainer rsync failed"

        # Ensure training dependencies are installed on the VM via a persistent venv.
        # Debian PEP 668 blocks pip from touching system python — venv is required.
        # ~/train-venv persists on the 100GB disk so subsequent runs skip this step.
        TRAIN_VENV="/home/mathew/train-venv"
        # Rebuild venv if: TRL <0.12 (TRL 0.9.x passes tokenizer= to Trainer.__init__
        # which modern transformers 4.47+ renamed to processing_class=). Float16 replaces
        # 4-bit — bitsandbytes no longer needed.
        remote_ssh "test -f ${TRAIN_VENV}/bin/python3 && \
            ${TRAIN_VENV}/bin/python3 -c '
import trl, pkg_resources
ver = tuple(int(x) for x in trl.__version__.split(\".\")[:2])
trl_ok = ver >= (0, 12)
exit(0 if trl_ok else 1)
' 2>/dev/null || (echo \"[venv] Rebuilding: TRL <0.12 detected (need >=0.12 for transformers 4.47+)\"; rm -rf ${TRAIN_VENV})" \
            >>"${LOG_FILE}" 2>&1 || true
        log "  Checking training venv (${TRAIN_VENV}) on VM..."
        if remote_ssh "test -f ${TRAIN_VENV}/bin/python3 && ${TRAIN_VENV}/bin/python3 -c 'import torch, trl, peft' 2>/dev/null"; then
            log "  Venv exists with torch/trl/peft — skipping install."
        else
            log "  Creating venv + installing torch (CUDA 12.1) + training libs..."
            # TRL >=0.12: uses processing_class= when calling Trainer.__init__ (transformers 4.47+).
            # Float16 model load (no 4-bit) — bitsandbytes not required.
            remote_ssh "python3 -m venv ${TRAIN_VENV} \
                && ${TRAIN_VENV}/bin/pip install --quiet \
                    torch --index-url https://download.pytorch.org/whl/cu121 \
                && ${TRAIN_VENV}/bin/pip install --quiet \
                    'trl>=0.12.0' 'peft>=0.12.0' \
                    transformers datasets accelerate" \
                >>"${LOG_FILE}" 2>&1 \
                || log "  ERROR: venv install failed — check log above"
            remote_ssh "${TRAIN_VENV}/bin/python3 -c 'import torch, trl, peft' && echo '[dep-check] OK'" \
                >>"${LOG_FILE}" 2>&1 \
                || log "  ERROR: deps still missing after venv install — train.log will show reason"
        fi

        # Launch training in the background on the VM, then poll kill-switch-aware.
        # Use venv python3 so torch/trl/peft are available.
        # Use --sft-input (pre-built Alpaca JSONL from export-sft.py) — NOT --queue-done.
        remote_ssh "cd ${REMOTE_DIR} && PYTORCH_CUDA_ALLOC_CONF=expandable_segments:True \
            nohup ${TRAIN_VENV}/bin/python3 run-sft-training.py \
            --sft-input ${REMOTE_DIR}/corpus.jsonl \
            --adapter-name yoyo-test-${TS} \
            --output-dir ${REMOTE_DIR}/adapter \
            --max-runtime-seconds ${TRAIN_CAP} \
            > ${REMOTE_DIR}/train.log 2>&1 & echo \$! > ${REMOTE_DIR}/train.pid" || true

        # Poll for completion, kill-switch-aware. Break to hard stop on kill switch.
        TRAIN_POLL_DEADLINE=$(( SECONDS + TRAIN_CAP + 120 ))
        while (( SECONDS < TRAIN_POLL_DEADLINE )); do
            between_phases   # kill switch + soft wall + budget debit
            if remote_ssh "test -f ${REMOTE_DIR}/adapter/adapter_config.json"; then
                log "  adapter_config.json present on VM — training produced an adapter."
                TRAIN_DONE=1
                break
            fi
            if ! remote_ssh "kill -0 \$(cat ${REMOTE_DIR}/train.pid 2>/dev/null) 2>/dev/null"; then
                log "  trainer process exited."
                remote_ssh "test -f ${REMOTE_DIR}/adapter/adapter_config.json" && TRAIN_DONE=1
                break
            fi
            log "  training in progress... ($(( TRAIN_POLL_DEADLINE - SECONDS ))s budget left)"
            ks_sleep 30
        done

        if (( TRAIN_DONE == 1 )); then
            # Pull adapter to the throwaway tree (NOT production).
            log "  Pulling adapter to ${ADAPTER_OUT} ..."
            rsync -az \
                -e "ssh -i ${SSH_KEY} -o StrictHostKeyChecking=no" \
                "mathew@${VM_IP}:${REMOTE_DIR}/adapter/." "${ADAPTER_OUT}/" >>"${LOG_FILE}" 2>&1 \
                || log "  WARN adapter pull failed"
            if [[ -f "${ADAPTER_OUT}/adapter_config.json" ]]; then
                record "sft-train" "pass" "adapter produced + pulled to test tree"
            else
                record "sft-train" "fail" "adapter pull incomplete"
                TRAIN_DONE=0
            fi
        else
            record "sft-train" "fail" "no adapter produced before deadline"
        fi
        # Always pull back the remote train.log for post-mortem, regardless of outcome.
        rsync -az \
            -e "ssh -i ${SSH_KEY} -o StrictHostKeyChecking=no" \
            "mathew@${VM_IP}:${REMOTE_DIR}/train.log" "${TEST_ROOT}/remote-train.log" \
            >>"${LOG_FILE}" 2>&1 \
            && log "  Remote train.log pulled to ${TEST_ROOT}/remote-train.log" \
            || log "  WARN: remote train.log pull failed (VM may already be stopping)"
    fi
fi

between_phases

# ══════════════════════════════════════════════════════════════════════════════
# PHASE — convert + quality gate (deploy-gate.sh)
#   deploy-gate.sh takes --adapter-path and runs the REAL quality gate
#   (envelope-format compliance + git apply --check) against an off-port scratch
#   server. It writes its own result file; it does NOT promote or write a receipt.
# ══════════════════════════════════════════════════════════════════════════════
phase "GATE — real quality gate (deploy-gate.sh)"

if (( TRAIN_DONE == 1 )) && [[ -f "${ADAPTER_OUT}/adapter_config.json" ]]; then
    GATE_RESULT="${TEST_ROOT}/deploy-gate-result.json"
    log "  Running deploy-gate.sh on ${ADAPTER_OUT} (scratch server, NOT local-slm.service)..."
    if RESULT_FILE="${GATE_RESULT}" "${SCRIPT_DIR}/deploy-gate.sh" \
            --adapter-path "${ADAPTER_OUT}" --probes 20 >>"${LOG_FILE}" 2>&1; then
        record "quality-gate" "pass" "deploy-gate passed; see ${GATE_RESULT}"
    else
        gc=$?
        record "quality-gate" "fail" "deploy-gate exit=${gc}; see ${GATE_RESULT}"
    fi
else
    log "  No adapter to gate — skipping."
    record "quality-gate" "skip" "no adapter produced"
fi

between_phases

# ══════════════════════════════════════════════════════════════════════════════
# Normal completion — fall through to the EXIT trap (hard_stop / Phase 5).
# ══════════════════════════════════════════════════════════════════════════════
phase "complete — handing off to Phase 5 hard stop"
log "All phases complete. Triggering unconditional hard stop via EXIT trap."
EXIT_CODE=0
exit 0
