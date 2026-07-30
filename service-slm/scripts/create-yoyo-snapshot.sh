#!/usr/bin/env bash
# Create a GCP snapshot of the Yo-Yo weights disk so that zone migrations
# can restore weights automatically (no re-upload needed).
#
# Run this once after uploading model weights to the disk.
# The snapshot is global — visible to all zones in the project.
#
# Usage:
#   ./scripts/create-yoyo-snapshot.sh
#
# On success, prints the snapshot name and the line to add to
# /etc/local-doorman/local-doorman.env.  The snapshot name is also written
# to DOORMAN_ENV_FILE automatically if the file is writable.
set -uo pipefail

PROJECT="${SLM_YOYO_GCP_PROJECT:-woodfine-node-gcp-free}"
ZONE="${SLM_YOYO_GCP_ZONE:-us-west1-b}"
INSTANCE="${SLM_YOYO_GCP_INSTANCE:-yoyo-tier-b-1}"
DOORMAN_ENV="${DOORMAN_ENV_FILE:-/etc/local-doorman/local-doorman.env}"
WEIGHTS_DISK="${INSTANCE}-weights"
SNAPSHOT_NAME="${INSTANCE}-weights-$(date -u +%Y%m%d-%H%M)"

echo "Creating snapshot ${SNAPSHOT_NAME} of disk ${WEIGHTS_DISK} in ${PROJECT}/${ZONE}..."
echo "Note: the disk must be attached to a running VM; GCP snapshots a live disk safely."

if ! gcloud compute disks snapshot "${WEIGHTS_DISK}" \
        --project="${PROJECT}" \
        --zone="${ZONE}" \
        --snapshot-names="${SNAPSHOT_NAME}" \
        --storage-location=us; then
    echo "ERROR: snapshot creation failed." >&2
    exit 1
fi

echo ""
echo "Snapshot created: ${SNAPSHOT_NAME}"
echo ""
echo "Add to ${DOORMAN_ENV}:"
echo "  SLM_YOYO_WEIGHTS_SNAPSHOT=${SNAPSHOT_NAME}"
echo ""

if [[ -w "${DOORMAN_ENV}" ]]; then
    if grep -q "^SLM_YOYO_WEIGHTS_SNAPSHOT=" "${DOORMAN_ENV}"; then
        sed -i "s|^SLM_YOYO_WEIGHTS_SNAPSHOT=.*|SLM_YOYO_WEIGHTS_SNAPSHOT=${SNAPSHOT_NAME}|" "${DOORMAN_ENV}"
    else
        echo "SLM_YOYO_WEIGHTS_SNAPSHOT=${SNAPSHOT_NAME}" >> "${DOORMAN_ENV}"
    fi
    echo "Written to ${DOORMAN_ENV} automatically."
else
    echo "Cannot write to ${DOORMAN_ENV} — add the line above manually."
fi

echo ""
echo "Export for this shell session:"
echo "  export SLM_YOYO_WEIGHTS_SNAPSHOT=${SNAPSHOT_NAME}"
