#!/bin/bash
set -euo pipefail

INDEX_CARD="../totebox-index.env"
source "$INDEX_CARD"
ROSTER_PATH="../data-ledgers/personnel_roster.jsonl"

echo "SYSTEM EVENT: Igniting Auto-Hunter Diagnostic Probe..."
TOKEN=$(curl -s -X POST https://login.microsoftonline.com/$AZURE_TENANT_ID/oauth2/v2.0/token \
  -d "grant_type=client_credentials" \
  -d "client_id=$AZURE_CLIENT_ID" \
  --data-urlencode "client_secret=$AZURE_CLIENT_SECRET" \
  -d "scope=https://outlook.office365.com/.default" | grep -o '"access_token":"[^"]*' | grep -o '[^"]*$')

MESSAGE_IDS=$(python -c "import sys, json; [sys.stdout.write(json.loads(line).get('MessageID','') + '\n') for line in sys.stdin if line.strip()]" < "$ROSTER_PATH")

for MESSAGE_ID in $MESSAGE_IDS; do
    if [ -z "$MESSAGE_ID" ]; then continue; fi

    cat << XML > ews_diag_payload.xml
<?xml version="1.0" encoding="utf-8"?>
<soap:Envelope xmlns:soap="http://schemas.xmlsoap.org/soap/envelope/"
               xmlns:t="http://schemas.microsoft.com/exchange/services/2006/types"
               xmlns:m="http://schemas.microsoft.com/exchange/services/2006/messages">
  <soap:Header>
    <t:RequestServerVersion Version="Exchange2016" />
    <t:ExchangeImpersonation>
      <t:ConnectingSID><t:PrimarySmtpAddress>$EXCHANGE_TARGET_USER</t:PrimarySmtpAddress></t:ConnectingSID>
    </t:ExchangeImpersonation>
  </soap:Header>
  <soap:Body>
    <m:DeleteItem DeleteType="HardDelete">
      <m:ItemIds>
        <t:ItemId Id="$MESSAGE_ID" />
      </m:ItemIds>
    </m:DeleteItem>
  </soap:Body>
</soap:Envelope>
XML

    RESPONSE=$(curl -s -X POST https://outlook.office365.com/EWS/Exchange.asmx \
      -H "Authorization: Bearer $TOKEN" \
      -H "Content-Type: text/xml; charset=utf-8" \
      -d @ews_diag_payload.xml)

    # If it is NOT a Success and NOT an ItemNotFound, we caught the anomaly!
    if ! echo "$RESPONSE" | grep -q 'ResponseClass="Success"' && ! echo "$RESPONSE" | grep -q 'ErrorItemNotFound'; then
        echo "==================== MICROSOFT RAW OUTPUT ===================="
        echo "$RESPONSE" | xmllint --format - || echo "$RESPONSE"
        echo "=============================================================="
        rm -f ews_diag_payload.xml
        exit 1
    fi
done

echo "SYSTEM EVENT: Auto-Hunter finished. No unknown errors found."
rm -f ews_diag_payload.xml
