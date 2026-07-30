#!/bin/bash
set -euo pipefail

cat << 'PY_EOF' > run_audit.py
import os, json, hashlib

env_vars = {}
with open("totebox-index.env", "r") as f:
    for line in f:
        if line.strip() and not line.startswith("#") and "=" in line:
            k, v = line.strip().split("=", 1)
            env_vars[k] = v.strip('"\n\'')

usb_path = env_vars.get("PHYSICAL_USB_PATH")
if not usb_path:
    print("FATAL: PHYSICAL_USB_PATH not defined in totebox-index.env")
    exit(1)

vault_new = os.path.join(usb_path, "new")
vault_cur = os.path.join(usb_path, "cur")
ledger_path = "data-ledgers/master_metadata_ledger.jsonl"

try: cur_files = set(os.listdir(vault_cur))
except: cur_files = set()

try: new_files = set(os.listdir(vault_new))
except: new_files = set()

# HEURISTIC CHECK 1: Vault Overlap
critical_alerts = []
overlap = cur_files.intersection(new_files)
if len(overlap) > 0:
    critical_alerts.append("VAULT OVERLAP DETECTED: " + str(len(overlap)) + " files exist in both /new and /cur. Transit failure.")

total_ledger = 0
secured_cur = 0
secured_new = 0
pending = 0
corrupted_legacy = 0

if not os.path.exists(ledger_path):
    print("SYSTEM EVENT: Master Ledger not found. Waiting for Phase 1 Primer.")
    exit(0)

with open(ledger_path, 'r') as f:
    for line in f:
        if not line.strip(): continue
        total_ledger += 1
        try:
            record = json.loads(line)
            msg_id = record.get("MessageID") or record.get("Id") or record.get("ItemId")
            if not msg_id: continue

            hex_id = hashlib.md5(msg_id.encode('utf-8')).hexdigest()
            legacy_id = msg_id.replace('/', '_').replace('+', '-')
            
            hex_eml = hex_id + ".eml"
            legacy_eml = legacy_id + ".eml"

            if hex_eml in cur_files:
                secured_cur += 1
            elif hex_eml in new_files:
                secured_new += 1
            elif legacy_eml in cur_files or legacy_eml in new_files:
                corrupted_legacy += 1
            else:
                pending += 1
        except:
            pass

# HEURISTIC CHECK 2: Legacy Trap
if corrupted_legacy > 0:
    critical_alerts.append("LEGACY TRAP TRIGGERED: " + str(corrupted_legacy) + " un-hexed APFS-vulnerable files detected.")

# HEURISTIC CHECK 3: Ledger Fracture
if total_ledger != (secured_cur + secured_new + pending + corrupted_legacy):
    critical_alerts.append("LEDGER FRACTURE: Mathematical drift detected between total records and physical drive state.")

print("\n=======================================================")
print(" POINT-SAV OMNI-VAULT INTEGRITY AUDIT (V3)")
print("=======================================================")
print(" Cloud Archive (Master Ledger): " + str(total_ledger) + " Assets")
print(" ------------------------------------------------------")
print(" Secured in Cold Storage:       " + str(secured_cur))
print(" Secured in Staging (New):      " + str(secured_new))
print(" Pending Extraction:            " + str(pending))
print("=======================================================")

if len(critical_alerts) > 0:
    print("\n!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!")
    print(" [ CRITICAL HALT ] - DO NOT PROCEED TO PHASE 4")
    print("!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!")
    for alert in critical_alerts:
        print(" -> " + alert)
    print("!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!")
    print(" CONTACT ENGINEERING FOR SURGICAL RESOLUTION.\n")
else:
    print("\n [ SYSTEM CLEAR - SAFE TO PROCEED ]")
    if total_ledger > 0 and total_ledger == (secured_cur + secured_new):
        print(" STATUS: 100% PARITY. ARCHIVE FULLY SECURED.\n")
    else:
        secured_total = secured_cur + secured_new
        if total_ledger > 0:
            pct = round((float(secured_total) / total_ledger) * 100, 2)
            print(" STATUS: EXTRACTION PIPELINE ACTIVE (" + str(pct) + "% Complete)\n")

PY_EOF

python run_audit.py
rm -f run_audit.py
