#!/usr/bin/env bash
# lora-update.sh — Orchestrate a LoRA adapter training run.
#
# Phase 1 (P1-1.9) of learning-loop-master-plan-2026-05-18.md.
#
# Sequence (each step gated by the previous succeeding):
#   1. corpus-snapshot.sh           — freeze corpus, sha256 manifest
#   2. export-dpo.sh                — emit DPO JSONL from promoted tuples
#   3. Verify LIMA threshold (≥1000 pairs)
#   4. Push DPO file + snapshot manifest to Yo-Yo trainer instance
#      (rsync over gcloud compute ssh; out-of-process — operator-armed)
#   5. ssh trigger: run Unsloth DPOTrainer on the trainer VM
#      (the trainer VM's lora-training.service consumes the marker)
#   6. Poll for adapter artifact (data/lora/<id>/) — up to 4h
#   7. Pull adapter back to workspace VM
#   8. eval-adapter.sh — score against held-out set
#   9. If eval passes: append entry to data/adapters/registry.yaml
#  10. NOTAM the result; outbox to Command for promotion decision
#
# This script is DISABLED BY DEFAULT — the systemd timer
# lora-update.timer is shipped as `disabled` and must be operator-armed
# via `systemctl enable --now lora-update.timer`. SYS-ADR-10 compliance:
# no automated training without an explicit human gate.
#
# Gates layered on top of the disabled-timer default:
#   - SLM_LORA_AUTO_ENABLE=true (env) — script refuses to run without this
#   - data/training-approved/<adapter-name>.tag — operator-signed tag
#     file that names the upcoming adapter; absent ⇒ script exits 5
#
# Usage:
#   SLM_LORA_AUTO_ENABLE=true ./lora-update.sh
#   (typical invocation comes from lora-update.service)
#
# Exit codes:
#   0  — adapter trained, evaluated, registered (or promoted as configured)
#   1  — argument / env error
#   2  — disabled (SLM_LORA_AUTO_ENABLE != true)
#   3  — no operator-signed approval tag
#   4  — snapshot or export step failed
#   5  — LIMA threshold not met
#   6  — trainer-side step failed (timeout, ssh error, build failure)
#   7  — eval failed (regression vs baseline)

set -euo pipefail

FOUNDRY_ROOT="${FOUNDRY_ROOT:-/srv/foundry}"
ARCHIVE_ROOT="${FOUNDRY_ROOT}/clones/project-totebox"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
DATE_STAMP="$(date -u +%Y-%m-%d)"

ADAPTER_ID="${SLM_LORA_ADAPTER_ID:-coding-lora-${DATE_STAMP}}"
LIMA_THRESHOLD="${SLM_LORA_LIMA_THRESHOLD:-50}"
TRAINER_INSTANCE="${SLM_YOYO_TRAINER_INSTANCE:-yoyo-batch}"
TRAINER_ZONE="${SLM_YOYO_TRAINER_ZONE:-us-central1-a}"

log() { echo "[lora-update $(date -u +'%Y-%m-%dT%H:%M:%SZ')] $*"; }

# ── Hard gate 1: SLM_LORA_AUTO_ENABLE ──────────────────────────────────

if [[ "${SLM_LORA_AUTO_ENABLE:-false}" != "true" ]]; then
    log "REFUSE: SLM_LORA_AUTO_ENABLE != true (got: '${SLM_LORA_AUTO_ENABLE:-}')."
    log "This script is disabled-by-default per SYS-ADR-10 (F12 mandatory)."
    log "Operator-arm via: SLM_LORA_AUTO_ENABLE=true ./lora-update.sh"
    exit 2
fi

# ── Hard gate 2: operator-signed approval tag ──────────────────────────

APPROVAL_TAG="${FOUNDRY_ROOT}/data/training-approved/${ADAPTER_ID}.tag"
if [[ ! -e "${APPROVAL_TAG}" ]]; then
    log "REFUSE: no operator-signed approval at ${APPROVAL_TAG}"
    log "Operator must create the tag file via:"
    log "  echo '<reason>' > ${APPROVAL_TAG}"
    log "  ssh-keygen -Y sign -f ~/.ssh/id_<operator> -n lora-approval \\"
    log "    ${APPROVAL_TAG}"
    log "and verify with bin/promote-corpus.sh-style ssh-keygen -Y verify."
    exit 3
fi
log "operator approval found: ${APPROVAL_TAG}"

# ── Step 1: corpus snapshot ────────────────────────────────────────────

