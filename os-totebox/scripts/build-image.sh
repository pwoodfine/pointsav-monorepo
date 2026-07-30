#!/usr/bin/env bash
# build-image.sh — Build the os-totebox NetBSD 10.1 guest image.
#
# Produces: build/os-totebox.qcow2 (root filesystem, 4 GiB FFS2)
# Requires: NetBSD cross tools under TOOLS_DIR (build from NetBSD src with
#           ./build.sh -U -T /path/to/tools tools) and qemu-img on PATH.
#
# Does NOT mount UFS2 from Linux — all filesystem writes go through nbmakefs.
# The resulting image is bootable on QEMU KVM (x86_64).
#
# Usage:
#   TOOLS_DIR=/path/to/netbsd-tools \
#   BINARIES_DIR=/path/to/cross-compiled-binaries \
#   bash scripts/build-image.sh
set -euo pipefail

NETBSD_VER="10.1"
ARCH="amd64"
SETS_URL="https://cdn.netbsd.org/pub/NetBSD/NetBSD-${NETBSD_VER}/${ARCH}/binary/sets"
TOOLS_DIR="${TOOLS_DIR:-build/netbsd-tools}"
# CARGO_TARGET_DIR is workspace-wide redirect (e.g. /srv/foundry/cargo-target/mathew).
# Fall back to local target/ if not set.
_CARGO_RELEASE="${CARGO_TARGET_DIR:-../../target}/x86_64-unknown-netbsd/release"
BINARIES_DIR="${BINARIES_DIR:-${_CARGO_RELEASE}}"
BUILD_DIR="build"
OVERLAY="${BUILD_DIR}/overlay"
IMAGE_RAW="${BUILD_DIR}/os-totebox.img"
IMAGE_QCOW2="${BUILD_DIR}/os-totebox.qcow2"
# Ubuntu makefs 20190105-3 has a 32-bit off_t overflow bug; fails on images > 2GB.
# 1GB is sufficient for the base server rootfs (253 MB used after pruning + runtime overhead).
# Data (audit JSONL, graph DB, etc.) lives on the separate data disk (provision-data-disk.sh).
IMAGE_SIZE="1g"

# ── 1. Preflight ─────────────────────────────────────────────────────────────
command -v qemu-img >/dev/null || { echo "error: qemu-img not found on PATH"; exit 1; }

# makefs: prefer nbmakefs (NetBSD cross tools) for reproducibility.
# Fall back to the Ubuntu/Debian makefs package (apt install makefs).
NBINSTALLBOOT="${TOOLS_DIR}/bin/nbinstallboot"
if [ -x "${TOOLS_DIR}/bin/nbmakefs" ]; then
    NBMAKEFS="${TOOLS_DIR}/bin/nbmakefs"
elif command -v makefs >/dev/null 2>&1; then
    NBMAKEFS="makefs"
    echo "  note: using host makefs (nbmakefs not found in TOOLS_DIR=${TOOLS_DIR})"
else
    echo "error: makefs not found. Install via: apt install makefs"
    echo "  Or build NetBSD cross tools: cd netbsd-src && ./build.sh -U -T ${TOOLS_DIR} tools"
    exit 1
fi

# ── 2. Download official NetBSD binary sets ───────────────────────────────────
# NetBSD CDN uses .tar.xz format (not .tgz) as of NetBSD 10.x.
mkdir -p "${BUILD_DIR}/sets"
for SET in base etc kern-GENERIC; do
    DEST="${BUILD_DIR}/sets/${SET}.tar.xz"
    [ -f "${DEST}" ] && { echo "  cached: ${SET}.tar.xz"; continue; }
    echo "  fetching: ${SET}.tar.xz"
    curl -fSL --output "${DEST}" "${SETS_URL}/${SET}.tar.xz"
done

# ── 3. Assemble rootfs overlay ───────────────────────────────────────────────
# Use sudo rm in case a prior build left root-owned files (SSH keys, etc.).
sudo rm -rf "${OVERLAY}"
mkdir -p "${OVERLAY}"
echo "  extracting base set..."
tar -xJf "${BUILD_DIR}/sets/base.tar.xz"         -C "${OVERLAY}"
echo "  extracting etc set..."
tar -xJf "${BUILD_DIR}/sets/etc.tar.xz"          -C "${OVERLAY}"
echo "  extracting kernel..."
tar -xJf "${BUILD_DIR}/sets/kern-GENERIC.tar.xz" -C "${OVERLAY}"

