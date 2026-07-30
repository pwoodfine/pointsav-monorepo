#!/bin/bash
set -e

echo "===================================================="
echo " 💿 FORGING PointSav Network Admin ISO (Node 3)"
echo "===================================================="

# Define absolute paths
MONOREPO_ROOT="$HOME/Foundry/factory-pointsav/pointsav-monorepo"
ADMIN_DIR="$MONOREPO_ROOT/os-network-admin"
SECURITY_BUILD="$MONOREPO_ROOT/system-security/build"
ISO_ROOT="$ADMIN_DIR/build_iso"
OUTPUT_ISO="$ADMIN_DIR/pointsav-os-network-admin.iso"

# Verify Artifacts
if [ ! -f "$SECURITY_BUILD/final_image.elf" ]; then
    echo "[ERROR] final_image.elf not found in system-security."
    exit 1
fi

KERNEL_SOURCE="$MONOREPO_ROOT/system-security/sel4_32.elf"
if [ ! -f "$KERNEL_SOURCE" ]; then
    echo "[ERROR] seL4 microkernel not found in system-security."
    exit 1
fi

echo "[1/3] Staging Payloads for Command Authority..."
cp "$KERNEL_SOURCE" "$ISO_ROOT/boot/kernel.elf"
cp "$SECURITY_BUILD/final_image.elf" "$ISO_ROOT/boot/final_image.elf"

echo "[2/3] Synthesizing Admin ISO..."
grub-mkrescue -o "$OUTPUT_ISO" "$ISO_ROOT" 2>/dev/null

echo "[3/3] Verification Status: SUCCESS"
echo "Artifact generated: $OUTPUT_ISO"
echo "===================================================="