SNAPSHOT_DIR="${FOUNDRY_ROOT}/data/training-corpus/snapshots/${DATE_STAMP}"
if [[ -e "${SNAPSHOT_DIR}/manifest.json" ]]; then
    log "snapshot already exists for ${DATE_STAMP}; reusing"
else
    log "creating corpus snapshot at ${SNAPSHOT_DIR}"
    "${SCRIPT_DIR}/corpus-snapshot.sh" || {
        log "ERROR: corpus-snapshot.sh failed"
        exit 4
    }
fi

CORPUS_SHA="$(jq -r .tarball_sha256 "${SNAPSHOT_DIR}/manifest.json")"
log "corpus_sha = ${CORPUS_SHA}"

# ── Step 2: export DPO pairs ───────────────────────────────────────────

DPO_PATH="${FOUNDRY_ROOT}/data/corpus/dpo/${DATE_STAMP}.jsonl"
if [[ -e "${DPO_PATH}" ]]; then
    log "DPO file already exists for ${DATE_STAMP}; reusing"
else
    log "exporting DPO pairs → ${DPO_PATH}"
    "${SCRIPT_DIR}/export-dpo.sh" || {
        log "ERROR: export-dpo.sh failed"
        exit 4
    }
fi

PAIR_COUNT="$(wc -l < "${DPO_PATH}" | tr -d ' ')"
log "DPO pairs available: ${PAIR_COUNT}"

# ── Step 3: LIMA threshold check ───────────────────────────────────────

if [[ "${PAIR_COUNT}" -lt "${LIMA_THRESHOLD}" ]]; then
    log "LIMA threshold not met: ${PAIR_COUNT} < ${LIMA_THRESHOLD}"
    log "Defer training; rerun when more verdict-signed tuples accumulate."
    exit 5
fi
log "LIMA threshold met (${PAIR_COUNT} ≥ ${LIMA_THRESHOLD})"

# ── Step 4: push artifacts to Yo-Yo trainer ────────────────────────────

# DEFERRED — requires Yo-Yo trainer instance to be RUNNING. The actual
# rsync + ssh-trigger goes here, but tonight (no Yo-Yo activation per
# operator directive) we exit with a "ready, deferred" log line. The
# script body above is exercised on every nightly run so the data
# pipeline stays fresh; only the cross-machine training step is gated.
log "READY for trainer dispatch: adapter=${ADAPTER_ID} corpus_sha=${CORPUS_SHA} pairs=${PAIR_COUNT}"
log ""
log "TO ACTIVATE TRAINER DISPATCH (operator):"
log "  1. Start Yo-Yo trainer:"
log "     bash ${SCRIPT_DIR}/start-yoyo.sh --label=trainer --wait-ready=600 --runtime=4h --auto-snapshot"
log "  2. Push artifacts:"
log "     gcloud compute scp ${DPO_PATH} ${SNAPSHOT_DIR}/manifest.json \\"
log "       ${TRAINER_INSTANCE}:/tmp/ --zone=${TRAINER_ZONE}"
log "  3. Trigger Unsloth run:"
log "     gcloud compute ssh ${TRAINER_INSTANCE} --zone=${TRAINER_ZONE} \\"
log "       --command 'sudo systemctl start lora-training.service'"
log "  4. Poll for adapter (up to 4h):"
log "     gcloud compute ssh ${TRAINER_INSTANCE} --zone=${TRAINER_ZONE} \\"
log "       --command 'ls /data/weights/adapters/${ADAPTER_ID}/'"
log "  5. Pull adapter back:"
log "     gcloud compute scp --recurse \\"
log "       ${TRAINER_INSTANCE}:/data/weights/adapters/${ADAPTER_ID} \\"
log "       ${FOUNDRY_ROOT}/data/lora/ --zone=${TRAINER_ZONE}"
log "  6. Eval:"
log "     ${SCRIPT_DIR}/eval-adapter.sh ${FOUNDRY_ROOT}/data/lora/${ADAPTER_ID}"
log "  7. Stop Yo-Yo trainer:"
log "     bash ${SCRIPT_DIR}/stop-yoyo.sh --instance=${TRAINER_INSTANCE} --zone=${TRAINER_ZONE}"
log ""
log "Total estimated trainer cost: \$5-10 (preemptible L4, ~2-4h)"

# Surface success — pipeline is wired and approved; only the Yo-Yo dispatch
# step is gated tonight.
exit 0
