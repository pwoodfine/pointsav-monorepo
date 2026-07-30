#!/usr/bin/env bash
# TARGET: fleet-infrastructure-leased (Laptop-B)
# PAYLOAD: Mobile Optical Spoke Factory (PNG Export)

set -e

if [ "$EUID" -ne 0 ]; then
  echo "FATAL: This script must be run as root."
  exit 1
fi

echo "==> [1/4] Installing Optical Tooling (qrencode)..."
apt-get update -qq
apt-get install -y qrencode

HUB_PUBKEY="2e1K3zPXdTmG5vwQdjmUZ6RlzDg6MVDjpnGc52t3pXE"
ENDPOINT="24.86.192.209:51820"
MOBILE_IP="10.8.0.5/32"
WORK_DIR="/tmp/mobile_spoke"

mkdir -p "${WORK_DIR}"
cd "${WORK_DIR}"
umask 077

echo "==> [2/4] Generating Mobile Cryptographic Identity (Jennifer Phone)..."
wg genkey | tee jen_phone_private | wg pubkey | tee jen_phone_public
PHONE_PRIV=$(cat jen_phone_private)
PHONE_PUB=$(cat jen_phone_public)

echo "==> [3/4] Constructing Mobile Payload and PNG Asset..."
cat << WG_EOF > jennifer-phone.conf
[Interface]
PrivateKey = ${PHONE_PRIV}
Address = ${MOBILE_IP}
DNS = 1.1.1.1, 8.8.8.8

[Peer]
PublicKey = ${HUB_PUBKEY}
Endpoint = ${ENDPOINT}
AllowedIPs = 0.0.0.0/0
PersistentKeepalive = 25
WG_EOF

# Export payload as a permanent PNG image file
qrencode -t PNG -o jennifer-phone-qr.png < jennifer-phone.conf

echo "==> [4/4] Authorizing Mobile Node on Hub..."
wg set wg0 peer ${PHONE_PUB} allowed-ips ${MOBILE_IP}
wg-quick save wg0

echo "==> Remote Mobile Factory Complete."
chown -R laptop-b:laptop-b ${WORK_DIR}
