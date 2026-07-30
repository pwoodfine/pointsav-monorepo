#!/bin/bash
# ==============================================================================
# PointSav Digital Systems: UI Relay Wiring
# Target: GCP-Node (os-console UI Chassis)
# ==============================================================================

UI_DIR="/home/mathew/Foundry/factory-pointsav/pointsav-monorepo/os-console"
RELAY_URL="http://127.0.0.1:3000"

echo "[SYS] Scanning presentation layer for legacy ingestion endpoints..."

# Target all JS controllers and HTML cartridges
# Replacing relative or legacy API calls with the strict local relay vector
find "$UI_DIR" -type f -name "*.js" -exec sed -i "s|/api/ingest|$RELAY_URL|g" {} +
find "$UI_DIR" -type f -name "*.html" -exec sed -i "s|/api/ingest|$RELAY_URL|g" {} +

echo "[SUCCESS] JavaScript controllers successfully tethered to the Cryptographic Relay."
