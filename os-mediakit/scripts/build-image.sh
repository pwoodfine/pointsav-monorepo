#!/usr/bin/env bash
# build-image.sh — Build the os-mediakit Ubuntu 24.04 QCOW2 guest image.
#
# Produces: build/os-mediakit.qcow2 (self-contained Ubuntu 24.04, ~2 GiB)
# Requires: qemu-img, qemu-nbd (qemu-utils package), sudo, curl, e2fsck, resize2fs
#
# What this image contains:
#   - Ubuntu 24.04 minimal base
#   - app-mediakit-knowledge binary (WIKI_KNOWLEDGE_TOML path)
#   - Three wiki instance systemd units (ports 9090/9093/9095)
#   - Starter TOML configs in /etc/wiki/ — edit to customise
#   - Sample content directories at /var/lib/wiki/{documentation,projects,corporate}/
#   - 'wiki' system user (UID 990) owning the content directories
#
# Usage:
#   BINARY_PATH=/usr/local/bin/app-mediakit-knowledge \
#   bash scripts/build-image.sh
#
# Environment variables:
#   BINARY_PATH      Path to the app-mediakit-knowledge binary (required if not
#                    at the default /usr/local/bin/app-mediakit-knowledge)
#   IMAGE_SIZE       Target disk size (default: "2G")
#   UBUNTU_IMG_URL   Override Ubuntu minimal cloud image download URL
#   SKIP_DOWNLOAD    Set to "1" to reuse a cached cloud image in build/
#   NBD_DEV          Override NBD device (default: /dev/nbd0)

set -euo pipefail

UBUNTU_IMG_URL="${UBUNTU_IMG_URL:-https://cloud-images.ubuntu.com/minimal/releases/noble/release/ubuntu-24.04-minimal-cloudimg-amd64.img}"
BINARY_PATH="${BINARY_PATH:-/usr/local/bin/app-mediakit-knowledge}"
IMAGE_SIZE="${IMAGE_SIZE:-2G}"
NBD_DEV="${NBD_DEV:-/dev/nbd0}"

BUILD_DIR="build"
CACHED_BASE="${BUILD_DIR}/ubuntu-24.04-minimal-cloudimg-amd64.img"
WORK_IMAGE="${BUILD_DIR}/os-mediakit-work.qcow2"
IMAGE_QCOW2="${BUILD_DIR}/os-mediakit.qcow2"
MOUNT_POINT="${BUILD_DIR}/rootfs-mount"

WIKI_UID=990
WIKI_GID=990
WIKI_CONTENT_ROOT="/var/lib/wiki"

# ── 1. Preflight ─────────────────────────────────────────────────────────────
echo "[preflight]"
command -v qemu-img >/dev/null || { echo "error: qemu-img not found on PATH"; exit 1; }
command -v qemu-nbd >/dev/null || {
    echo "error: qemu-nbd not found. Install: sudo apt install qemu-utils"
    exit 1
}
command -v e2fsck   >/dev/null || { echo "error: e2fsck not found. Install: sudo apt install e2fsprogs"; exit 1; }
command -v resize2fs >/dev/null || { echo "error: resize2fs not found. Install: sudo apt install e2fsprogs"; exit 1; }
[ -f "${BINARY_PATH}" ] || {
    echo "error: binary not found at BINARY_PATH=${BINARY_PATH}"
    echo "  Set BINARY_PATH= to the path of the app-mediakit-knowledge binary."
    exit 1
}

mkdir -p "${BUILD_DIR}" "${MOUNT_POINT}"

# ── 2. Download Ubuntu 24.04 minimal cloud image (cached) ────────────────────
echo "[download]"
if [ "${SKIP_DOWNLOAD:-0}" = "1" ] && [ -f "${CACHED_BASE}" ]; then
    echo "  cached: $(basename "${CACHED_BASE}")"
else
    echo "  fetching: $(basename "${CACHED_BASE}")"
    curl -fSL --output "${CACHED_BASE}" "${UBUNTU_IMG_URL}"
fi

# ── 3. Create a standalone working copy sized to IMAGE_SIZE ──────────────────
echo "[prepare image — ${IMAGE_SIZE}]"
rm -f "${WORK_IMAGE}"
# Create a fresh QCOW2 from the cloud image at the target size.
# qemu-img create with -b makes a delta (copy-on-write) image; the final
# qemu-img convert step produces a self-contained QCOW2 without a backing file.
qemu-img create -f qcow2 -b "$(realpath "${CACHED_BASE}")" -F qcow2 \
    "${WORK_IMAGE}" "${IMAGE_SIZE}"
