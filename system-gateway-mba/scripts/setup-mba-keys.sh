#!/bin/bash
# PointSav Digital Systems: MBA Key Generation
# System: os-console (GCP-Node)
# Objective: Generate Ed25519 identity for secure totebox communication

KEY_DIR="/home/mathew/Foundry/factory-pointsav/pointsav-monorepo/system-gateway-mba/keys"
KEY_NAME="gcp_node_mba_ed25519"

echo "[SYS] Initializing Machine-Based Authentication Cryptographic Directory..."
# Lock down the directory to the current user (mathew)
chmod 700 "$KEY_DIR"

echo "[SYS] Generating Ed25519 Keypair..."
# -t ed25519: Specifies the high-security curve
# -N "": Creates the key without a passphrase to allow automated machine-to-machine sync
# -C: Adds an operational comment for the asset ledger
ssh-keygen -t ed25519 -f "$KEY_DIR/$KEY_NAME" -N "" -C "os-console-gcp-node-identity"

echo "[SYS] Securing Assets (Least Privilege)..."
chmod 600 "$KEY_DIR/$KEY_NAME"
chmod 644 "$KEY_DIR/$KEY_NAME.pub"

echo "[SYS] MBA Keypair Generation Complete."
echo "--------------------------------------------------------"
echo "PRIVATE KEY PATH : $KEY_DIR/$KEY_NAME"
echo "PUBLIC KEY PATH  : $KEY_DIR/$KEY_NAME.pub"
echo "--------------------------------------------------------"
echo "[ACTION REQUIRED] Execute this command to view the Public Key for the iMac Vault:"
echo "cat $KEY_DIR/$KEY_NAME.pub"