# ── 4. Install our binaries (cross-compiled for x86_64-unknown-netbsd) ───────
echo "  installing binaries..."
install -D -m 0755 "${BINARIES_DIR}/system-ledger-server" \
    "${OVERLAY}/usr/bin/system-ledger-server"
# slm-doorman-server and service-content are built separately; copy from
# BINARIES_DIR if present, else skip with a warning.
for BIN in slm-doorman-server service-content; do
    SRC="${BINARIES_DIR}/${BIN}"
    if [ -f "${SRC}" ]; then
        install -D -m 0755 "${SRC}" "${OVERLAY}/usr/bin/${BIN}"
    else
        echo "  warning: ${BIN} not found in BINARIES_DIR — skipping"
    fi
done
# llama-server from pkgsrc — must be pre-built on a NetBSD host or via
# NetBSD cross-pkgsrc. Expect it at BINARIES_DIR/llama-server.
for BIN in llama-server; do
    SRC="${BINARIES_DIR}/${BIN}"
    if [ -f "${SRC}" ]; then
        install -D -m 0755 "${SRC}" "${OVERLAY}/usr/bin/${BIN}"
    else
        echo "  warning: ${BIN} not found — inference will be unavailable"
    fi
done

# ── 5. Install rc.d scripts ──────────────────────────────────────────────────
echo "  installing rc.d scripts..."
install -D -m 0755 scripts/rc.d/system_ledger  "${OVERLAY}/etc/rc.d/system_ledger"
install -D -m 0755 scripts/rc.d/doorman         "${OVERLAY}/etc/rc.d/doorman"
install -D -m 0755 scripts/rc.d/service_content "${OVERLAY}/etc/rc.d/service_content"
install -D -m 0755 scripts/rc.d/llama_server    "${OVERLAY}/etc/rc.d/llama_server"

# ── 6. Configure rc.conf ─────────────────────────────────────────────────────
cat >> "${OVERLAY}/etc/rc.conf" << 'EOF'
rc_configured=YES
# os-totebox services
sshd=YES
system_ledger=YES
doorman=YES
service_content=NO  # binary not in base image; deployment-time injection
llama_server=NO     # inference weights are deployment-time injection; not in base image
EOF

# ── 6b. Generate /etc/fstab ──────────────────────────────────────────────────
# Root FFS2 is on dk1 (NetBSD GPT partition device naming: dk0=ESP, dk1=FFS2 root).
# Listing it with pass=1 allows fsck -p to verify the FS; the kernel already mounted it.
# Without this file, NetBSD rc drops to a rescue shell with "cannot open /etc/fstab".
cat > "${OVERLAY}/etc/fstab" << 'FSTAB_EOF'
# /etc/fstab — os-totebox base image
# Root FFS2 is mounted by kernel from dk1 (GPT partition 2 of wd0 under QEMU IDE).
/dev/dk1        /       ffs     rw              1 1
FSTAB_EOF

# ── 7. Configure network interfaces ──────────────────────────────────────────
# wm0: QEMU e1000 maps to the NetBSD wm(4) driver (Intel Gigabit Ethernet).
# Static IP for QEMU SLIRP testing (10.0.2.15, GW 10.0.2.2).
# In hardware deployment, replace with dhcpcd or site-specific static config.
# The "!command" prefix runs the command after ifconfig completes.
cat > "${OVERLAY}/etc/ifconfig.wm0" << 'NET_EOF'
inet 10.0.2.15 netmask 255.255.255.0
!route add default 10.0.2.2
NET_EOF

# wg0: WireGuard tunnel. Actual keys are injected at deployment time by
# project-infrastructure. This file is a template; project-infrastructure
# replaces <PPN_PRIVATE_KEY> and <GCP_PUBLIC_KEY> via cloud-init or a
# provisioning script.
cat > "${OVERLAY}/etc/ifconfig.wg0" << 'EOF'
create
!wgconfig wg0 set private-key /etc/wireguard/private.key
!wgconfig wg0 add peer <GCP_PUBLIC_KEY> \
    --allowed-ips 10.8.0.0/24 \
    --endpoint <GCP_ENDPOINT>:51820