echo "  work image: ${WORK_IMAGE}"

# ── 4. Connect via qemu-nbd and locate the ext4 root partition ───────────────
echo "[mount via qemu-nbd]"
sudo modprobe nbd max_part=8 2>/dev/null || true
# Disconnect any prior connection to this device before re-using it.
sudo qemu-nbd --disconnect "${NBD_DEV}" 2>/dev/null || true
sleep 0.5

sudo qemu-nbd --connect="${NBD_DEV}" "${WORK_IMAGE}"
sleep 1

# Auto-detect the root (ext4) partition. Ubuntu minimal cloud images use GPT
# with a small ESP (p1) followed by the ext4 root (p2), but some older images
# have the root at p1. Walk in reverse order of likelihood.
ROOT_PART=""
for PART in "${NBD_DEV}p2" "${NBD_DEV}p1"; do
    if [ -b "${PART}" ]; then
        FS_TYPE=$(sudo blkid -s TYPE -o value "${PART}" 2>/dev/null || true)
        if [ "${FS_TYPE}" = "ext4" ]; then
            ROOT_PART="${PART}"
            break
        fi
    fi
done
if [ -z "${ROOT_PART}" ]; then
    echo "error: could not detect ext4 root partition on ${NBD_DEV}"
    sudo qemu-nbd --disconnect "${NBD_DEV}"
    exit 1
fi
echo "  root partition: ${ROOT_PART}"

# Grow the filesystem to fill the expanded QCOW2 layer before mounting.
sudo e2fsck -f -y "${ROOT_PART}" 2>/dev/null || true
sudo resize2fs "${ROOT_PART}"

sudo mount "${ROOT_PART}" "${MOUNT_POINT}"
echo "  mounted: ${ROOT_PART} → ${MOUNT_POINT}"

# Register a cleanup trap so the loop device is always released on exit.
cleanup() {
    echo "  [cleanup]"
    sudo umount "${MOUNT_POINT}" 2>/dev/null || true
    sudo qemu-nbd --disconnect "${NBD_DEV}" 2>/dev/null || true
}
trap cleanup EXIT

# ── 5a. Create 'wiki' system user and group ───────────────────────────────────
# Direct /etc/passwd manipulation — no chroot required, no package manager.
echo "[wiki system user (uid=${WIKI_UID})]"
if ! grep -q "^wiki:" "${MOUNT_POINT}/etc/passwd" 2>/dev/null; then
    echo "wiki:x:${WIKI_UID}:${WIKI_GID}:Wiki Service:${WIKI_CONTENT_ROOT}:/usr/sbin/nologin" \
        | sudo tee -a "${MOUNT_POINT}/etc/passwd" > /dev/null
fi
if ! grep -q "^wiki:" "${MOUNT_POINT}/etc/group" 2>/dev/null; then
    echo "wiki:x:${WIKI_GID}:" \
        | sudo tee -a "${MOUNT_POINT}/etc/group" > /dev/null
fi
if ! grep -q "^wiki:" "${MOUNT_POINT}/etc/shadow" 2>/dev/null; then
    printf 'wiki:!:19900:0:99999:7:::\n' \
        | sudo tee -a "${MOUNT_POINT}/etc/shadow" > /dev/null
fi

# ── 5b. Install binary ────────────────────────────────────────────────────────
echo "[install binary]"
sudo install -D -m 0755 "${BINARY_PATH}" \
    "${MOUNT_POINT}/usr/local/bin/app-mediakit-knowledge"
echo "  /usr/local/bin/app-mediakit-knowledge"

# ── 5c. Create content directories and sample article ────────────────────────
echo "[wiki content directories]"
for INSTANCE in documentation projects corporate; do
    sudo mkdir -p "${MOUNT_POINT}${WIKI_CONTENT_ROOT}/${INSTANCE}"
done

# Getting Started article — minimal valid frontmatter for the documentation instance.
sudo tee "${MOUNT_POINT}${WIKI_CONTENT_ROOT}/documentation/getting-started.md" > /dev/null << 'EOF'
---
title: Getting Started
description: Welcome to your PointSav Knowledge Wiki
date: 2026-01-01
quality: stub
---

