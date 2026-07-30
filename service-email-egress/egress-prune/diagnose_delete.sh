#!/bin/bash
set -euo pipefail

# © 2026 PointSav Digital Systems
INDEX_CARD="../totebox-index.env"
source "$INDEX_CARD"
ROSTER_PATH="../data-ledgers/personnel_roster.jsonl"

echo "SYSTEM EVENT: Firing Single Destruction Probe at Exchange Kernel..."
TOKEN=$(curl -s -X POST https://login.microsoftonline.com/$AZURE_TENANT_ID/oauth2/v2.0/token \
  -d "grant_type=client_credentials" \
  -d "client_id=$AZURE_CLIENT_ID" \
  --data-urlencode "client_secret=$AZURE_CLIENT_SECRET" \
  -d "scope=https://outlook.office365.com/.default" | grep -o '"access_token":"[^"]*' | grep -o '[^"]*$')

# Extract the very first MessageID
MESSAGE_ID=$(head -n 1 "$ROSTER_PATH" | python -c "import sys, json; print(json.loads(sys.stdin.read()).get('MessageID',''))")

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

echo "==================== MICROSOFT RAW OUTPUT ===================="
curl -s -X POST https://outlook.office365.com/EWS/Exchange.asmx \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: text/xml; charset=utf-8" \
  -d @ews_diag_payload.xml | xmllint --format - || echo "XML Parsing Failed."
echo "=============================================================="

rm -f ews_diag_payload.xml
