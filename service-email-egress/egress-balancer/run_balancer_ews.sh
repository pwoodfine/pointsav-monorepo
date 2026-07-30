#!/bin/bash
# SPDX-License-Identifier: AGPL-3.0-or-later
# SPDX-FileCopyrightText: 2026 Woodfine Capital Projects Inc.

set -euo pipefail

# © 2026 PointSav Digital Systems
# Institutional Brutalism: Phase 5 Adaptive Thermodynamic Balancer

INDEX_CARD="../totebox-index.env"
source "$INDEX_CARD"

# Dynamic Process Isolation Variables
PRIMARY_PARSER_PY="balancer_primary_parser_${TARGET_MAILBOX}.py"
EXPIRED_PARSER_PY="balancer_expired_parser_${TARGET_MAILBOX}.py"

# Enforce the 10-Month Boundary
CUTOFF_DATE=$(date -v-10m +"%Y-%m-%dT%H:%M:%SZ")

# The Volumetric Governor: 800 Megabytes (838,860,800 Bytes)
TARGET_BYTES=838860800 
CURRENT_BYTES=0
MOVED_COUNT=0
PAGE_SIZE=100

echo "SYSTEM EVENT: Initiating Phase 5 Adaptive Thermodynamic Balancer [$TARGET_MAILBOX]..."
echo "SYSTEM EVENT: Enforcing 10-Month Boundary (Cutoff: $CUTOFF_DATE)"
echo "SYSTEM EVENT: Setting Volumetric Governor to ~800 MB Target..."

TOKEN=$(curl -s -X POST https://login.microsoftonline.com/$AZURE_TENANT_ID/oauth2/v2.0/token \
  -d "grant_type=client_credentials" \
  -d "client_id=$AZURE_CLIENT_ID" \
  --data-urlencode "client_secret=$AZURE_CLIENT_SECRET" \
  -d "scope=https://outlook.office365.com/.default" | grep -o '"access_token":"[^"]*' | grep -o '[^"]*$')

echo "SYSTEM EVENT: Mapping Primary Mailbox Architecture..."
cat << XML > ews_primary_folders.xml
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
    <m:FindFolder Traversal="Deep">
      <m:FolderShape>
        <t:BaseShape>IdOnly</t:BaseShape>
      </m:FolderShape>
      <m:ParentFolderIds>
        <t:DistinguishedFolderId Id="msgfolderroot" />
      </m:ParentFolderIds>
    </m:FindFolder>
  </soap:Body>
</soap:Envelope>
XML

curl -s -X POST https://outlook.office365.com/EWS/Exchange.asmx \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: text/xml; charset=utf-8" \
  -d @ews_primary_folders.xml > ews_primary_folders_response.xml

# Python kernel to parse primary folders (Isolated)
cat << PY_EOF > "$PRIMARY_PARSER_PY"
import xml.etree.ElementTree as ET, sys; tree=ET.parse(sys.argv[1]); [sys.stdout.write(f.attrib.get('Id','')+'\n') for f in tree.findall('.//{http://schemas.microsoft.com/exchange/services/2006/types}FolderId')]
PY_EOF

python "$PRIMARY_PARSER_PY" ews_primary_folders_response.xml > primary_folder_queue.txt

FOLDER_COUNT=$(wc -l < primary_folder_queue.txt | tr -d ' ')
echo "SYSTEM EVENT: Discovered $FOLDER_COUNT primary sub-folders. Hunting for expired assets..."

# Python kernel to parse ItemID and Size dynamically (Isolated)
cat << PY_EOF > "$EXPIRED_PARSER_PY"
import xml.etree.ElementTree as ET, sys
ns = {'t': 'http://schemas.microsoft.com/exchange/services/2006/types'}
try:
    tree = ET.parse(sys.argv[1])
    for item in tree.findall('.//t:ItemId/..', ns):
        i_id = item.find('t:ItemId', ns).attrib.get('Id', '')
        size_node = item.find('t:Size', ns)
        size_val = size_node.text if size_node is not None else '0'
        print("{}|{}".format(i_id, size_val))