inet 10.8.0.7/24
EOF
chmod 0600 "${OVERLAY}/etc/ifconfig.wg0"
mkdir -p "${OVERLAY}/etc/wireguard"
chmod 0700 "${OVERLAY}/etc/wireguard"

# ── 7b. Pre-generate SSH host keys ───────────────────────────────────────────
# sshd skips host keys not owned by uid=0. makefs captures the source uid/gid,
# so the keys must be owned by root on the host BEFORE makefs runs.
# This step requires sudo for the chown. To skip chown (e.g. when running this
# script under sudo already), set SKIP_SSH_CHOWN=1.
echo "  generating SSH host keys..."
mkdir -p "${OVERLAY}/etc/ssh"
for TYPE in ed25519 ecdsa rsa; do
    KEY="${OVERLAY}/etc/ssh/ssh_host_${TYPE}_key"
    [ -f "${KEY}" ] && continue  # keep existing key on re-runs
    ssh-keygen -q -t "${TYPE}" -f "${KEY}" -N ""
done
if [ "${SKIP_SSH_CHOWN:-0}" != "1" ]; then
    sudo chown 0:0 \
        "${OVERLAY}/etc/ssh/ssh_host_ed25519_key" \
        "${OVERLAY}/etc/ssh/ssh_host_ed25519_key.pub" \
        "${OVERLAY}/etc/ssh/ssh_host_ecdsa_key" \
        "${OVERLAY}/etc/ssh/ssh_host_ecdsa_key.pub" \
        "${OVERLAY}/etc/ssh/ssh_host_rsa_key" \
        "${OVERLAY}/etc/ssh/ssh_host_rsa_key.pub" \
        "${OVERLAY}/var/chroot/sshd"
fi

# Disable reverse-DNS lookup on connecting IPs. Under SLIRP the host appears
# as 10.0.2.2 which has no PTR record; sshd hangs at banner exchange waiting
# for a DNS timeout before sending the SSH version string.
printf '\nUseDNS no\n' >> "${OVERLAY}/etc/ssh/sshd_config"

# ── 7c. Root authorized_keys for smoke testing ───────────────────────────────
# Inject a test pubkey so SSH smoke tests can connect as root without a password.
# TEST_PUBKEY_FILE defaults to /tmp/totebox-ssh-test.pub; set to "" to skip (production).
TEST_PUBKEY_FILE="${TEST_PUBKEY_FILE:-/tmp/totebox-ssh-test.pub}"
if [ -n "${TEST_PUBKEY_FILE}" ] && [ -f "${TEST_PUBKEY_FILE}" ]; then
    echo "  injecting smoke-test SSH pubkey into root authorized_keys..."
    sudo mkdir -p "${OVERLAY}/root/.ssh"
    sudo chmod 0700 "${OVERLAY}/root/.ssh"
    cat "${TEST_PUBKEY_FILE}" | sudo tee "${OVERLAY}/root/.ssh/authorized_keys" > /dev/null
    sudo chmod 0600 "${OVERLAY}/root/.ssh/authorized_keys"
    sudo chown -R 0:0 "${OVERLAY}/root/.ssh"
    # Allow root login via pubkey; append to override any commented-out default.
    printf '\nPermitRootLogin yes\nPasswordAuthentication no\n' \
        >> "${OVERLAY}/etc/ssh/sshd_config"
fi

# TCP wrappers: NetBSD OpenSSH is compiled with libwrap. The default
# /etc/hosts.allow is absent; SLIRP forwarded connections arrive from 10.0.2.2
# (not LOCAL). Permit sshd from any source so the smoke-test SSH works.
echo "sshd: ALL" >> "${OVERLAY}/etc/hosts.allow"

# ── 8. Strip non-server content (server image only needs runtime, not docs) ───
echo "  pruning non-server content..."
# Strip all of /usr/share/ except zoneinfo (needed for localtime(3)) and
# tabset/nls (referenced by some NetBSD rc scripts).
# This saves ~60 MB and avoids a known Ubuntu makefs 20190105-3 EINVAL bug
# that triggers on files > ~90 KB during FFS2 image population.
# Pruning runs BEFORE Veriexec manifest so no stale paths are fingerprinted.
if [ -d "${OVERLAY}/usr/share" ]; then
    find "${OVERLAY}/usr/share" -mindepth 1 -maxdepth 1 \
        ! -name "zoneinfo" ! -name "tabset" ! -name "nls" \
        -exec rm -rf {} \;
