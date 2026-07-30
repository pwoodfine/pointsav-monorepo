#!/bin/bash
# PointSav Digital Systems | Tier 1 to Tier 2 Strict Pull Diode (Template)
set -euo pipefail

REMOTE_TARGET="<REMOTE_IP_ADDRESS>"
REMOTE_USER="<OPERATOR_USERNAME>"
TODAY=$(date +%Y-%m-%d)
LOCAL_DIR="/path/to/local/deployment/app-mediakit-telemetry"

mkdir -p "${LOCAL_DIR}/outbox"
echo "[COMM] Synchronizing telemetry reports..."
rsync -aqz "${REMOTE_USER}@${REMOTE_TARGET}:/opt/deployments/pointsav-fleet-deployment/media-marketing-landing/app-mediakit-telemetry/outbox/" "${LOCAL_DIR}/outbox/"
