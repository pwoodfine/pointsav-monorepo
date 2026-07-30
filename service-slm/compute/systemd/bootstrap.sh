#!/usr/bin/env bash
# Bootstrap installer for service-slm Doorman as systemd unit
# Idempotent: safe to run multiple times. Suitable for Master scope VM sysadmin.
set -euo pipefail

CLUSTER_ROOT="${CLUSTER_ROOT:=/srv/foundry/clones/project-slm}"
SERVICE_SLM_DIR="${CLUSTER_ROOT}/service-slm"
UNIT_SOURCE="${SERVICE_SLM_DIR}/compute/systemd/slm-doorman.service"
BINARY_SOURCE="${SERVICE_SLM_DIR}/target/release/slm-doorman-server"
BINARY_DEST="/usr/local/bin/slm-doorman-server"
UNIT_DEST="/etc/systemd/system/slm-doorman.service"
USER="slm-doorman"
GROUP="slm-doorman"
STATE_DIR="/var/lib/slm-doorman"
AUDIT_DIR="${STATE_DIR}/audit"

echo "[DOORMAN] Bootstrap starting..."

# 1. Build release binary (idempotent: cargo checks if rebuild needed)
echo "[DOORMAN] Building release binary..."
cd "${SERVICE_SLM_DIR}"
cargo build --release -p slm-doorman-server

if [ ! -f "${BINARY_SOURCE}" ]; then
  echo "[ERROR] Binary not found at ${BINARY_SOURCE}" >&2
  exit 1
fi

# 2. Create system user and group (idempotent)
if ! id "${USER}" &>/dev/null; then
  echo "[DOORMAN] Creating system user ${USER}..."
  useradd --system --shell /sbin/nologin --home-dir "${STATE_DIR}" "${USER}" || true
else
  echo "[DOORMAN] User ${USER} already exists"
fi

# 3. Create state directory with proper ownership (idempotent)
echo "[DOORMAN] Creating state directory ${STATE_DIR}..."
mkdir -p "${STATE_DIR}" "${AUDIT_DIR}"
chown "${USER}:${GROUP}" "${STATE_DIR}" "${AUDIT_DIR}"
chmod 0750 "${STATE_DIR}" "${AUDIT_DIR}"

# 4. Copy binary to /usr/local/bin (overwrites old version)
echo "[DOORMAN] Installing binary to ${BINARY_DEST}..."
cp "${BINARY_SOURCE}" "${BINARY_DEST}"
chmod 0755 "${BINARY_DEST}"

# 5. Install systemd unit (overwrites old version)
echo "[DOORMAN] Installing systemd unit to ${UNIT_DEST}..."
cp "${UNIT_SOURCE}" "${UNIT_DEST}"
chmod 0644 "${UNIT_DEST}"

# 6. Reload systemd and enable unit
echo "[DOORMAN] Reloading systemd daemon..."
systemctl daemon-reload

echo "[DOORMAN] Enabling slm-doorman.service to start on boot..."
systemctl enable slm-doorman.service

echo "[DOORMAN] Bootstrap complete."

# 7. Install nightly-run systemd units (timer + service)
NIGHTLY_TIMER_SOURCE="${SERVICE_SLM_DIR}/compute/systemd/nightly-run.timer"
NIGHTLY_SERVICE_SOURCE="${SERVICE_SLM_DIR}/compute/systemd/nightly-run.service"
echo "[NIGHTLY] Installing nightly-run.timer + nightly-run.service..."
install -m 644 "${NIGHTLY_TIMER_SOURCE}"   /etc/systemd/system/nightly-run.timer
install -m 644 "${NIGHTLY_SERVICE_SOURCE}" /etc/systemd/system/nightly-run.service
systemctl daemon-reload
echo "[NIGHTLY] Units installed. Enable with: systemctl enable --now nightly-run.timer"

echo ""
echo "Next steps:"
echo "  1. Verify the unit installed: systemctl cat slm-doorman.service"
echo "  2. Start the service: systemctl start slm-doorman.service"
echo "  3. Check logs: journalctl -u slm-doorman -f"
echo "  4. Test endpoint: curl http://127.0.0.1:9080/healthz"
echo "  5. Enable nightly timer: systemctl enable --now nightly-run.timer"
echo "  6. Verify: systemctl list-timers nightly-run.timer"
echo ""