# Getting Started

This is your PointSav Knowledge Wiki. Add Markdown files to this directory to create articles.

## Directory layout

Each wiki instance reads from its content directory. Articles are `.md` files with YAML
frontmatter. Subdirectories become categories. An `_index.md` file in each category directory
provides the category title and short description shown on the home page.

## Configuration

Edit the TOML file for each instance and restart the corresponding service:

```bash
# Edit documentation instance configuration
sudo nano /etc/wiki/documentation.toml

# Restart after editing
sudo systemctl restart wiki-documentation.service
```

## Healthcheck

```bash
curl http://127.0.0.1:9090/healthz   # documentation
curl http://127.0.0.1:9093/healthz   # projects
curl http://127.0.0.1:9095/healthz   # corporate
```
EOF

# Transfer ownership of all content dirs to the wiki user.
sudo chown -R "${WIKI_UID}:${WIKI_GID}" "${MOUNT_POINT}${WIKI_CONTENT_ROOT}"
sudo chmod 0750 "${MOUNT_POINT}${WIKI_CONTENT_ROOT}"

# State directories (search index, edit queue, sessions).
for INSTANCE in documentation projects corporate; do
    sudo mkdir -p "${MOUNT_POINT}/var/lib/wiki-state/${INSTANCE}"
done
sudo chown -R "${WIKI_UID}:${WIKI_GID}" "${MOUNT_POINT}/var/lib/wiki-state"
sudo chmod 0750 "${MOUNT_POINT}/var/lib/wiki-state"

# ── 5d. Install TOML configuration files ─────────────────────────────────────
echo "[toml configs → /etc/wiki/]"
sudo mkdir -p "${MOUNT_POINT}/etc/wiki"

sudo tee "${MOUNT_POINT}/etc/wiki/documentation.toml" > /dev/null << 'TOML'
# /etc/wiki/documentation.toml
# PointSav Knowledge Wiki — documentation instance
# Edit to customise; restart wiki-documentation.service to apply.

[site]
title         = "Knowledge Wiki"
brand         = "pointsav"
bind          = "0.0.0.0:9090"
state_dir     = "/var/lib/wiki-state/documentation"
instance      = "documentation"
canonical_url = "http://localhost:9090"
categories    = ["architecture", "systems", "services", "governance", "reference"]

[[mount]]
path          = "/var/lib/wiki/documentation"
role          = "primary"
blueprint_set = ["TOPIC", "GUIDE"]
TOML

sudo tee "${MOUNT_POINT}/etc/wiki/projects.toml" > /dev/null << 'TOML'
# /etc/wiki/projects.toml
# PointSav Knowledge Wiki — projects instance
# Edit to customise; restart wiki-projects.service to apply.

[site]
title         = "Projects Wiki"
brand         = "woodfine"
bind          = "0.0.0.0:9093"
state_dir     = "/var/lib/wiki-state/projects"
instance      = "projects"
canonical_url = "http://localhost:9093"
categories    = ["architecture", "urban", "reference"]

[[mount]]
path          = "/var/lib/wiki/projects"
role          = "primary"
blueprint_set = ["TOPIC"]
TOML

sudo tee "${MOUNT_POINT}/etc/wiki/corporate.toml" > /dev/null << 'TOML'
# /etc/wiki/corporate.toml
# PointSav Knowledge Wiki — corporate instance
# Edit to customise; restart wiki-corporate.service to apply.

[site]
title         = "Corporate Wiki"
brand         = "woodfine"
bind          = "0.0.0.0:9095"
state_dir     = "/var/lib/wiki-state/corporate"
instance      = "corporate"
canonical_url = "http://localhost:9095"
categories    = ["governance", "investment", "reference"]

[[mount]]
path          = "/var/lib/wiki/corporate"
role          = "primary"
blueprint_set = ["TOPIC"]
TOML

# Lock down config directory permissions.
sudo chmod 0750 "${MOUNT_POINT}/etc/wiki"
sudo chown "root:${WIKI_GID}" "${MOUNT_POINT}/etc/wiki"
sudo chmod 0640 "${MOUNT_POINT}/etc/wiki/"*.toml
sudo chown "root:${WIKI_GID}" "${MOUNT_POINT}/etc/wiki/"*.toml

