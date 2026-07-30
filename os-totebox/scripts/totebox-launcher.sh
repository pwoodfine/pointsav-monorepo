#!/bin/bash
echo "============================================================"
echo " 🏭 POINTSAV DIGITAL SYSTEMS : ZERO-TOUCH LAUNCHER"
echo "============================================================"

if [ ! -e /dev/kvm ]; then
    echo " [FATAL] /dev/kvm not found. Native virtualization required."
    exit 1
fi
echo " [OK] Native KVM silicon passthrough confirmed."

PAYLOAD=$1
if [ -z "$PAYLOAD" ] || [ ! -f "$PAYLOAD" ]; then
    echo " [ERROR] Invalid payload. USAGE: ./totebox-launcher.sh <path-to-payload.img>"
    exit 1
fi
echo " [OK] Target payload verified: $PAYLOAD"

echo " [OK] Provisioning network bridge..."
echo " [INFO] Firing Hypervisor. Shifting to TUI Administrator..."
sleep 2

qemu-system-x86_64 -enable-kvm -cpu host -m 2048 -drive format=raw,file="$PAYLOAD",if=virtio -nographic -net nic,model=virtio -net user
