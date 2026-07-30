#!/usr/bin/env bash
#
# bootstrap.sh — install app-orchestration-command as a systemd unit on the
# workspace VM. Idempotent; safe to re-run for binary updates or upgrades.
#
# Run as:
#   sudo /srv/foundry/infrastructure/local-orchestration-command/bootstrap.sh
#
# BINARY ACQUISITION (in priority order):
#
#   1. BINARY_URL env var — curl-download from software.pointsav.com (production):
#        export BINARY_URL="https://software.pointsav.com/download/soft-orchestration-command/linux-amd64/v0.0.1/orchestration-command-server"
#        sudo -E bootstrap.sh
#
#   2. BINARY_SRC env var — local path (dev builds, CI):
#        export BINARY_SRC="/srv/foundry/cargo-target/orchestration-command/release/orchestration-command-server"
#        sudo -E bootstrap.sh
#
#   3. Default URL — used when neither env var is set. Update version tag when upgrading.
#
# BETA NOTE (2026-06-29): Binary is in BETA; software.pointsav.com listing is free
# (no payment gate). URL is publicly accessible during BETA. project-software will
# re-enable pricing when BETA ends.
#
# To set the license token after bootstrapping:
#   sudo systemctl edit local-orchestration-command
#   Add under [Service]:
#     Environment="COMMAND_LICENSE_TOKEN=<token>"
#     Environment="COMMAND_LICENSE_PUBKEY_HEX=<hex>"
#   Then: sudo systemctl restart local-orchestration-command
#
set -euo pipefail

INFRA_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BINARY_DST="/usr/local/bin/orchestration-command-server"
SERVICE_NAME="local-orchestration-command"
SERVICE_FILE="${INFRA_DIR}/local-orchestration-command.service"
SERVICE_DEST="/etc/systemd/system/local-orchestration-command.service"
SVC_USER="local-orchestration-command"
SVC_HOME="/var/lib/local-orchestration-command"
DEPLOYMENT_DIR="/srv/foundry/deployments/gateway-orchestration-command-1"

# Default download URL — project-software will confirm the canonical URL pattern.
# Override with BINARY_URL env var for a specific version or mirror.
DEFAULT_BINARY_URL="https://software.pointsav.com/download/soft-orchestration-command/linux-amd64/v0.0.1/orchestration-command-server"

# --- 1. Sanity ----------------------------------------------------------

if [[ "${EUID}" -ne 0 ]]; then
    echo "Error: bootstrap.sh must run as root (use sudo -E to preserve env vars)." >&2
    exit 1
fi

if [[ ! -d "${DEPLOYMENT_DIR}" ]]; then
    echo "Error: deployment instance not found at ${DEPLOYMENT_DIR}" >&2
    exit 1
fi

# --- 2. Acquire binary --------------------------------------------------

BINARY_SRC="${BINARY_SRC:-}"

if [[ -n "${BINARY_URL:-}" ]]; then
    echo "Downloading binary from ${BINARY_URL}..."
    curl -fsSL --progress-bar "${BINARY_URL}" -o "${BINARY_DST}.tmp"
    install -o root -g root -m 0755 "${BINARY_DST}.tmp" "${BINARY_DST}"
    rm -f "${BINARY_DST}.tmp"
    echo "  Downloaded → ${BINARY_DST}"
elif [[ -n "${BINARY_SRC}" && -f "${BINARY_SRC}" ]]; then
    echo "Installing binary from local path ${BINARY_SRC}..."
    install -o root -g root -m 0755 "${BINARY_SRC}" "${BINARY_DST}"
    echo "  Installed → ${BINARY_DST}"
else
    echo "Downloading binary from default URL (BETA — free download)..."
    echo "  ${DEFAULT_BINARY_URL}"
    if ! curl -fsSL --progress-bar "${DEFAULT_BINARY_URL}" -o "${BINARY_DST}.tmp" 2>/dev/null; then
        echo "" >&2
        echo "Error: download failed. Options:" >&2
        echo "  A) Supply a local dev build: export BINARY_SRC=/path/to/orchestration-command-server" >&2
        echo "     Build with:" >&2
        echo "       cd /srv/foundry/clones/project-orchestration/pointsav-monorepo/app-orchestration-command" >&2
        echo "       CARGO_TARGET_DIR=/srv/foundry/cargo-target/orchestration-command cargo build --release -p orchestration-command-server" >&2
        echo "  B) Supply an explicit download URL: export BINARY_URL=<url>" >&2
        exit 1
    fi
    install -o root -g root -m 0755 "${BINARY_DST}.tmp" "${BINARY_DST}"
    rm -f "${BINARY_DST}.tmp"
    echo "  Downloaded → ${BINARY_DST}"
fi

# --- 3. Service user ----------------------------------------------------

if ! id -u "${SVC_USER}" >/dev/null 2>&1; then
    echo "Creating service user ${SVC_USER}..."
    useradd --system --home-dir "${SVC_HOME}" --create-home \
            --shell /usr/sbin/nologin "${SVC_USER}"
fi

mkdir -p "${SVC_HOME}"
chown -R "${SVC_USER}:${SVC_USER}" "${SVC_HOME}"
chmod 0750 "${SVC_HOME}"

# Give service user read access to workspace paths via foundry group.
usermod -a -G foundry "${SVC_USER}" 2>/dev/null || true

# --- 4. Install systemd unit --------------------------------------------

echo "Installing unit ${SERVICE_FILE} → ${SERVICE_DEST}..."
install -o root -g root -m 0644 "${SERVICE_FILE}" "${SERVICE_DEST}"
systemctl daemon-reload

# --- 5. Enable and start ------------------------------------------------

echo "Enabling and starting ${SERVICE_NAME}.service..."
systemctl enable --now "${SERVICE_NAME}.service"
sleep 2

if systemctl is-active --quiet "${SERVICE_NAME}.service"; then
    echo "OK — ${SERVICE_NAME}.service active."
else
    echo "Warning: ${SERVICE_NAME}.service did not start cleanly. Check:" >&2
    echo "  journalctl -u ${SERVICE_NAME}.service -n 30" >&2
    exit 1
fi

# --- 6. Smoke tests -----------------------------------------------------

echo ""
echo "Smoke test: GET /healthz"
curl -sS -m 5 http://127.0.0.1:8020/healthz && echo ""

echo ""
echo "Smoke test: GET /readyz"
curl -sS -m 5 http://127.0.0.1:8020/readyz | python3 -m json.tool 2>/dev/null || \
    curl -sS -m 5 http://127.0.0.1:8020/readyz
echo ""

# --- 7. Next steps ------------------------------------------------------

cat <<'EOF'

bootstrap.sh complete. Next steps:

1. Set license token (enables invite/pair endpoints):
     sudo systemctl edit local-orchestration-command
     # add under [Service]:
     #   Environment="COMMAND_LICENSE_TOKEN=<token>"
     #   Environment="COMMAND_LICENSE_PUBKEY_HEX=<hex>"
     sudo systemctl restart local-orchestration-command

2. Verify /readyz shows license: "valid" and archives loaded:
     curl -s http://127.0.0.1:8020/readyz | python3 -m json.tool

3. Test archive listing:
     curl -s http://127.0.0.1:8020/v1/archives | python3 -m json.tool

4. Update cluster manifest tetrad.deployment status to active once smoke passes.

BETA reminder: pricing is disabled on software.pointsav.com for this product.
When BETA ends, project-software will re-enable the $1/$19 pricing gate.
EOF
