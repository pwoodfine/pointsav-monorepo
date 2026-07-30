#!/bin/bash
# PointSav Digital Systems | Generic Synthesis Template
set -euo pipefail

REMOTE_TARGET="<TARGET_IP>"
REMOTE_USER="<TARGET_USER>"
FLEET_DEPLOYMENT_PATH="<PATH_TO_DEPLOYMENT>"

echo "[SYSTEM] Igniting Generic Python Engine..."
ssh "${REMOTE_USER}@${REMOTE_TARGET}" "sudo bash -c '
rm -f ${FLEET_DEPLOYMENT_PATH}/app-mediakit-telemetry/outbox/*.md 2>/dev/null || true
cd ${FLEET_DEPLOYMENT_PATH}
FLEET_ID=<IDENTIFIER> python3 omni-matrix-engine.py
'"
