#!/usr/bin/env bash
set -euo pipefail

if [ -z "${REMOTE_TARGET:-}" ] || [ -z "${TOTEBOX_ROOT:-}" ]; then
    echo "[ERROR] Environment variables REMOTE_TARGET and TOTEBOX_ROOT must be set."
    exit 1
fi

LOCAL_EXPORT_DIR="${LOCAL_EXPORT_DIR:-./Sovereign-Exports/People}"

echo "[SYSTEM] Injecting Substrate Flattener to $REMOTE_TARGET (with Sudo)..."
ssh "$REMOTE_TARGET" "sudo python3 -c \"
import os, json, csv
substrate_file = '${TOTEBOX_ROOT}/service-people/substrate/claims.jsonl'
out_file = '/tmp/personnel_export.csv'
data = []

if os.path.exists(substrate_file):
    with open(substrate_file, 'r') as f:
        for line in f:
            if line.strip():
                try:
                    d = json.loads(line)
                    data.append([
                        d.get('value', 'Unknown'),
                        d.get('attribute', 'Unknown'),
                        d.get('source_id', 'Unknown'),
                        d.get('target_uuid', 'Unknown'),
                        d.get('timestamp', 'Unknown')
                    ])
                except: pass

with open(out_file, 'w', newline='') as f:
    w = csv.writer(f)
    w.writerow(['Extracted Value', 'Attribute Type', 'Source Document', 'Sovereign UUID', 'Timestamp'])
    w.writerows(data)
\" && sudo chmod 644 /tmp/personnel_export.csv"

mkdir -p "$LOCAL_EXPORT_DIR"
rsync -avz "$REMOTE_TARGET:/tmp/personnel_export.csv" "$LOCAL_EXPORT_DIR/"
ssh "$REMOTE_TARGET" "sudo rm -f /tmp/personnel_export.csv"
echo "[SUCCESS] Personnel Substrate extracted to: $LOCAL_EXPORT_DIR"
