#!/usr/bin/env bash
# phase-d-monitor.sh — Poll for Phase D readiness and trigger verification steps.
#
# Phase D = end-to-end proof:
#   training receipt (LoRA ran successfully) →
#   deploy-gate.sh (base vs adapter delta probe) →
#   lora-scaled-dropin.sh --apply →
#   sudo systemctl restart local-slm.service →
#   mailbox notification
#
# Run via crontab every 15 minutes:
#   */15 * * * * /srv/foundry/clones/project-totebox/service-slm/scripts/phase-d-monitor.sh
#
# Completion marker: /srv/foundry/data/phase-d-complete
# To re-run: rm /srv/foundry/data/phase-d-complete

set -uo pipefail

FOUNDRY_ROOT="${FOUNDRY_ROOT:-/srv/foundry}"
ARCHIVE_ROOT="${FOUNDRY_ROOT}/clones/project-totebox"
SCRIPTS_DIR="${ARCHIVE_ROOT}/service-slm/scripts"
ADAPTERS_DIR="${FOUNDRY_ROOT}/data/adapters"
ADAPTER_PATH="${ADAPTERS_DIR}/apprenticeship-pointsav-incremental"
COMPLETE_MARKER="${FOUNDRY_ROOT}/data/phase-d-complete"
LOG_FILE="${FOUNDRY_ROOT}/data/phase-d-monitor.log"
RECEIPTS_DIR="${FOUNDRY_ROOT}/data/training-approved"

log() { echo "[$(date -u +%Y-%m-%dT%H:%M:%SZ)] $*" | tee -a "${LOG_FILE}"; }

# ── Already done ──────────────────────────────────────────────────────────────
if [[ -f "${COMPLETE_MARKER}" ]]; then
    exit 0
fi

# ── Check training receipt ────────────────────────────────────────────────────
# Training runs daily; look for any receipt within the last 3 days.
RECEIPT_FOUND=""
for i in 0 1 2; do
    _date=$(date -u -d "${i} days ago" +%Y-%m-%d 2>/dev/null || date -u -v-${i}d +%Y-%m-%d)
    _receipt="${RECEIPTS_DIR}/coding-lora-${_date}.ran"
    if [[ -f "${_receipt}" ]]; then
        RECEIPT_FOUND="${_receipt}"
        break
    fi
done

if [[ -z "${RECEIPT_FOUND}" ]]; then
    log "Phase D: no training receipt yet — waiting"
    exit 0
fi

log "Phase D: training receipt found: ${RECEIPT_FOUND}"
log "Phase D: receipt contents: $(cat "${RECEIPT_FOUND}")"

# ── Check Doorman health ──────────────────────────────────────────────────────
DOORMAN_HEALTH=$(curl -s --connect-timeout 3 "http://127.0.0.1:9080/health" 2>/dev/null)
TIER_A_UP=$(echo "${DOORMAN_HEALTH}" | python3 -c "import sys,json; d=json.load(sys.stdin); print(d.get('tier_a',{}).get('health_up','false'))" 2>/dev/null || echo "false")
TIER_B_UP=$(echo "${DOORMAN_HEALTH}" | python3 -c "import sys,json; d=json.load(sys.stdin); print(d.get('tier_b',{}).get('health_up','false'))" 2>/dev/null || echo "false")

log "Phase D: Tier A health_up=${TIER_A_UP} Tier B health_up=${TIER_B_UP}"

# ── Check adapter exists ──────────────────────────────────────────────────────
if [[ ! -d "${ADAPTER_PATH}" ]]; then
    log "Phase D: adapter not yet pulled to ${ADAPTER_PATH} — waiting"
    exit 0
fi

ADAPTER_TYPE="peft"
GGUF_PATH="${ADAPTER_PATH%.*/}.gguf"
if [[ -f "${ADAPTER_PATH}.gguf" ]]; then
    ADAPTER_TYPE="gguf"
    GGUF_PATH="${ADAPTER_PATH}.gguf"
fi

log "Phase D: adapter found at ${ADAPTER_PATH} (type=${ADAPTER_TYPE})"

# ── Run deploy-gate.sh (Tier A base probe — no GPU needed) ───────────────────
if [[ "${TIER_A_UP}" == "True" || "${TIER_A_UP}" == "true" ]]; then
    log "Phase D: running deploy-gate.sh (20 probes, base vs adapter)"
    GATE_RESULT=0
    bash "${SCRIPTS_DIR}/deploy-gate.sh" \
        --adapter-path "${ADAPTER_PATH}" \
        --probes 20 2>&1 | tee -a "${LOG_FILE}" || GATE_RESULT=$?
    log "Phase D: deploy-gate.sh exited rc=${GATE_RESULT}"
