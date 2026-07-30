#!/usr/bin/env bash
set -euo pipefail

if [ -z "${REMOTE_TARGET:-}" ] || [ -z "${TOTEBOX_ROOT:-}" ]; then
    echo "[ERROR] Environment variables REMOTE_TARGET and TOTEBOX_ROOT must be set."
    exit 1
fi

LOCAL_EXPORT_DIR="${LOCAL_EXPORT_DIR:-./Sovereign-Exports/Content}"

echo "[SYSTEM] Guaranteeing remote vault boundaries exist (with Sudo)..."
ssh "$REMOTE_TARGET" "sudo mkdir -p \"${TOTEBOX_ROOT}/service-content/knowledge-graph\" \"${TOTEBOX_ROOT}/service-content/verified-ledger\" \"${TOTEBOX_ROOT}/service-slm/transient-queues\""

echo "[SYSTEM] Extracting Raw, Transient, and Verified Content Ledgers..."
mkdir -p "$LOCAL_EXPORT_DIR/knowledge-graph" "$LOCAL_EXPORT_DIR/verified-ledger" "$LOCAL_EXPORT_DIR/raw-transient-queues"

# The Patch: Injecting --rsync-path="sudo rsync" to breach the woodfine-operator lock
rsync -avz --rsync-path="sudo rsync" "$REMOTE_TARGET:${TOTEBOX_ROOT}/service-slm/transient-queues/" "$LOCAL_EXPORT_DIR/raw-transient-queues/"
rsync -avz --rsync-path="sudo rsync" "$REMOTE_TARGET:${TOTEBOX_ROOT}/service-content/knowledge-graph/" "$LOCAL_EXPORT_DIR/knowledge-graph/"
rsync -avz --rsync-path="sudo rsync" "$REMOTE_TARGET:${TOTEBOX_ROOT}/service-content/verified-ledger/" "$LOCAL_EXPORT_DIR/verified-ledger/"

echo "[SUCCESS] Full Content Pipeline extracted safely to: $LOCAL_EXPORT_DIR"
