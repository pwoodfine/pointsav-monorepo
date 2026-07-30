#!/usr/bin/env bash
# TARGET: fleet-infrastructure-leased (Laptop-B)
# PAYLOAD: Master Spoke Factory (Auto-Key Extraction)

set -e

if [ "$EUID" -ne 0 ]; then
  echo "FATAL: This script must be run as root."
  exit 1
fi

echo "==> [1/5] Extracting Pristine Hub Public Key..."
# This reads the exact 44-character key directly from the source
HUB_PUBKEY=$(cat /etc/wireguard/publickey)
ENDPOINT="24.86.192.209:51820"
WORK_DIR="/tmp/master_spokes"

mkdir -p "${WORK_DIR}"
cd "${WORK_DIR}"
umask 077

echo "==> [2/5] Minting Cryptographic Identities..."
# Peter MacBook Air
wg genkey | tee peter_private | wg pubkey | tee peter_public
PETER_PRIV=$(cat peter_private); PETER_PUB=$(cat peter_public)

# Jennifer MacPro
wg genkey | tee jen_mac_private | wg pubkey | tee jen_mac_public
JEN_MAC_PRIV=$(cat jen_mac_private); JEN_MAC_PUB=$(cat jen_mac_public)

# Jennifer Phone (Optical)
wg genkey | tee jen_phone_private | wg pubkey | tee jen_phone_public
JEN_PHONE_PRIV=$(cat jen_phone_private); JEN_PHONE_PUB=$(cat jen_phone_public)

echo "==> [3/5] Constructing Payload Assets..."
# Peter Mac
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

# Jennifer Mac
cat << WG_EOF > jennifer-macpro.conf
[Interface]
PrivateKey = ${JEN_MAC_PRIV}
Address = 10.8.0.3/32
DNS = 1.1.1.1, 8.8.8.8

[Peer]
PublicKey = ${HUB_PUBKEY}
Endpoint = ${ENDPOINT}
AllowedIPs = 0.0.0.0/0
PersistentKeepalive = 25
WG_EOF

# Jennifer Phone
cat << WG_EOF > jennifer-phone.conf
[Interface]
PrivateKey = ${JEN_PHONE_PRIV}
Address = 10.8.0.5/32
DNS = 1.1.1.1, 8.8.8.8

[Peer]
PublicKey = ${HUB_PUBKEY}
Endpoint = ${ENDPOINT}
AllowedIPs = 0.0.0.0/0
PersistentKeepalive = 25
WG_EOF

# Optical Export for Phone
apt-get install -y qrencode >/dev/null 2>&1
qrencode -t PNG -o jennifer-phone-qr.png < jennifer-phone.conf

echo "==> [4/5] Authorizing All Nodes on Hub..."
wg set wg0 peer ${PETER_PUB} allowed-ips 10.8.0.2/32
wg set wg0 peer ${JEN_MAC_PUB} allowed-ips 10.8.0.3/32
wg set wg0 peer ${JEN_PHONE_PUB} allowed-ips 10.8.0.5/32
wg-quick save wg0

echo "==> [5/5] Factory Complete."
chown -R laptop-b:laptop-b ${WORK_DIR}
