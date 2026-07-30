#!/bin/bash
set -euo pipefail

# © 2026 PointSav Digital Systems
# Institutional Brutalism: Ground-Truth CRM Primer (14-Column Provenance Engine)

INDEX_CARD="../totebox-index.env"
source "$INDEX_CARD"

CRM_PATH="../data-ledgers/crm_contacts.csv"
ROSTER_PATH="../data-ledgers/personnel_roster.jsonl"
VAULT_ROOT="$PHYSICAL_USB_PATH"

echo "SYSTEM EVENT: Initiating 14-Column Provenance Primer..."
rm -f "$CRM_PATH"

cat << 'PY_EOF' > prime_eml.py
import os, csv, sys, email, email.utils, re, json
from datetime import datetime

vault_dir = sys.argv[1]
crm_path = sys.argv[2]
jsonl_path = sys.argv[3]
default_owner = sys.argv[4]

contacts = {}
file_meta = {}

# Build Ephemeral Provenance Map
if os.path.exists(jsonl_path):
    with open(jsonl_path, 'r') as jf:
        for line in jf:
            if not line.strip(): continue
            try:
                record = json.loads(line)
                msg_id = record.get('MessageID', '')
                if msg_id:
                    safe_id = msg_id.replace('/', '_').replace('+', '-')
                    filename = safe_id + '.eml'
                    file_meta[filename] = {
                        'owner': record.get('ArchiveOwner', default_owner),
                        'folder': record.get('FolderName', 'In-Place Archive')
                    }
            except: pass

PHONE_RE = re.compile(r'(\+?\d{1,3}[-.\s]?\(?\d{2,4}\)?[-.\s]?\d{3,4}[-.\s]?\d{4})')
FWD_RE = re.compile(r'From:\s*"?([^"<]+)"?\s*<([a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,})>')
US_RE = re.compile(r'\b([A-Z][a-zA-Z\s.-]+),\s*([A-Z]{2})\s+(\d{5}(?:-\d{4})?)\b')
CA_RE = re.compile(r'\b([A-Z][a-zA-Z\s.-]+),\s*(BC|AB|ON|QC|NS|NB|MB|PE|SK|NL|YT|NT|NU)\s*([A-Za-z]\d[A-Za-z][\s-]?\d[A-Za-z]\d)\b', re.IGNORECASE)
UK_RE = re.compile(r'\b([A-Z][a-zA-Z\s.-]+),\s*([A-Z]{1,2}\d[A-Z\d]?\s*\d[A-Z]{2})\b', re.IGNORECASE)
GENERIC_CITY_STATE = re.compile(r'\b([A-Z][a-zA-Z\s.-]+),\s*(BC|AB|ON|QC|NS|NB|MB|PE|SK|NL|YT|NT|NU|WA|CA|NY|TX|FL|MA|IL|PA|UK|Canada|USA)\b', re.IGNORECASE)

FREE_DOMAINS = {'gmail.com', 'yahoo.com', 'hotmail.com', 'outlook.com', 'aol.com', 'icloud.com', 'me.com', 'mac.com'}
BLACKLIST = {'CPA', 'MBA', 'PHD', 'LLC', 'INC', 'LTD', 'CORP', 'MD'}
US_STATES = {'AL','AK','AZ','AR','CA','CO','CT','DE','FL','GA','HI','ID','IL','IN','IA','KS','KY','LA','ME','MD','MA','MI','MN','MS','MO','MT','NE','NV','NH','NJ','NM','NY','NC','ND','OH','OK','OR','PA','RI','SC','SD','TN','TX','UT','VT','VA','WA','WV','WI','WY','DC'}
CA_PROVS = {'AB','BC','MB','NB','NL','NS','NT','NU','ON','PE','QC','SK','YT'}

def get_body(msg):
    body = ""
    for part in msg.walk():
        if part.get_content_type() == 'text/plain':
            try:
                p = part.get_payload(decode=True)
                if p: body += p.decode('utf-8', 'ignore')
            except: pass
    return body

