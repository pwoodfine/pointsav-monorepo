#!/usr/bin/env python3
"""
PointSav Digital Systems | service-people
Deterministic Flat-File Personnel Ledger Engine
Temporal Upgrade: The 27-Hour Matrix (Home 07-22 & Target 08-20)
"""

import os
import json
import argparse
import datetime
import sys
try:
    from zoneinfo import ZoneInfo
except ImportError:
    print("FATAL: Python 3.9+ with zoneinfo module is required.", file=sys.stderr)
    sys.exit(1)

LEDGER_PATH = os.getenv("LEDGER_PATH", os.path.join(os.path.dirname(__file__), "ledger_personnel.json"))
HOME_TIMEZONE = "America/Vancouver"

def load_ledger():
    if not os.path.exists(LEDGER_PATH):
        return {"contacts": []}
    with open(LEDGER_PATH, 'r') as f:
        return json.load(f)

def save_ledger(data):
    with open(LEDGER_PATH, 'w') as f:
        json.dump(data, f, indent=4)

def check_dual_temporal_window(target_timezone_str):
    """
    Enforces the 27-Hour Dual-Timezone Intersection.
    Returns True ONLY if Home Time is 07:00-22:00 AND Target Time is 08:00-20:00.
    """
    if not target_timezone_str:
        return False
        
    now_utc = datetime.datetime.now(datetime.timezone.utc)
    
    try:
        # 1. Home Node Biological Guardrail (07:00 to 22:00)
        home_tz = ZoneInfo(HOME_TIMEZONE)
        home_local_time = now_utc.astimezone(home_tz)
        if not (7 <= home_local_time.hour < 22):
            return False
            
        # 2. Target Node Receiving Guardrail (08:00 to 20:00)
        target_tz = ZoneInfo(target_timezone_str)
        target_local_time = now_utc.astimezone(target_tz)
        if not (8 <= target_local_time.hour < 20):
            return False
            
        return True
    except Exception:
        # Failsafe: If timezone string is invalid, drop the target.
        return False

def query_targets(limit, campaign_id):
    ledger = load_ledger()
    targets = []
    
    for contact in ledger.get("contacts", []):
        if not contact.get("linkedin_url"):
            continue
            
        history = contact.get("communication_history", {})
        if campaign_id not in history:
            # Enforce the 27-Hour dual-timezone intersection
            if check_dual_temporal_window(contact.get("timezone")):
                targets.append(contact)
                if len(targets) >= limit:
                    break
                
    return targets

def update_state(contact_id, campaign_id):
    ledger = load_ledger()
    updated = False
    
    for contact in ledger.get("contacts", []):
        if contact.get("id") == contact_id:
            if "communication_history" not in contact:
                contact["communication_history"] = {}
                
            contact["communication_history"][campaign_id] = {
                "status": "SENT",
                "timestamp": datetime.datetime.now().isoformat()
            }
            updated = True
            break
            
    if updated:
        save_ledger(ledger)
        print(f"SUCCESS: Ledger updated for Contact ID: {contact_id}")
    else:
        print(f"FATAL: Contact ID: {contact_id} not found in ledger.", file=sys.stderr)
        sys.exit(1)

def main():
    parser = argparse.ArgumentParser(description="service-people: Master Contact Ledger")
    subparsers = parser.add_subparsers(dest="action", required=True)

    query_parser = subparsers.add_parser("query")
    query_parser.add_argument("--limit", type=int, default=75)
    query_parser.add_argument("--campaign", required=True)

    update_parser = subparsers.add_parser("update")
    update_parser.add_argument("--id", required=True)
    update_parser.add_argument("--campaign", required=True)

    args = parser.parse_args()

    if args.action == "query":
        targets = query_targets(args.limit, args.campaign)
        print(json.dumps(targets))

    elif args.action == "update":
        update_state(args.id, args.campaign)

if __name__ == "__main__":
    main()
