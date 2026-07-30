#!/usr/bin/env bash
# TARGET: fleet-infrastructure-leased (Laptop-B / Linux Mint)
# PAYLOAD: service-vpn
# VENDOR: PointSav Digital Systems
# CUSTOMER: Woodfine Management Corp.

set -e

if [ "$EUID" -ne 0 ]; then
  echo "FATAL: This script must be run as root."
  exit 1
fi

echo "==> [1/5] Installing Dependencies..."
apt-get update
apt-get install -y wireguard iptables ufw

echo "==> [2/5] Generating Cryptographic Keys..."
mkdir -p /etc/wireguard
cd /etc/wireguard
umask 077
wg genkey | tee privatekey | wg pubkey | tee publickey
PRIVATE_KEY=$(cat privatekey)

echo "==> [3/5] Configuring Kernel-Level IPv4 Forwarding..."
cat << 'SYSCTL_EOF' > /etc/sysctl.d/99-wireguard-forwarding.conf
net.ipv4.ip_forward=1
SYSCTL_EOF
sysctl -p /etc/sysctl.d/99-wireguard-forwarding.conf

echo "==> [4/5] Constructing wg0.conf for Physical Egress..."
PRIMARY_IFACE=$(ip route | grep default | awk '{print $5}' | head -n 1)

cat << WG_EOF > /etc/wireguard/wg0.conf
[Interface]
Address = 10.8.0.1/24
ListenPort = 51820
PrivateKey = ${PRIVATE_KEY}
SaveConfig = false

# IP Masquerading for Physical Egress (Customer Routing)
PostUp = iptables -A FORWARD -i wg0 -j ACCEPT; iptables -t nat -A POSTROUTING -o ${PRIMARY_IFACE} -j MASQUERADE
PostDown = iptables -D FORWARD -i wg0 -j ACCEPT; iptables -t nat -D POSTROUTING -o ${PRIMARY_IFACE} -j MASQUERADE
WG_EOF

echo "==> [5/5] Aligning Firewall and Bootstrapping Service..."
ufw allow 51820/udp
systemctl enable wg-quick@wg0
systemctl restart wg-quick@wg0

echo "==> Hub Provisioning Complete."
echo "==> Hub Public Key: $(cat /etc/wireguard/publickey)"
