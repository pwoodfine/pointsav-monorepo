#!/bin/bash
set -euo pipefail

# © 2026 PointSav Digital Systems
# Institutional Brutalism: Retroactive SLM Metadata Generator

INDEX_CARD="../totebox-index.env"
source "$INDEX_CARD"

VAULT_CUR="$PHYSICAL_USB_PATH/cur"
VAULT_NEW="$PHYSICAL_USB_PATH/new"
MASTER_LEDGER="../data-ledgers/master_metadata_ledger.jsonl"

echo "SYSTEM EVENT: Initiating SLM Metadata Retrofit Engine..."
echo "SYSTEM EVENT: Scanning physical platters for orphaned metadata..."

cat << 'PY_EOF' > retrofit_eml.py
import os, sys, json, email
from email.header import decode_header

vault_dirs = [sys.argv[1], sys.argv[2]]
master_jsonl = sys.argv[3]
owner = sys.argv[4]

def clean_header(header_val):
    if not header_val: return ""
    try:
        decoded = decode_header(header_val)
        res = ""
        for val, enc in decoded:
            if isinstance(val, bytes):
                res += val.decode(enc or 'utf-8', 'ignore')
            else:
                res += str(val)
        return res.replace('\n', '').replace('\r', '')
    except:
        return str(header_val).replace('\n', '').replace('\r', '')

processed = 0

with open(master_jsonl, 'a') as out_f:
    for v_dir in vault_dirs:
        if not os.path.isdir(v_dir): continue
        for root, dirs, files in os.walk(v_dir):
            for file in files:
                if file.endswith('.eml'):
                    filepath = os.path.join(root, file)
                    try:
                        with open(filepath, 'r') as in_f:
                            msg = email.message_from_file(in_f)
                            
                        # Reconstruct SLM-ready JSON record
                        record = {
                            "MessageID": clean_header(msg.get("Message-ID", file)),
                            "Subject": clean_header(msg.get("Subject", "No Subject")),
                            "DateReceived": clean_header(msg.get("Date", "")),
                            "Sender": {"EmailAddress": clean_header(msg.get("From", "")), "Name": ""},
                            "ToRecipients": [{"EmailAddress": clean_header(msg.get("To", ""))}],
                            "ArchiveOwner": owner,
                            "FolderName": "In-Place Archive (Retrofit)",
                            "HasAttachments": "true" if msg.is_multipart() else "false",
                            "IsRetrofit": "true"
                        }
                        out_f.write(json.dumps(record) + "\n")
                        processed += 1
                    except Exception as e:
                        pass

print(processed)
PY_EOF

PROCESSED=$(python retrofit_eml.py "$VAULT_CUR" "$VAULT_NEW" "$MASTER_LEDGER" "$ARCHIVE_OWNER")
rm retrofit_eml.py

echo "====================================================================="
echo "SYSTEM EVENT: Retrofit Complete."
echo "Successfully reverse-engineered $PROCESSED assets into the Master Metadata Ledger."
echo "====================================================================="
