#!/bin/bash
set -euo pipefail

# © 2026 PointSav Digital Systems
# Institutional Brutalism: Phase 1 Autonomous Archive Crawler (Dual-State SLM Architecture)

INDEX_CARD="../totebox-index.env"
source "$INDEX_CARD"

ROSTER_PATH="../data-ledgers/personnel_roster.jsonl"
MASTER_LEDGER="../data-ledgers/master_metadata_ledger.jsonl"

# Dynamic Process Isolation Variables
PARSE_FOLDERS_PY="parse_folders_${TARGET_MAILBOX}.py"
PARSE_ITEMS_PY="parse_items_to_jsonl_${TARGET_MAILBOX}.py"

# The Volumetric Governor: ~800 Megabytes (838,860,800 Bytes)
TARGET_BYTES=838860800 
CURRENT_BYTES=0
PAGE_SIZE=100

echo "SYSTEM EVENT: Initiating Phase 1 Autonomous Archive Crawler [$TARGET_MAILBOX]..."
echo "SYSTEM EVENT: Wiping ephemeral daily kill list..."
rm -f "$ROSTER_PATH"
touch "$ROSTER_PATH"

echo "SYSTEM EVENT: Negotiating EWS Token..."
TOKEN=$(curl -s -X POST https://login.microsoftonline.com/$AZURE_TENANT_ID/oauth2/v2.0/token \
  -d "grant_type=client_credentials" \
  -d "client_id=$AZURE_CLIENT_ID" \
  --data-urlencode "client_secret=$AZURE_CLIENT_SECRET" \
  -d "scope=https://outlook.office365.com/.default" | grep -o '"access_token":"[^"]*' | grep -o '[^"]*$')

echo "SYSTEM EVENT: Mapping In-Place Archive Topology..."
cat << XML > ews_archive_folders.xml
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
        <t:AdditionalProperties>
            <t:FieldURI FieldURI="folder:DisplayName" />
        </t:AdditionalProperties>
      </m:FolderShape>
      <m:ParentFolderIds>
        <t:DistinguishedFolderId Id="archivemsgfolderroot" />
      </m:ParentFolderIds>
    </m:FindFolder>
  </soap:Body>
</soap:Envelope>
XML

curl -s -X POST https://outlook.office365.com/EWS/Exchange.asmx \
  -H "Authorization: Bearer $TOKEN" \
  -H "Content-Type: text/xml; charset=utf-8" \
  -d @ews_archive_folders.xml > ews_folders_response.xml

# Python Kernel: Extract Folder IDs and Names (Isolated)
cat << PY_EOF > "$PARSE_FOLDERS_PY"
import xml.etree.ElementTree as ET, sys
ns = {'t': 'http://schemas.microsoft.com/exchange/services/2006/types'}
try:
    tree = ET.parse(sys.argv[1])
    for f in tree.findall('.//{http://schemas.microsoft.com/exchange/services/2006/types}Folder'):
        f_id = f.find('t:FolderId', ns).attrib.get('Id', '')
        d_name = f.find('t:DisplayName', ns).text if f.find('t:DisplayName', ns) is not None else 'Unknown'
        print("{}|{}".format(f_id, d_name.encode('utf-8') if isinstance(d_name, unicode) else d_name))
except Exception:
    pass
PY_EOF

python "$PARSE_FOLDERS_PY" ews_folders_response.xml > folder_queue.txt
FOLDER_COUNT=$(wc -l < folder_queue.txt | tr -d ' ')
echo "SYSTEM EVENT: Discovered $FOLDER_COUNT Archive sub-folders. Hunting for oldest chronological assets..."

