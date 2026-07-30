#!/usr/bin/env bash
# Yo-Yo adapter-publish trigger.
#
# Invoked by lora-training.service via `systemctl start adapter-publish.service`
# with ADAPTER_OUT_DIR pointing at the freshly-trained adapter directory.
# Uploads the adapter to gs://<bucket>/adapters/<tenant>/<role>/v<n>/ and emits
# a completion event Doorman can poll.
#
# This unit is implicitly active (oneshot, started on demand). It does not run
# at boot. The scaffold is in place from day 1 so when LoRA training is
# ratified, no image rebuild is required.

set -euo pipefail

ADAPTER_OUT_DIR="${ADAPTER_OUT_DIR:-}"
LOG_FILE="/var/log/yoyo-adapter-publish.log"

mkdir -p "$(dirname "${LOG_FILE}")"
exec >> >(tee -a "${LOG_FILE}") 2>&1

log() { echo "[adapter-publish $(date -u +'%Y-%m-%dT%H:%M:%SZ')] $*"; }

if [[ -z "${ADAPTER_OUT_DIR}" ]]; then
    log "ERROR: ADAPTER_OUT_DIR env var not set. Nothing to publish."
    exit 1
fi
if [[ ! -d "${ADAPTER_OUT_DIR}" ]]; then
    log "ERROR: ${ADAPTER_OUT_DIR} does not exist or is not a directory."
    exit 1
fi

# Read GCS bucket from instance metadata
GCS_BUCKET=$(curl -fsS --max-time 5 -H 'Metadata-Flavor: Google' \
    "http://metadata.google.internal/computeMetadata/v1/instance/attributes/weights-gcs-bucket" 2>/dev/null || echo "")
if [[ -z "${GCS_BUCKET}" ]]; then
    log "FATAL: instance metadata weights-gcs-bucket not set."
    exit 2
fi

# Compute target prefix from path layout
# Expected ADAPTER_OUT_DIR shape: /data/weights/adapters/<tenant>/<role>/v<n>
ADAPTER_REL="${ADAPTER_OUT_DIR#/data/weights/adapters/}"
GCS_TARGET="gs://${GCS_BUCKET}/adapters/${ADAPTER_REL}/"

log "Publishing ${ADAPTER_OUT_DIR} → ${GCS_TARGET}"
gcloud storage cp -r "${ADAPTER_OUT_DIR}/*" "${GCS_TARGET}"

# Emit completion event for Doorman to discover
EVENT_TIME=$(date -u +'%Y-%m-%dT%H:%M:%SZ')
EVENT_PATH="gs://${GCS_BUCKET}/adapters/.events/$(date -u +'%Y%m%dT%H%M%SZ')-${ADAPTER_REL//\//_}.json"
echo "{\"event\":\"adapter-published\",\"path\":\"${ADAPTER_REL}\",\"at\":\"${EVENT_TIME}\"}" \
    | gcloud storage cp - "${EVENT_PATH}"

log "Published ${ADAPTER_REL}; event written to ${EVENT_PATH}."
exit 0
