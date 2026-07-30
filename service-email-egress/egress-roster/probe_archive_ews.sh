#!/bin/bash
set -euo pipefail

INDEX_CARD="../totebox-index.env"
source "$INDEX_CARD"

echo "SYSTEM EVENT: Negotiating EWS Token for Executive Officer Archive..."
TOKEN=$(curl -s -X POST https://login.microsoftonline.com/$AZURE_TENANT_ID/oauth2/v2.0/token \
  -d "grant_type=client_credentials" \
  -d "client_id=$AZURE_CLIENT_ID" \
  --data-urlencode "client_secret=$AZURE_CLIENT_SECRET" \
  -d "scope=https://outlook.office365.com/.default" | grep -o '"access_token":"[^"]*' | grep -o '[^"]*$')

if [ -z "$TOKEN" ]; then
    echo "FATAL: Failed to negotiate EWS Token."
    exit 1
fi

echo "SYSTEM EVENT: Forging XML SOAP payload for the Exchange Kernel (Target: $EXCHANGE_TARGET_USER)..."
cat << XML > ews_payload.xml
<?xml version="1.0" encoding="utf-8"?>
<soap:Envelope xmlns:soap="http://schemas.xmlsoap.org/soap/envelope/"
               xmlns:t="http://schemas.microsoft.com/exchange/services/2006/types"
               xmlns:m="http://schemas.microsoft.com/exchange/services/2006/messages">
  <soap:Header>
    <t:RequestServerVersion Version="Exchange2016" />
    <t:ExchangeImpersonation>
      <t:ConnectingSID>
        <t:PrimarySmtpAddress>$EXCHANGE_TARGET_USER</t:PrimarySmtpAddress>
      </t:ConnectingSID>
    </t:ExchangeImpersonation>
  </soap:Header>
  <soap:Body>
    <m:FindFolder Traversal="Shallow">
      <m:FolderShape>
        <t:BaseShape>Default</t:BaseShape>
      </m:FolderShape>
      <m:ParentFolderIds>
        <t:DistinguishedFolderId Id="archivemsgfolderroot" />
      </m:ParentFolderIds>
    </m:FindFolder>
  </soap:Body>
</soap:Envelope>
XML

echo "SYSTEM EVENT: Firing EWS SOAP sequence at Peter's In-Place Archive..."
curl -s -X POST https://outlook.office365.com/EWS/Exchange.asmx \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: text/xml; charset=utf-8" \
  -d @ews_payload.xml | grep -iE "DisplayName|FolderId Id"