# Python Kernel: Convert EWS Items to SLM JSONL (Isolated)
cat << PY_EOF > "$PARSE_ITEMS_PY"
import xml.etree.ElementTree as ET, sys, json
ns = {'t': 'http://schemas.microsoft.com/exchange/services/2006/types', 'm': 'http://schemas.microsoft.com/exchange/services/2006/messages'}
try:
    tree = ET.parse(sys.argv[1])
    owner = sys.argv[2]
    folder_name = sys.argv[3]
    
    for item in tree.findall('.//t:Message', ns) + tree.findall('.//t:CalendarItem', ns):
        msg_id_node = item.find('t:ItemId', ns)
        if msg_id_node is None: continue
        
        subj = item.find('t:Subject', ns)
        size = item.find('t:Size', ns)
        dt = item.find('t:DateTimeReceived', ns)
        has_att = item.find('t:HasAttachments', ns)
        
        sender = {"EmailAddress": "", "Name": ""}
        s_node = item.find('t:Sender/t:Mailbox', ns) or item.find('t:From/t:Mailbox', ns)
        if s_node is not None:
            e = s_node.find('t:EmailAddress', ns)
            n = s_node.find('t:Name', ns)
            if e is not None and e.text: sender["EmailAddress"] = e.text
            if n is not None and n.text: sender["Name"] = n.text

        record = {
            "MessageID": msg_id_node.attrib.get('Id', ''),
            "Subject": subj.text if subj is not None else '',
            "DateReceived": dt.text if dt is not None else '',
            "Sender": sender,
            "ToRecipients": [],
            "CcRecipients": [],
            "ArchiveOwner": owner,
            "FolderName": folder_name,
            "HasAttachments": has_att.text.lower() if has_att is not None else 'false',
            "SizeBytes": size.text if size is not None else '0',
            "IsRead": "true"
        }
        print(json.dumps(record))
        print("SIZE_SYNC|{}".format(record["SizeBytes"]))
except Exception:
    pass
PY_EOF

while IFS='|' read -r FOLDER_ID FOLDER_NAME; do
    if [ -z "$FOLDER_ID" ]; then continue; fi
    if [ "$CURRENT_BYTES" -ge "$TARGET_BYTES" ]; then break; fi

    OFFSET=0
    while [ "$CURRENT_BYTES" -lt "$TARGET_BYTES" ]; do
        cat << XML > ews_find_items.xml
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
          -d @ews_find_items.xml > ews_items_response.xml

        # Check if empty page
        if ! grep -q 't:ItemId' ews_items_response.xml; then
            break
        fi

        # Parse XML, output JSON lines, and extract sizes (Isolated)
        python "$PARSE_ITEMS_PY" ews_items_response.xml "$ARCHIVE_OWNER" "$FOLDER_NAME" > raw_output.txt

        grep -v "^SIZE_SYNC|" raw_output.txt >> "$ROSTER_PATH" || true
        
        # Tally the bytes securely
        while read -r SIZE_LINE; do
            FILE_BYTES=$(echo "$SIZE_LINE" | cut -d'|' -f2)
            if [[ "$FILE_BYTES" =~ ^[0-9]+$ ]]; then
                CURRENT_BYTES=$((CURRENT_BYTES + FILE_BYTES))
            fi
        done < <(grep "^SIZE_SYNC|" raw_output.txt)

        echo -n "."
        OFFSET=$((OFFSET + PAGE_SIZE))
    done
done < folder_queue.txt

# Clean up temp files
rm -f ews_archive_folders.xml ews_folders_response.xml folder_queue.txt ews_find_items.xml ews_items_response.xml raw_output.txt
rm -f "$PARSE_FOLDERS_PY" "$PARSE_ITEMS_PY"

CURRENT_MB=$((CURRENT_BYTES / 1024 / 1024))
TOTAL_EXTRACTED=$(wc -l < "$ROSTER_PATH" | tr -d ' ')

echo ""
echo "====================================================================="
echo "SYSTEM EVENT: Phase 1 Crawler Complete."
echo "Secured $TOTAL_EXTRACTED Assets to Kill List ($CURRENT_MB MB Data Volume)."
echo "====================================================================="

# THE 2030 UPGRADE: Secure the Metadata to the permanent SLM Ledger
echo "SYSTEM EVENT: Aggregating Metadata to Permanent SLM Master Ledger..."
cat "$ROSTER_PATH" >> "$MASTER_LEDGER"
echo "SYSTEM EVENT: AI Data Infrastructure secured. Ready for Phase 2."
