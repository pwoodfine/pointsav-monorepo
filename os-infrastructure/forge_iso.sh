#!/bin/bash
set -e

echo "===================================================="
echo " 💿 FORGING PointSav Infrastructure ISO (Node 1)"
echo "===================================================="

# Define absolute paths based on the Factory Silo
MONOREPO_ROOT="/srv/foundry/vendor/pointsav-monorepo"
INFRA_DIR="$MONOREPO_ROOT/os-infrastructure"
SECURITY_BUILD="$MONOREPO_ROOT/system-security/build"
ISO_ROOT="$INFRA_DIR/build_iso"
OUTPUT_ISO="$INFRA_DIR/pointsav-os-infrastructure.iso"

# 1. Verify Artifacts Exist
if [ ! -f "$SECURITY_BUILD/final_image.elf" ]; then
    echo "[ERROR] final_image.elf not found. Run 'make' in system-security first."
    exit 1
fi

# 2. Locate the seL4 microkernel (using the QEMU test kernel for now)
KERNEL_SOURCE="$MONOREPO_ROOT/system-security/sel4_32.elf"
if [ ! -f "$KERNEL_SOURCE" ]; then
    echo "[ERROR] seL4 microkernel ($KERNEL_SOURCE) not found."
    exit 1
fi

echo "[1/3] Staging Payloads..."
cp "$KERNEL_SOURCE" "$ISO_ROOT/boot/kernel.elf"
cp "$SECURITY_BUILD/final_image.elf" "$ISO_ROOT/boot/final_image.elf"

echo "[2/3] Synthesizing ISO..."
# grub-mkrescue wraps the directory into a bootable ISO using xorriso
grub-mkrescue -o "$OUTPUT_ISO" "$ISO_ROOT" 2>/dev/null

echo "[3/3] Verification Status: SUCCESS"
echo "Artifact generated: $OUTPUT_ISO"
echo "===================================================="
