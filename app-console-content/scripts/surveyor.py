#!/usr/bin/env python3
import os
import sys
import json
import csv
import datetime
import shutil

# Active Deployment Boundaries
BASE_DIR = "/home/mathew/deployments/woodfine-fleet-deployment/cluster-totebox-personnel"
QUEUE_DIR = os.path.join(BASE_DIR, "service-people/discovery-queue")
VERIFIED_DIR = os.path.join(BASE_DIR, "service-people/verified-ledger")
ARCHETYPES_CSV = os.path.join(BASE_DIR, "service-content/ontology/archetypes.csv")
THROTTLE_FILE = os.path.expanduser("~/.pointsav_surveyor_count")

MAX_DAILY_VERIFICATIONS = 10

def check_throttle():
    today = datetime.datetime.now().strftime("%Y-%m-%d")
    count = 0
    if os.path.exists(THROTTLE_FILE):
        with open(THROTTLE_FILE, "r") as f:
            data = f.read().strip().split(",")
            if len(data) == 2 and data[0] == today:
                count = int(data[1])
                
    if count >= MAX_DAILY_VERIFICATIONS:
        print(f"\n[SYSTEM HALT] Cognitive Throttle Reached.")
        print(f"[MANDATE] You have completed the maximum {MAX_DAILY_VERIFICATIONS} verifications for {today}.")
        print(f"[DIRECTIVE] The terminal is locked until 00:00 to preserve data fidelity.\n")
        sys.exit(0)
    return today, count

def increment_throttle(today, count):
    with open(THROTTLE_FILE, "w") as f:
        f.write(f"{today},{count + 1}")

def load_archetypes():
    archetypes = {}
    if not os.path.exists(ARCHETYPES_CSV):
        print(f"[ERROR] Ontology missing: {ARCHETYPES_CSV}")
        sys.exit(1)
    with open(ARCHETYPES_CSV, "r") as f:
        reader = csv.DictReader(f)
        for row in reader:
            archetypes[row['id']] = row['name']
    return archetypes

def run_surveyor():
    print("========================================================")
    print(" 🧭 POINTSAV CONSOLE: VERIFICATION SURVEYOR")
    print("========================================================")
    
    today, count = check_throttle()
    print(f"[SYSTEM] Operator authenticated. Daily capacity: {count}/{MAX_DAILY_VERIFICATIONS}\n")
    
    if not os.path.exists(QUEUE_DIR):
        print("[SYSTEM] Discovery queue directory missing. Awaiting ingestion.")
        return
        
    os.makedirs(VERIFIED_DIR, exist_ok=True)
    
    fragments = [f for f in os.listdir(QUEUE_DIR) if f.endswith('.json')]
    if not fragments:
        print("[SUCCESS] The Discovery Queue is mathematically empty. Zero pending fragments.")
        return

    # Load Ontology
    archetypes = load_archetypes()
    
    # Pull the Top Index Card
    target_file = fragments[0]
    target_path = os.path.join(QUEUE_DIR, target_file)
    
    with open(target_path, "r") as f:
        entity = json.load(f)
        
    claims = entity.get("claims", {})
    
    print("--------------------------------------------------------")
    print(" 📇 UNVERIFIED INDEX CARD")
    print("--------------------------------------------------------")
    print(f" ID:        {entity.get('sovereign_id')}")
    print(f" Name:      {entity.get('display_name', 'Unknown')}")
    print(f" Email:     {claims.get('email', 'Unknown')}")
    print(f" Company:   {claims.get('company', 'Unknown')}")
    print(f" Position:  {claims.get('position', 'Unknown')}")
    print(f" Source:    {entity.get('provenance', {}).get('source_file', 'Unknown')}")
    print("--------------------------------------------------------")
    print("\n[ACTION REQUIRED] Execute air-gapped verification using external browser.")
    
    url = input("Paste Verified LinkedIn URL (or type 'reject' / 'skip'): ").strip()
    
    if url.lower() == 'skip':
        print("[SYSTEM] Fragment skipped. Yielding to next cycle.")
        return
    elif url.lower() == 'reject':
        print("[SYSTEM] Fragment rejected. Obliterating from queue.")
        os.remove(target_path)
        return
        
    print("\n[ONTOLOGY] Select target Archetype:")
    for arch_id, name in archetypes.items():
        print(f"  {arch_id}) {name}")
        
    arch_selection = input("\nEnter Archetype ID: ").strip()
    
    if arch_selection not in archetypes:
        print("[ERROR] Invalid Archetype ID. Halting extraction to preserve ontology.")
        return
        
    # Enrich the JSON
    entity["status"] = "Verified"
    entity["claims"]["verified_url"] = url
    entity["claims"]["archetype"] = archetypes[arch_selection]
    entity["provenance"]["verification_timestamp"] = datetime.datetime.now(datetime.timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")
    
    # Save to Verified Ledger
    verified_path = os.path.join(VERIFIED_DIR, target_file)
    with open(verified_path, "w") as f:
        json.dump(entity, f, indent=4)
        
    # Remove from Queue
    os.remove(target_path)
    
    increment_throttle(today, count)
    print(f"\n[SUCCESS] Identity permanently socketed to {verified_path}")
    print(f"[SYSTEM] Cognitive throttle updated: {count + 1}/{MAX_DAILY_VERIFICATIONS}")

if __name__ == "__main__":
    run_surveyor()
