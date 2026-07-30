#!/bin/bash

INDEX_CARD="../totebox-index.env"

if [ ! -f "$INDEX_CARD" ]; then
    echo "FATAL: Index card not found at $INDEX_CARD. Cannot ignite."
    exit 1
fi

echo "SYSTEM EVENT: Reading configuration from Local Index Card..."
source "$INDEX_CARD"

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

echo "SYSTEM EVENT: Token acquired. Engaging Cloud Bridge to MSFT 365..."
export AZURE_ACCESS_TOKEN="$TOKEN"

cargo run