def extract_geospatial_data(body_text):
    geo = {'city': '', 'state': '', 'postal': '', 'country': ''}
    if not body_text: return geo
    lines = body_text.splitlines()
    sig_lines = lines[-40:] if len(lines) > 40 else lines
    
    for line in reversed(sig_lines):
        line = line.strip()
        if not (5 < len(line) < 150): continue
        us_match = US_RE.search(line)
        if us_match and us_match.group(1).strip().upper() not in BLACKLIST:
            geo['city'], geo['state'], geo['postal'], geo['country'] = us_match.group(1).strip(), us_match.group(2).strip().upper(), us_match.group(3).strip(), 'USA'
            return geo
        ca_match = CA_RE.search(line)
        if ca_match and ca_match.group(1).strip().upper() not in BLACKLIST:
            geo['city'], geo['state'], geo['postal'], geo['country'] = ca_match.group(1).strip(), ca_match.group(2).strip().upper(), ca_match.group(3).strip().upper(), 'Canada'
            return geo
        uk_match = UK_RE.search(line)
        if uk_match and uk_match.group(1).strip().upper() not in BLACKLIST:
            geo['city'], geo['postal'], geo['country'] = uk_match.group(1).strip(), uk_match.group(2).strip().upper(), 'United Kingdom'
            return geo

    for line in reversed(sig_lines):
        line = line.strip()
        if not (5 < len(line) < 150): continue
        gen_match = GENERIC_CITY_STATE.search(line)
        if gen_match and gen_match.group(1).strip().upper() not in BLACKLIST:
            geo['city'] = gen_match.group(1).strip()
            region = gen_match.group(2).strip().upper()
            if region in ['UK', 'UNITED KINGDOM']: geo['country'] = 'United Kingdom'
            elif region in ['CANADA']: geo['country'] = 'Canada'
            elif region in ['USA', 'UNITED STATES']: geo['country'] = 'USA'
            else: 
                geo['state'] = region
                if region in US_STATES: geo['country'] = 'USA'
                elif region in CA_PROVS: geo['country'] = 'Canada'
            return geo
    return geo

def process_person(e_addr, name, date_str, origin, body_text, owner, folder):
    if not e_addr: return
    e_addr = e_addr.lower().strip()
    name = name.strip() if name else ""
    
    if e_addr not in contacts:
        domain = e_addr.split('@')[-1] if '@' in e_addr else ''
        company = domain.split('.')[0].capitalize() if domain not in FREE_DOMAINS else ''
        website = domain if domain and domain not in FREE_DOMAINS else ''
        
        contacts[e_addr] = {
            'name': name, 'count': 0, 'last_contact': date_str,
            'phone': '', 'company': company, 'website': website,
            'city': '', 'state': '', 'postal': '', 'country': '', 'origin': origin,
            'owners': set(), 'folders': set()
        }
    
    contacts[e_addr]['count'] += 1
    if owner: contacts[e_addr]['owners'].add(owner)
    if folder: contacts[e_addr]['folders'].add(folder)
    if not contacts[e_addr]['name'] and name: contacts[e_addr]['name'] = name
    if date_str and date_str > contacts[e_addr]['last_contact']: contacts[e_addr]['last_contact'] = date_str
    
    if body_text:
        phones = PHONE_RE.findall(body_text)
        if phones and not contacts[e_addr]['phone']: contacts[e_addr]['phone'] = phones[-1]
            
        if not contacts[e_addr]['city'] and not contacts[e_addr]['postal']:
            geo = extract_geospatial_data(body_text)
            if geo['city'] or geo['postal']:
                contacts[e_addr]['city'] = geo['city']
                contacts[e_addr]['state'] = geo['state']
                contacts[e_addr]['postal'] = geo['postal']
                contacts[e_addr]['country'] = geo['country']

