#!/bin/bash
set -euo pipefail

INDEX_CARD="../totebox-index.env"
source "$INDEX_CARD"
ROSTER_PATH="../data-ledgers/personnel_roster.csv"
VAULT_DIR="$PHYSICAL_USB_PATH/new"

mkdir -p "$VAULT_DIR"

echo "SYSTEM EVENT: PointSav High-Fidelity Egress Engine (macOS 10.13.6 Target)"
echo "SYSTEM EVENT: Negotiating EWS Token..."
TOKEN=$(curl -s -X POST https://login.microsoftonline.com/$AZURE_TENANT_ID/oauth2/v2.0/token \
  -d "grant_type=client_credentials" \
  -d "client_id=$AZURE_CLIENT_ID" \
  --data-urlencode "client_secret=$AZURE_CLIENT_SECRET" \
  -d "scope=https://outlook.office365.com/.default" | grep -o '"access_token":"[^"]*' | grep -o '[^"]*$')

tail -n +2 "$ROSTER_PATH" | while IFS=',' read -r ARCHIVE_OWNER FOLDER_ID MESSAGE_ID REST; do
    if [ -z "$MESSAGE_ID" ]; then continue; fi
    
    ATTEMPT=1
    SUCCESS=0
    
    # 2030 Leapfrog: Exponential Backoff Network Loop
    while [ $ATTEMPT -le $NETWORK_RETRY_LIMIT ]; do
        echo "SYSTEM EVENT: Extracting Payload ${MESSAGE_ID:0:15}... (Attempt $ATTEMPT)"
        
        cat << XML > ews_mime_payload.xml
<?xml version="1.0" encoding="utf-8"?>
<soap:Envelope xmlns:soap="http://schemas.xmlsoap.org/soap/envelope/" xmlns:t="http://schemas.microsoft.com/exchange/services/2006/types" xmlns:m="http://schemas.microsoft.com/exchange/services/2006/messages"><soap:Header><t:RequestServerVersion Version="Exchange2016" /><t:ExchangeImpersonation><t:ConnectingSID><t:PrimarySmtpAddress>$EXCHANGE_TARGET_USER</t:PrimarySmtpAddress></t:ConnectingSID></t:ExchangeImpersonation></soap:Header><soap:Body><m:GetItem><m:ItemShape><t:BaseShape>IdOnly</t:BaseShape><t:IncludeMimeContent>true</t:IncludeMimeContent></m:ItemShape><m:ItemIds><t:ItemId Id="$MESSAGE_ID" /></m:ItemIds></m:GetItem></soap:Body></soap:Envelope>
XML

        RESPONSE=$(curl -s -X POST https://outlook.office365.com/EWS/Exchange.asmx \
          -H "Authorization: Bearer $TOKEN" \
          -H "Content-Type: text/xml; charset=utf-8" \
          -d @ews_mime_payload.xml || echo "NETWORK_FAIL")
          
        if [[ "$RESPONSE" == *"NETWORK_FAIL"* ]] || [[ -z "$RESPONSE" ]]; then
            echo "WARNING: Network destabilized. Re-engaging in $((ATTEMPT * 2)) seconds..."
            sleep $((ATTEMPT * 2))
            ((ATTEMPT++))
            continue
        fi

        MIME_B64=$(echo "$RESPONSE" | perl -0777 -ne 'print $1 if /<t:MimeContent[^>]*>(.*?)<\/t:MimeContent>/s')

        if [ -n "$MIME_B64" ]; then
            FILENAME="$(date +%s).$(cat /proc/sys/kernel/random/uuid 2>/dev/null || uuidgen)_V1_TOTEBOX.eml"
            echo "$MIME_B64" | base64 -d > "$VAULT_DIR/$FILENAME"
            echo "SUCCESS: Asset physically secured to $VAULT_DIR/$FILENAME."
            SUCCESS=1
            break
        else
            echo "WARNING: Kernel rejected MIME payload. Retrying..."
            sleep $((ATTEMPT * 2))
            ((ATTEMPT++))
        fi
    done
    
    if [ $SUCCESS -eq 0 ]; then
        echo "FATAL: Asset ${MESSAGE_ID:0:15} failed after $NETWORK_RETRY_LIMIT attempts. Halting to preserve data parity."
        exit 1
    fi
done