else
    log "Phase D: Tier A down — skipping deploy-gate.sh; will retry"
    exit 0
fi

# ── GGUF conversion required before --lora-scaled ────────────────────────────
if [[ "${ADAPTER_TYPE}" == "peft" ]]; then
    log "Phase D: PEFT adapter needs GGUF conversion before --lora-scaled"
    log "Phase D: run on yoyo-batch VM when next online:"
    log "  python3 convert_lora_to_gguf.py --base /data/weights/olmo-3-7b-instruct-hf ${ADAPTER_PATH}"
    log "  # then pull the .gguf back to this VM"
    DROPIN_READY=false
else
    DROPIN_READY=true
fi

# ── Apply dropin if GGUF ready and gate passed ────────────────────────────────
DROPIN_APPLIED=false
if [[ "${DROPIN_READY}" == "true" && "${GATE_RESULT}" -eq 0 ]]; then
    log "Phase D: gate PASSED + GGUF ready — applying lora-scaled drop-in"
    if bash "${SCRIPTS_DIR}/lora-scaled-dropin.sh" --adapter-path "${GGUF_PATH}" --apply 2>&1 | tee -a "${LOG_FILE}"; then
        log "Phase D: drop-in applied — restarting local-slm.service"
        if sudo systemctl restart local-slm.service 2>&1 | tee -a "${LOG_FILE}"; then
            log "Phase D: local-slm.service restarted with adapter"
            DROPIN_APPLIED=true
        else
            log "Phase D: WARNING — systemctl restart failed; may need manual restart"
        fi
    fi
fi

# ── Write completion marker ───────────────────────────────────────────────────
GATE_STATUS="PASS"
[[ "${GATE_RESULT}" -ne 0 ]] && GATE_STATUS="FAIL"
[[ "${DROPIN_READY}" == "false" ]] && GATE_STATUS="PEFT-PENDING-CONVERSION"

{
    echo "date=$(date -u +%Y-%m-%dT%H:%M:%SZ)"
    echo "receipt=${RECEIPT_FOUND}"
    echo "adapter_type=${ADAPTER_TYPE}"
    echo "gate_rc=${GATE_RESULT}"
    echo "gate_status=${GATE_STATUS}"
    echo "dropin_applied=${DROPIN_APPLIED}"
    echo "tier_b_up=${TIER_B_UP}"
} > "${COMPLETE_MARKER}"

log "Phase D: complete marker written: ${COMPLETE_MARKER}"

# ── Mailbox notification ──────────────────────────────────────────────────────
BODY="Phase D monitor completed.

Training receipt: ${RECEIPT_FOUND}
$(cat "${RECEIPT_FOUND}")

Adapter: ${ADAPTER_PATH} (type=${ADAPTER_TYPE})
Deploy gate: rc=${GATE_RESULT} (${GATE_STATUS})
Drop-in applied: ${DROPIN_APPLIED}
Tier B online: ${TIER_B_UP}

$(if [[ "${ADAPTER_TYPE}" == "peft" ]]; then
echo "NEXT STEP (GGUF conversion needed):
  1. Wait for yoyo-batch to come online
  2. SSH to VM: ssh mathew@<VM_IP>
  3. Run: python3 convert_lora_to_gguf.py --base /data/weights/olmo-3-7b-instruct-hf ${ADAPTER_PATH}
  4. Pull GGUF to workspace: scp mathew@<VM_IP>:${ADAPTER_PATH}.gguf ${ADAPTER_PATH}.gguf
  5. rm ${COMPLETE_MARKER} && bash ${SCRIPTS_DIR}/phase-d-monitor.sh"
elif [[ "${GATE_RESULT}" -ne 0 ]]; then
echo "NEXT STEP: deploy gate FAILED — check ${LOG_FILE} for details"
else
echo "Phase D complete. local-slm is serving with adapter.
Verify: curl http://127.0.0.1:8080/v1/models | python3 -m json.tool"
fi)

Full log: ${LOG_FILE}"

python3 -c "
import subprocess, sys
msg = sys.argv[1]
subject = sys.argv[2]
subprocess.run([
    '${FOUNDRY_ROOT}/bin/mailbox-send.sh',
    '--to', 'totebox@project-totebox',
    '--re', subject,
    '--body-stdin'
], input=msg.encode(), check=False)
" "${BODY}" "phase-d-complete — ${GATE_STATUS}" 2>/dev/null \
|| log "Phase D: mailbox send failed (non-fatal)"

log "Phase D: monitor run complete (gate=${GATE_STATUS} dropin=${DROPIN_APPLIED})"
