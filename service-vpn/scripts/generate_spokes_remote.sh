#!/usr/bin/env bash
# TARGET: fleet-infrastructure-leased (Laptop-B / Linux Mint)
# PAYLOAD: Remote Spoke Factory & Node Authorization

set -e

if [ "$EUID" -ne 0 ]; then
  echo "FATAL: This script must be run as root."
  exit 1
fi

HUB_PUBKEY="2e1K3zPXdTmG5vwQdjmUZ6RlzDg6MVDjpnGc52t3pXE"
ENDPOINT="24.86.192.209:51820"
OUTPUT_DIR="/tmp/spokes"

mkdir -p "${OUTPUT_DIR}"
cd "${OUTPUT_DIR}"
umask 077

echo "==> Generating Keys: Peter (MacBook Air - Mexico)"
wg genkey | tee peter_private | wg pubkey | tee peter_public
PETER_PRIV=$(cat peter_private)
PETER_PUB=$(cat peter_public)

echo "==> Generating Keys: Jennifer (MacPro - Local Parity)"
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

echo "==> Authorizing Nodes on Hub (wg0)..."
wg set wg0 peer ${PETER_PUB} allowed-ips 10.8.0.2/32
wg set wg0 peer ${JEN_PUB} allowed-ips 10.8.0.3/32
wg-quick save wg0

echo "==> Remote Factory Complete."
# Adjust permissions so the non-root laptop-b user can scp them back
chown -R laptop-b:laptop-b ${OUTPUT_DIR}
