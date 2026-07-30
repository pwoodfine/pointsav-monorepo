#!/bin/bash
# PointSav Mesh Status Utility
# Tier-4 Infrastructure | Node 3 (iMac)

GCP_RELAY_IP="[ENTER_YOUR_GCP_STATIC_IP_HERE]"
LEASED_NODE_ID="NODE-LAPTOP-B"

echo "===================================================="
echo " 📡 PointSav Private Network (PPN) Status Check"
echo "===================================================="

# 1. Check Cloud Relay Connectivity
if ping -c 1 "$GCP_RELAY_IP" > /dev/null; then
    echo "🟢 NODE 2 (GCP RELAY): REACHABLE"
else
    echo "🔴 NODE 2 (GCP RELAY): OFFLINE"
fi

# 2. Check Edge Anchor Connectivity (via Tunnel)
# (This checks if the PSST tunnel is active to Node 1)
if ip route | grep -q "ppn0"; then
    echo "🟢 NODE 1 (LEASED): TUNNEL ACTIVE"
else
    echo "🔴 NODE 1 (LEASED): TUNNEL DOWN"
fi

echo "===================================================="
