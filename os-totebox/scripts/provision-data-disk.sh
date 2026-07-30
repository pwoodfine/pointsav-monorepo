#!/usr/bin/env bash
# provision-data-disk.sh — Create the 8 GiB data QCOW2 for OLMo 7B weights.
#
# Produces: build/os-totebox-data.qcow2 (sparse QCOW2, up to 8 GiB)
# The guest mounts this as /dev/vdb (second virtio-blk device).
# Filesystem: FFS2 — formatted on first boot by /etc/rc.local if not yet
# initialised (detected by missing /data/weights/.initialized sentinel).
#
# Weights placement after guest format:
#   /data/weights/OLMo-7B-Instruct-Q4_K_M.gguf  (5.2 GiB approx)
#   /data/cluster/                               (cluster-totebox mounts)
#
# Transfer weights once the guest is running:
#   scp OLMo-7B-Instruct-Q4_K_M.gguf root@10.8.0.7:/data/weights/
#
# Usage:
#   bash scripts/provision-data-disk.sh
set -euo pipefail

BUILD_DIR="build"
DATA_DISK="${BUILD_DIR}/os-totebox-data.qcow2"
DATA_DISK_SIZE="8G"

command -v qemu-img >/dev/null || { echo "error: qemu-img not found on PATH"; exit 1; }

mkdir -p "${BUILD_DIR}"

if [ -f "${DATA_DISK}" ]; then
    echo "  data disk already exists: ${DATA_DISK}"
    echo "  delete it first to recreate: rm -f ${DATA_DISK}"
    exit 0
fi

echo "  creating sparse QCOW2 data disk (${DATA_DISK_SIZE})..."
qemu-img create -f qcow2 "${DATA_DISK}" "${DATA_DISK_SIZE}"

echo "  done: ${DATA_DISK}"
echo "  $(qemu-img info ${DATA_DISK} | grep 'virtual size')"
echo ""
echo "  the guest formats and mounts this disk on first boot."
echo "  transfer OLMo weights after the guest is running:"
echo "    scp OLMo-7B-Instruct-Q4_K_M.gguf root@10.8.0.7:/data/weights/"
