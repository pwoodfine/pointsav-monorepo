#!/bin/bash

# Dynamic Index Routing (Agnostic to Cluster Name)
INDEX_CARD="../totebox-index.env"

if [ ! -f "$INDEX_CARD" ]; then
    echo "FATAL: Index card not found at $INDEX_CARD. Cannot ignite."
    exit 1
fi

echo "SYSTEM EVENT: Reading configuration from Local Index Card..."
source "$INDEX_CARD"

echo "--------------------------------------------------------"
echo "WARNING: DESTRUCTIVE EGRESS PROTOCOL INITIATED ($ARCHIVE_OWNER)"
echo "--------------------------------------------------------"
echo "Please enter the absolute path to the Vault on the 1.0 TB external drive."
read -p "Vault Path: " VAULT_PATH

if [ ! -d "$VAULT_PATH/new" ]; then
    echo "FATAL: Valid Maildir structure (tmp/ new/ cur/) not found at $VAULT_PATH"
    exit 1
fi

export TOTEBOX_VAULT_PATH="$VAULT_PATH"

echo "SYSTEM EVENT: Negotiating fresh OAuth2 token with Microsoft Azure..."

RESPONSE=$(curl -s -X POST https://login.microsoftonline.com/$AZURE_TENANT_ID/oauth2/v2.0/token \
  -d "grant_type=client_credentials" \
  -d "client_id=$AZURE_CLIENT_ID" \
  -d "client_secret=$AZURE_CLIENT_SECRET" \
  -d "scope=https://graph.microsoft.com/.default")

TOKEN=$(echo "$RESPONSE" | grep -o '"access_token":"[^"]*' | grep -o '[^"]*$')

if [ -z "$TOKEN" ]; then
    echo "FATAL: Token negotiation failed. Verify your Azure credentials."
    exit 1
fi

echo "SYSTEM EVENT: Token acquired. Engaging Parity Gates for $ARCHIVE_OWNER..."
export AZURE_ACCESS_TOKEN="$TOKEN"

cargo run
