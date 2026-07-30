#!/bin/bash
set -euo pipefail

# © 2026 PointSav Digital Systems
# Institutional Brutalism: Omnichannel Fusion Engine (Schema Parity Matrix)

EMAIL_CRM_PATH="/Users/Office/Foundry/pointsav-monorepo/service-totebox-egress/data-ledgers/crm_contacts.csv"
LINKEDIN_LEDGER_PATH="/Users/Office/Foundry/pointsav-monorepo/service-totebox-parser/archive.jsonl"
UNIFIED_CRM_PATH="/Users/Office/Foundry/pointsav-monorepo/service-totebox-egress/data-ledgers/unified_master_crm.csv"

echo "SYSTEM EVENT: Initiating Omnichannel Fusion Sequence (18-Column Parity Mode)..."

cat << 'PY_EOF' > fusion_engine.py
import os
import sys
import csv
import json
from datetime import datetime, timezone

email_crm_path = sys.argv[1]
linkedin_ledger_path = sys.argv[2]
unified_crm_path = sys.argv[3]

unified_ledger = {}

def parse_date(date_str):
    if not date_str or date_str == 'N/A':
        return None
    try:
        # Handle Email Format (2026-03-22T14:30:00Z)
        if 'T' in date_str and date_str.endswith('Z'):
            dt = datetime.strptime(date_str, '%Y-%m-%dT%H:%M:%SZ')
            return dt.replace(tzinfo=timezone.utc)
        
        # Handle LinkedIn Format (ISO format with +offset)
        dt = datetime.fromisoformat(date_str.replace('Z', '+00:00'))
        if dt.tzinfo is None:
            dt = dt.replace(tzinfo=timezone.utc)
        return dt
    except Exception:
        return None

def get_latest_date(date1, date2):
    d1 = parse_date(date1)
    d2 = parse_date(date2)
    if d1 and d2:
        return date1 if d1 > d2 else date2
    if d1: return date1
    if d2: return date2
    return 'N/A'

# 1. Ingest Email Truth (crm_contacts.csv) - Exact 14 Column Parity
if os.path.exists(email_crm_path):
    with open(email_crm_path, 'r', encoding='utf-8') as f:
        reader = csv.reader(f)
        header = next(reader, None)
        for row in reader:
            if len(row) >= 14:
                e_addr = row[0].strip().lower()
                name = row[1].strip()
                primary_key = e_addr if e_addr else name.lower()
                
                if primary_key:
                    unified_ledger[primary_key] = {
                        'EmailAddress': e_addr,
                        'Name': name,
                        'InteractionCount': row[2],
                        'LastContactDate': row[3],
                        'Phone': row[4],
                        'Company': row[5],
                        'Website': row[6],
                        'City': row[7],
                        'State_Province': row[8],
                        'Postal_Code': row[9],
                        'Country': row[10],
                        'EntityOrigin': row[11],
                        'MailboxOwner': row[12],
                        'SourceFolder': row[13],
                        'LinkedInURL': 'N/A',
                        'LinkedIn_Status': 'N/A',
                        'LinkedIn_Headline': 'N/A',
                        'Data_Provenance': 'Email Archive'
                    }