fi
rm -rf "${OVERLAY}/usr/games" "${OVERLAY}/usr/share/games"

# ── 8b. Veriexec manifest (OS-level binary signing) ──────────────────────────
# Fingerprint every executable in the overlay: custom binaries + system binaries.
# strict=1 (IDS mode) denies exec of any unlisted binary at runtime.
# Run after pruning so removed paths are never included in the policy.
echo "  generating Veriexec manifest (all executables)..."
VERIEXEC_MANIFEST="${OVERLAY}/etc/signatures"
: > "${VERIEXEC_MANIFEST}"
# Disable pipefail for the find pipeline: find exits 1 when it encounters
# unreadable directories (root/.ssh, var/spool/ftp/hidden). Those dirs contain
# no executables; the 2>/dev/null suppresses the messages but not the exit code.
set +o pipefail
find "${OVERLAY}" -type f -perm /111 2>/dev/null | sort | while read -r BIN_PATH; do
    REL_PATH="${BIN_PATH#${OVERLAY}}"
    DIGEST=$(sha256sum "${BIN_PATH}" | awk '{print $1}')
    printf '%s %s VERIEXEC_DIRECT\n' "${REL_PATH}" "${DIGEST}" \
        >> "${VERIEXEC_MANIFEST}"
done
set -o pipefail
# Enable Veriexec at boot; strict=1 raises to IDS mode (deny unlisted exec).
echo "veriexec=YES" >> "${OVERLAY}/etc/security.conf"
echo "kern.veriexec.strict=1" >> "${OVERLAY}/etc/sysctl.conf"

# ── 9. Build FFS2 disk image using NetBSD cross-makefs ───────────────────────
# Set correct ownership before image capture. tar extraction runs as mathew
# (uid=1001) so extracted files are owned by uid=1001. PAM, login(1), and
# sshd enforce that /etc/login.conf, /etc/pam.d/*, and other system files must
# be owned by root (uid=0); boot fails with "insecure ownership" otherwise.
# This must run AFTER all file writes (rc.conf, fstab, Veriexec manifest, etc.)
# and BEFORE makefs so the image captures correct uid=0 ownership.
echo "  fixing overlay ownership (chown -R 0:0)..."
sudo chown -R 0:0 "${OVERLAY}"
# Fix any execute-only directories created by NetBSD tarball extraction —
# makefs cannot opendir() them.
sudo find "${OVERLAY}" -type d ! -readable -exec chmod u+rx {} \;
echo "  building FFS2 image (${IMAGE_SIZE})..."
sudo "${NBMAKEFS}" -t ffs -s "${IMAGE_SIZE}" -o version=2 "${IMAGE_RAW}" "${OVERLAY}"
sudo chown "$(id -un):$(stat -c '%G' "${OVERLAY}")" "${IMAGE_RAW}"

# Install bootloader (NetBSD BIOS GPT/MBR boot).
if [ -f "${OVERLAY}/usr/mdec/biosboot" ] && [ -x "${NBINSTALLBOOT}" ]; then
    "${NBINSTALLBOOT}" "${IMAGE_RAW}" \
        "${OVERLAY}/usr/mdec/biosboot" "/usr/mdec/boot"
else
    echo "  warning: biosboot not found — image may not be directly bootable"
    echo "    use -kernel vmlinuz or direct ELF load in QEMU instead"
fi

# ── 10. Convert to QCOW2 ─────────────────────────────────────────────────────
echo "  converting to QCOW2..."
qemu-img convert -f raw -O qcow2 "${IMAGE_RAW}" "${IMAGE_QCOW2}"
rm -f "${IMAGE_RAW}"

echo ""
echo "  done: ${IMAGE_QCOW2}"
echo "  $(du -sh ${IMAGE_QCOW2} | cut -f1)"
echo ""
echo "  launch with:"
echo "    qemu-system-x86_64 -enable-kvm -cpu host -m 8192 \\"
echo "      -drive file=${IMAGE_QCOW2},format=qcow2,if=virtio \\"
echo "      -drive file=build/os-totebox-data.qcow2,format=qcow2,if=virtio \\"
echo "      -netdev tap,id=net0,ifname=tap-intelligence \\"
echo "      -device virtio-net-pci,netdev=net0 \\"
echo "      -nographic -serial mon:stdio"
