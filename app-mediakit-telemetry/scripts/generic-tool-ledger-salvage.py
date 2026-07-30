#!/usr/bin/env python3
# PointSav Digital Systems | V6.4 Generic Ledger Salvage & Deduplication Core
# Community Release (Sovereign Data Protocol)
import os
import glob
import csv
import sys

def salvage_ledger(target_directory):
    outbox = os.path.join(target_directory, "outbox")
    assets = os.path.join(target_directory, "assets")
    target = os.path.join(assets, "ledger_telemetry.csv")
    
    files = glob.glob(os.path.join(outbox, "RAW_*.csv"))
    if not files:
        print(f"[WARN] No backups found in {outbox}.")
        return
    
    unique_rows = set()
    for f in files:
        with open(f, 'r', encoding='utf-8') as csv_f:
            reader = csv.reader(csv_f)
            for row in reader:
                if len(row) >= 4:
                    unique_rows.add(tuple(row))
                    
    sorted_rows = sorted(list(unique_rows), key=lambda x: x[1]) 
    
    os.makedirs(assets, exist_ok=True)
    with open(target, 'w', newline='', encoding='utf-8') as out:
        writer = csv.writer(out)
        writer.writerows(sorted_rows)
        
    print(f"[SUCCESS] Master Ledger Salvaged: {len(sorted_rows)} absolute unique events restored to {target}")

if __name__ == "__main__":
    if len(sys.argv) < 2:
        print("[FATAL] Missing target directory.")
        print("Usage: python3 generic-tool-ledger-salvage.py <path_to_telemetry_module>")
        sys.exit(1)
        
    target_dir = sys.argv[1]
    if not os.path.isdir(target_dir):
        print(f"[FATAL] Target directory does not exist: {target_dir}")
        sys.exit(1)
        
    salvage_ledger(target_dir)