except Exception:
    pass
PY_EOF

while read -r FOLDER_ID; do
    if [ -z "$FOLDER_ID" ]; then continue; fi
    if [ "$CURRENT_BYTES" -ge "$TARGET_BYTES" ]; then break; fi

    OFFSET=0
    while [ "$CURRENT_BYTES" -lt "$TARGET_BYTES" ]; do
        cat << XML > ews_find_expired.xml
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
    <m:FindItem Traversal="Shallow">
      <m:ItemShape>
        <t:BaseShape>Default</t:BaseShape>
      </m:ItemShape>
      <m:IndexedPageItemView MaxEntriesReturned="$PAGE_SIZE" Offset="$OFFSET" BasePoint="Beginning" />
      <m:Restriction>
        <t:IsLessThan>
          <t:FieldURI FieldURI="item:DateTimeReceived" />
          <t:FieldURIOrConstant>
            <t:Constant Value="$CUTOFF_DATE" />
          </t:FieldURIOrConstant>
        </t:IsLessThan>
      </m:Restriction>
      <m:ParentFolderIds>
        <t:FolderId Id="$FOLDER_ID" />
      </m:ParentFolderIds>
    </m:FindItem>
  </soap:Body>
</soap:Envelope>
XML

        curl -s -X POST https://outlook.office365.com/EWS/Exchange.asmx \
          -H "Authorization: Bearer $TOKEN" \
          -H "Content-Type: text/xml; charset=utf-8" \
          -d @ews_find_expired.xml > ews_expired_response.xml

        ITEMS_DATA=$(python "$EXPIRED_PARSER_PY" ews_expired_response.xml)
        
        if [ -z "$ITEMS_DATA" ]; then
            break # Folder empty of expired items, move to next folder
        fi

        for ITEM in $ITEMS_DATA; do
            if [ -z "$ITEM" ]; then continue; fi
            if [ "$CURRENT_BYTES" -ge "$TARGET_BYTES" ]; then break; fi

            MESSAGE_ID=$(echo "$ITEM" | cut -d'|' -f1)
            MESSAGE_SIZE=$(echo "$ITEM" | cut -d'|' -f2)

            cat << XML > ews_move_payload.xml
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
    <m:MoveItem>
      <m:ToFolderId>
        <t:DistinguishedFolderId Id="archivemsgfolderroot" />
      </m:ToFolderId>
      <m:ItemIds>
        <t:ItemId Id="$MESSAGE_ID" />
      </m:ItemIds>
    </m:MoveItem>
  </soap:Body>
</soap:Envelope>
XML

            RESPONSE=$(curl -s -X POST https://outlook.office365.com/EWS/Exchange.asmx \
              -H "Authorization: Bearer $TOKEN" \
              -H "Content-Type: text/xml; charset=utf-8" \
              -d @ews_move_payload.xml)

            if echo "$RESPONSE" | grep -q 'ResponseClass="Success"'; then
                echo -n "."
                CURRENT_BYTES=$((CURRENT_BYTES + MESSAGE_SIZE))
                ((MOVED_COUNT++))
            fi
        done
        
        OFFSET=$((OFFSET + PAGE_SIZE))
    done
done < primary_folder_queue.txt

CURRENT_MB=$((CURRENT_BYTES / 1024 / 1024))

echo ""
echo "====================================================================="
echo "PHASE 5 COMPLETE: Adaptive Thermodynamic Balance Achieved."
echo "Total Assets Siphoned to Archive: $MOVED_COUNT"
echo "Total Volume Displaced: $CURRENT_MB MB"
echo "====================================================================="

rm -f ews_primary_folders.xml ews_primary_folders_response.xml primary_folder_queue.txt ews_find_expired.xml ews_expired_response.xml ews_move_payload.xml
rm -f "$PRIMARY_PARSER_PY" "$EXPIRED_PARSER_PY"
