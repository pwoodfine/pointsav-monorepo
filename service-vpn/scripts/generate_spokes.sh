#!/usr/bin/env bash
# TARGET: Tier 1 Monorepo (iMac) -> Spoke Factory
# PAYLOAD: service-vpn
# VENDOR: PointSav Digital Systems

set -e

# Cryptographic Anchor (Laptop-B)
HUB_PUBKEY="2e1K3zPXdTmG5vwQdjmUZ6RlzDg6MVDjpnGc52t3pXE"
ENDPOINT="24.86.192.209:51820"
OUTPUT_DIR="/home/mathew/Foundry/pointsav-monorepo/service-vpn/spokes"

mkdir -p "${OUTPUT_DIR}"
cd "${OUTPUT_DIR}"

echo "==> Generating Cryptographic Identity: Peter (MacBook Air - Mexico)"
wg genkey | tee peter_private | wg pubkey | tee peter_public
PETER_PRIV=$(cat peter_private)
PETER_PUB=$(cat peter_public)

echo "==> Generating Cryptographic Identity: Jennifer (MacPro - Support Parity)"
wg genkey | tee jennifer_private | wg pubkey | tee jennifer_public
JEN_PRIV=$(cat jennifer_private)
JEN_PUB=$(cat jennifer_public)

echo "==> Constructing peter-mexico.conf"
cat << WG_EOF > peter-mexico.conf
[Interface]
PrivateKey = ${PETER_PRIV}
Address = 10.8.0.2/32
DNS = 1.1.1.1, 8.8.8.8

[Peer]
PublicKey = ${HUB_PUBKEY}
Endpoint = ${ENDPOINT}
AllowedIPs = 0.0.0.0/0
PersistentKeepalive = 25
WG_EOF

echo "==> Constructing jennifer-macpro.conf"
cat << WG_EOF > jennifer-macpro.conf
[Interface]
PrivateKey = ${JEN_PRIV}
Address = 10.8.0.3/32
DNS = 1.1.1.1, 8.8.8.8

[Peer]
PublicKey = ${HUB_PUBKEY}
Endpoint = ${ENDPOINT}
AllowedIPs = 0.0.0.0/0
PersistentKeepalive = 25
WG_EOF

echo "==========================================================="
echo "SPOKE GENERATION COMPLETE."
echo "Configuration files saved to: ${OUTPUT_DIR}"
echo "==========================================================="
echo "CRITICAL REGISTRATION STEP:"
echo "You must execute the following mathematical authorizations"
echo "on fleet-infrastructure-leased (Laptop-B via SSH):"
echo "-----------------------------------------------------------"
echo "sudo wg set wg0 peer ${PETER_PUB} allowed-ips 10.8.0.2/32"
echo "sudo wg set wg0 peer ${JEN_PUB} allowed-ips 10.8.0.3/32"
echo "sudo wg-quick save wg0"
echo "==========================================================="
