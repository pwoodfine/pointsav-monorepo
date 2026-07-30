#!/bin/bash
# PointSav Digital Systems | Tier-2 Cloud Synthesis Trigger (Template)
set -euo pipefail

REMOTE_TARGET="<REMOTE_IP_ADDRESS>"
REMOTE_USER="<OPERATOR_USERNAME>"

echo "[SYSTEM] Initiating Manual Tier-2 Telemetry Synthesis..."
ssh "${REMOTE_USER}@${REMOTE_TARGET}" "sudo bash -c '
rm -f /opt/deployments/pointsav-fleet-deployment/media-marketing-landing/app-mediakit-telemetry/outbox/*.md 2>/dev/null || true
cd /opt/deployments/pointsav-fleet-deployment/media-marketing-landing
FLEET_ID=POINTSAV python3 /usr/local/bin/omni-matrix-engine.py
'"
echo "[SUCCESS] Markdown reports mathematically forged."
