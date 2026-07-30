
# [SECURITY OVERRIDE 1] Physical Anchor Verification
if [ ! -f "/Volumes/BACKUP-DRIVE/.pointsav_anchor" ]; then
    echo "CRITICAL SYSTEM HALT: Physical USB Anchor (.pointsav_anchor) not found!"
    echo "Aborting to prevent Phantom Drive data bleed to internal SSD."
    exit 1
fi
#!/bin/bash
set -euo pipefail

# © 2026 PointSav Digital Systems
# Institutional Brutalism: Master EWS Destructor (Calendar Fallback Edition)

INDEX_CARD="../totebox-index.env"
source "$INDEX_CARD"
ROSTER_PATH="../data-ledgers/personnel_roster.jsonl"
VAULT_NEW="$PHYSICAL_USB_PATH/new"
VAULT_CUR="$PHYSICAL_USB_PATH/cur"

# Dynamic Process Isolation Variables
PRUNE_PARSER_PY="prune_roster_parser_${TARGET_MAILBOX}.py"

mkdir -p "$VAULT_CUR"

echo "[INIT] PointSav Master Pruning Daemon (macOS Target: $EXCHANGE_TARGET_USER) [$TARGET_MAILBOX]"

if [ ! -f "$ROSTER_PATH" ]; then
    echo "FATAL: JSONL Ledger not found at $ROSTER_PATH"
    exit 1
fi

PHYSICAL_COUNT=$(find "$VAULT_NEW" -maxdepth 1 -type f -name "*.eml" | wc -l | tr -d ' ')
LEDGER_COUNT=$(wc -l < "$ROSTER_PATH" | tr -d ' ')

echo "[AUDIT] Physical payload count in Staging (new): $PHYSICAL_COUNT"
echo "[AUDIT] Ledger kill-list count: $LEDGER_COUNT"

if [ "$PHYSICAL_COUNT" -lt "$LEDGER_COUNT" ]; then
    echo "======================================================="
    echo "[SECURITY] CRITICAL PARITY FAILURE!"
    echo "SYSTEM HALTED. No destruction commands will be issued."
    echo "======================================================="
    exit 1
fi

echo "======================================================="
echo "[SECURITY] PARITY VERIFIED. ARCHIVE BOUNDARY PIERCED."
echo "======================================================="
read -p "Awaiting Kinetic Lock. Type EXECUTE to permanently destroy Archive assets: " KINETIC_LOCK

if [ "$KINETIC_LOCK" != "EXECUTE" ]; then
    echo "SYSTEM EVENT: Kinetic Lock aborted."
    exit 0
fi

echo "SYSTEM EVENT: Negotiating EWS Token for Phase 3 Destruction..."
TOKEN=$(curl -s -X POST https://login.microsoftonline.com/$AZURE_TENANT_ID/oauth2/v2.0/token \
  -d "grant_type=client_credentials" \
  -d "client_id=$AZURE_CLIENT_ID" \
  --data-urlencode "client_secret=$AZURE_CLIENT_SECRET" \
  -d "scope=https://outlook.office365.com/.default" | grep -o '"access_token":"[^"]*' | grep -o '[^"]*$')

echo "SYSTEM EVENT: Igniting Master HardDelete Sequence..."

# Python Kernel: Roster Parse (Isolated)
cat << PY_EOF > "$PRUNE_PARSER_PY"
import sys, json; [sys.stdout.write(json.loads(line).get('MessageID','') + '\n') for line in sys.stdin if line.strip()]
PY_EOF

MESSAGE_IDS=$(python "$PRUNE_PARSER_PY" < "$ROSTER_PATH")

for MESSAGE_ID in $MESSAGE_IDS; do
    if [ -z "$MESSAGE_ID" ]; then continue; fi

    cat << XML > ews_delete_payload.xml
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
      -d @ews_delete_payload.xml)

    if echo "$RESPONSE" | grep -q 'ResponseClass="Success"'; then
        echo "SUCCESS: Email Asset ${MESSAGE_ID:0:10}... vaporized."
    elif echo "$RESPONSE" | grep -q 'ErrorItemNotFound'; then
        echo "SUCCESS: Asset ${MESSAGE_ID:0:10}... already confirmed dead (Dumpster)."
    elif echo "$RESPONSE" | grep -q 'ErrorSendMeetingCancellationsRequired'; then
        
        # [FALLBACK ENGINE]: Dynamically inject the SendToNone attribute for Calendar Items
        cat << XML > ews_delete_cal_payload.xml
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
    <m:DeleteItem DeleteType="HardDelete" SendMeetingCancellations="SendToNone">
      <m:ItemIds>
        <t:ItemId Id="$MESSAGE_ID" />
      </m:ItemIds>
    </m:DeleteItem>
  </soap:Body>
</soap:Envelope>
XML
        CAL_RESPONSE=$(curl -s -X POST https://outlook.office365.com/EWS/Exchange.asmx \
          -H "Authorization: Bearer $TOKEN" \
          -H "Content-Type: text/xml; charset=utf-8" \
          -d @ews_delete_cal_payload.xml)
          
        if echo "$CAL_RESPONSE" | grep -q 'ResponseClass="Success"'; then
            echo "SUCCESS: Calendar Asset ${MESSAGE_ID:0:10}... silently vaporized."
        else
            echo "ERROR: Calendar Fallback failed for ${MESSAGE_ID:0:10}..."
        fi
        rm -f ews_delete_cal_payload.xml

    else
        echo "ERROR: Unknown Exchange rejection for ${MESSAGE_ID:0:10}..."
    fi
done

rm -f ews_delete_payload.xml "$PRUNE_PARSER_PY"
echo "SYSTEM EVENT: Phase 3 Archive Egress Complete."

echo "SYSTEM EVENT: Executing Phase 4 (Cold Storage Commit)..."
rsync -a --remove-source-files "$VAULT_NEW/" "$VAULT_CUR/" 2>/dev/null || true
echo "SYSTEM EVENT: All physical payloads shifted to Cold Storage (cur)."
echo "SYSTEM EVENT: Staging folder cleared. Pipeline primed for next cycle."
