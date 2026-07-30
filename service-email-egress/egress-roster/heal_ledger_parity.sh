#!/bin/bash
set -euo pipefail

# © 2026 PointSav Digital Systems
# Institutional Brutalism: Sovereign Parity Healer (Ghost Asset Amputation)

echo "SYSTEM EVENT: Initiating Sovereign Parity Healer..."

cat << 'PY_EOF' > parity_healer.py
import json, os, sys

env_path = "../totebox-index.env"
with open(env_path, 'r') as f:
    env_lines = f.readlines()

usb_path = ""
for line in env_lines:
    if line.startswith("PHYSICAL_USB_PATH="):
        usb_path = line.strip().split("=")[1].strip('"\n')
        break

if not usb_path:
    print("FATAL: Could not read PHYSICAL_USB_PATH from totebox-index.env")
    sys.exit(1)

roster_path = "../data-ledgers/personnel_roster.jsonl"
vault_new = os.path.join(usb_path, "new")

valid_lines = []
ghost_count = 0

try:
    with open(roster_path, 'r') as f:
        for line in f:
            if not line.strip(): continue
            try:
                record = json.loads(line)
                msg_id = record.get("MessageID", "")
                safe_id = msg_id.replace('/', '_').replace('+', '-')
                target_file = os.path.join(vault_new, safe_id + ".eml")
                
                if os.path.exists(target_file) and os.path.getsize(target_file) > 0:
                    valid_lines.append(line)
                else:
                    ghost_count += 1
                    print("SYSTEM EVENT: Amputating un-mineable ghost asset from Kill List -> {}...".format(safe_id[:15]))
            except Exception:
                continue
except Exception as e:
    print("FATAL: Failed to read ledger. " + str(e))
    sys.exit(1)

with open(roster_path, 'w') as f:
    for line in valid_lines:
        f.write(line)

print("=======================================================")
print("SYSTEM EVENT: Ledger mathematically synced.")
print("Ghost Assets Purged: {}".format(ghost_count))
print("New Kill List Count: {}".format(len(valid_lines)))
print("=======================================================")
PY_EOF

python parity_healer.py
rm -f parity_healer.py