# ── 5e. Install systemd service units ────────────────────────────────────────
echo "[systemd units]"
sudo mkdir -p "${MOUNT_POINT}/etc/systemd/system"

for INSTANCE in documentation projects corporate; do
    case "${INSTANCE}" in
        documentation) PORT=9090 ;;
        projects)      PORT=9093 ;;
        corporate)     PORT=9095 ;;
    esac

    sudo tee "${MOUNT_POINT}/etc/systemd/system/wiki-${INSTANCE}.service" > /dev/null << UNIT
[Unit]
Description=PointSav Knowledge Wiki — ${INSTANCE} (port ${PORT})
Documentation=https://software.pointsav.com/products/app-mediakit-knowledge
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
User=wiki
Group=wiki

WorkingDirectory=${WIKI_CONTENT_ROOT}

Environment="WIKI_KNOWLEDGE_TOML=/etc/wiki/${INSTANCE}.toml"
ExecStart=/usr/local/bin/app-mediakit-knowledge serve

Restart=on-failure
RestartSec=5s

NoNewPrivileges=true
ProtectSystem=strict
ProtectHome=true
PrivateTmp=true
ReadWritePaths=${WIKI_CONTENT_ROOT}
ReadWritePaths=/var/lib/wiki-state
ReadOnlyPaths=/etc/wiki

StandardOutput=journal
StandardError=journal

[Install]
WantedBy=multi-user.target
UNIT
done

# ── 5f. Enable all three wiki services ───────────────────────────────────────
echo "[enable services]"
sudo mkdir -p "${MOUNT_POINT}/etc/systemd/system/multi-user.target.wants"
for INSTANCE in documentation projects corporate; do
    sudo ln -sf \
        "/etc/systemd/system/wiki-${INSTANCE}.service" \
        "${MOUNT_POINT}/etc/systemd/system/multi-user.target.wants/wiki-${INSTANCE}.service"
done

# ── 5g. Hostname and /etc/hosts ──────────────────────────────────────────────
echo "[hostname: os-mediakit]"
echo "os-mediakit" | sudo tee "${MOUNT_POINT}/etc/hostname" > /dev/null
sudo tee "${MOUNT_POINT}/etc/hosts" > /dev/null << 'HOSTS'
127.0.0.1   localhost
127.0.1.1   os-mediakit
::1         localhost ip6-localhost ip6-loopback
HOSTS

# ── 5h. Disable cloud-init (pre-configured image, no metadata server needed) ──
echo "[disable cloud-init]"
sudo touch "${MOUNT_POINT}/etc/cloud/cloud-init.disabled"

# ── 6. Unmount and disconnect ─────────────────────────────────────────────────
echo "[unmount]"
sudo sync
sudo umount "${MOUNT_POINT}"
sudo qemu-nbd --disconnect "${NBD_DEV}"
trap - EXIT
sleep 0.5
rmdir "${MOUNT_POINT}" 2>/dev/null || true

# ── 7. Convert working image to final standalone QCOW2 ───────────────────────
# -c enables QCOW2 compression; produces a ~400-600 MB file vs ~2 GB uncompressed.
echo "[convert → ${IMAGE_QCOW2}]"
qemu-img convert -f qcow2 -O qcow2 -c "${WORK_IMAGE}" "${IMAGE_QCOW2}"
rm -f "${WORK_IMAGE}"

FINAL_SIZE=$(du -sh "${IMAGE_QCOW2}" | cut -f1)
echo ""
echo "  done: ${IMAGE_QCOW2}  (${FINAL_SIZE})"
echo ""
echo "  QEMU launch (ports 9090/9093/9095 forwarded to host):"
echo "    qemu-system-x86_64 \\"
echo "      -m 1G \\"
echo "      -drive file=${IMAGE_QCOW2},format=qcow2 \\"
echo "      -net user,hostfwd=tcp::9090-:9090,hostfwd=tcp::9093-:9093,hostfwd=tcp::9095-:9095 \\"
echo "      -nographic"
echo ""
echo "  Verify (once booted):"
echo "    curl http://127.0.0.1:9090/healthz   # documentation → ok"
echo "    curl http://127.0.0.1:9093/healthz   # projects      → ok"
echo "    curl http://127.0.0.1:9095/healthz   # corporate     → ok"
echo ""
echo "  SHA-256:"
sha256sum "${IMAGE_QCOW2}"