# 2. Ingest & Synthesize LinkedIn Truth (archive.jsonl)
if os.path.exists(linkedin_ledger_path):
    with open(linkedin_ledger_path, 'r', encoding='utf-8') as f:
        for line in f:
            if not line.strip(): continue
            try:
                record = json.loads(line)
                li_email = record.get('target_email', '').strip().lower()
                li_email = "" if li_email == "n/a" else li_email
                
                li_name = record.get('extracted_name', '').strip()
                primary_key = li_email if li_email else li_name.lower()
                
                # Synthesis Logic
                if primary_key in unified_ledger:
                    existing = unified_ledger[primary_key]
                    existing['LinkedInURL'] = record.get('target_url', existing['LinkedInURL'])
                    existing['LinkedIn_Status'] = record.get('action_simulated', existing['LinkedIn_Status'])
                    existing['LastContactDate'] = get_latest_date(existing['LastContactDate'], record.get('timestamp', ''))
                    existing['Data_Provenance'] = 'Omnichannel Fusion'
                    existing['LinkedIn_Headline'] = record.get('scraped_headline', 'N/A')
                    
                    # Patch missing data
                    if not existing['Name'] and li_name: existing['Name'] = li_name
                    if not existing['Company'] or existing['Company'] == '': 
                        existing['Company'] = record.get('scraped_headline', 'N/A')
                    
                    # Smart Geo-Parsing (Splitting "City, State, Country" string)
                    scraped_loc = record.get('scraped_location', 'N/A')
                    if scraped_loc != 'N/A' and not existing['City'] and not existing['Country']:
                        parts = [p.strip() for p in scraped_loc.split(',')]
                        if len(parts) == 3:
                            existing['City'], existing['State_Province'], existing['Country'] = parts[0], parts[1], parts[2]
                        elif len(parts) == 2:
                            existing['City'], existing['Country'] = parts[0], parts[1]
                        else:
                            existing['City'] = parts[0]
                        
                else:
                    # New Entity (LinkedIn Only)
                    scraped_loc = record.get('scraped_location', 'N/A')
                    city, state, country = 'N/A', 'N/A', 'N/A'
                    if scraped_loc != 'N/A':
                        parts = [p.strip() for p in scraped_loc.split(',')]
                        if len(parts) == 3:
                            city, state, country = parts[0], parts[1], parts[2]
                        elif len(parts) == 2:
                            city, country = parts[0], parts[1]
                        else:
                            city = parts[0]
                            
                    unified_ledger[primary_key] = {
                        'EmailAddress': li_email if li_email else 'N/A',
                        'Name': li_name,
                        'InteractionCount': 0,
                        'LastContactDate': record.get('timestamp', 'N/A'),
                        'Phone': 'N/A',
                        'Company': record.get('scraped_headline', 'N/A'),
                        'Website': 'N/A',
                        'City': city,
                        'State_Province': state,
                        'Postal_Code': 'N/A',
                        'Country': country,
                        'EntityOrigin': 'LinkedIn Prospect',
                        'MailboxOwner': 'N/A',
                        'SourceFolder': 'N/A',
                        'LinkedInURL': record.get('target_url', 'N/A'),
                        'LinkedIn_Status': record.get('action_simulated', 'N/A'),
                        'LinkedIn_Headline': record.get('scraped_headline', 'N/A'),
                        'Data_Provenance': 'LinkedIn Parser'
                    }
            except Exception as e:
                pass

# 3. Output Unified Artifact
utc_min = datetime.min.replace(tzinfo=timezone.utc)
sorted_records = sorted(unified_ledger.values(), key=lambda x: parse_date(x['LastContactDate']) or utc_min, reverse=True)

headers = [
    'EmailAddress', 'Name', 'InteractionCount', 'LastContactDate', 
    'Phone', 'Company', 'Website', 'City', 'State_Province', 'Postal_Code', 
    'Country', 'EntityOrigin', 'MailboxOwner', 'SourceFolder', 
    'LinkedInURL', 'LinkedIn_Status', 'LinkedIn_Headline', 'Data_Provenance'
]

with open(unified_crm_path, 'w', encoding='utf-8', newline='') as f:
    writer = csv.writer(f)
    writer.writerow(headers)
    for d in sorted_records:
        writer.writerow([d[h] for h in headers])

print(len(sorted_records))
PY_EOF

PROCESSED=$(python3 fusion_engine.py "$EMAIL_CRM_PATH" "$LINKEDIN_LEDGER_PATH" "$UNIFIED_CRM_PATH")
rm -f fusion_engine.py

echo "SYSTEM EVENT: Omnichannel Fusion complete."
echo "SYSTEM EVENT: 18-Column Unified Master CRM generated across $PROCESSED total unique entities."
echo "OUTPUT ROUTED TO: $UNIFIED_CRM_PATH"