processed_count = 0
for root, dirs, files in os.walk(vault_dir):
    for filename in files:
        if filename.endswith(".eml"):
            meta = file_meta.get(filename, {'owner': default_owner, 'folder': 'In-Place Archive'})
            try:
                with open(os.path.join(root, filename), 'r') as f:
                    msg = email.message_from_file(f)
                
                raw_date = msg.get('Date')
                date_str = raw_date
                if raw_date:
                    parsed = email.utils.parsedate_tz(raw_date)
                    if parsed:
                        try:
                            dt = datetime.fromtimestamp(email.utils.mktime_tz(parsed))
                            date_str = dt.strftime('%Y-%m-%dT%H:%M:%SZ')
                        except: pass
                
                body_text = get_body(msg)
                
                for header_key in ['From', 'To', 'Cc']:
                    raw_header = msg.get_all(header_key, [])
                    for name, e_addr in email.utils.getaddresses(raw_header):
                        process_person(e_addr, name, date_str, 'Direct', body_text if header_key == 'From' else '', meta['owner'], meta['folder'])
                
                for match in FWD_RE.findall(body_text):
                    fwd_name, fwd_email = match
                    process_person(fwd_email, fwd_name, date_str, 'Forwarded Thread', '', meta['owner'], meta['folder'])
                    
                processed_count += 1
            except: pass

# Relational Cross-Pollination
domain_ledger = {}
for e_addr, d in contacts.items():
    domain = e_addr.split('@')[-1] if '@' in e_addr else ''
    if domain and domain not in FREE_DOMAINS:
        if domain not in domain_ledger:
            domain_ledger[domain] = {'city': '', 'state': '', 'postal': '', 'country': ''}
        if d['city'] and not domain_ledger[domain]['city']: domain_ledger[domain]['city'] = d['city']
        if d['state'] and not domain_ledger[domain]['state']: domain_ledger[domain]['state'] = d['state']
        if d['postal'] and not domain_ledger[domain]['postal']: domain_ledger[domain]['postal'] = d['postal']
        if d['country'] and not domain_ledger[domain]['country']: domain_ledger[domain]['country'] = d['country']

for e_addr, d in contacts.items():
    domain = e_addr.split('@')[-1] if '@' in e_addr else ''
    if domain in domain_ledger:
        if not d['city']: d['city'] = domain_ledger[domain]['city']
        if not d['state']: d['state'] = domain_ledger[domain]['state']
        if not d['postal']: d['postal'] = domain_ledger[domain]['postal']
        if not d['country']: d['country'] = domain_ledger[domain]['country']

sorted_contacts = sorted(contacts.items(), key=lambda x: x[1]['count'], reverse=True)
with open(crm_path, 'w') as f:
    writer = csv.writer(f)
    writer.writerow(['EmailAddress', 'Name', 'InteractionCount', 'LastContactDate', 'Phone', 'Company', 'Website', 'City', 'State_Province', 'Postal_Code', 'Country', 'EntityOrigin', 'MailboxOwner', 'SourceFolder'])
    for e_addr, d in sorted_contacts:
        owners_str = ' | '.join(filter(bool, d['owners']))
        folders_str = ' | '.join(filter(bool, d['folders']))
        row = [e_addr, d['name'], d['count'], d['last_contact'], d['phone'], d['company'], d['website'], d['city'], d['state'], d['postal'], d['country'], d['origin'], owners_str, folders_str]
        writer.writerow([unicode(x).encode('utf-8') if isinstance(x, unicode) else str(x) for x in row])

print(processed_count)
PY_EOF

PROCESSED=$(python prime_eml.py "$VAULT_ROOT" "$CRM_PATH" "$ROSTER_PATH" "$ARCHIVE_OWNER")
rm prime_eml.py

ACTUAL_CONTACTS=$(tail -n +2 "$CRM_PATH" | wc -l | tr -d ' ')
echo "SYSTEM EVENT: 14-Column Provenance Sweep complete on $PROCESSED physical payloads."
echo "SYSTEM EVENT: Fleet Tracking initialized across $ACTUAL_CONTACTS unique entities."
