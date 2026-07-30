#!/usr/bin/env python3
# PointSav Digital Systems | V6.4 Omni-Matrix Engine (Typographic & Temporal Core)
import os
import csv
from datetime import datetime
from collections import Counter
import sys
import glob

try:
    import maxminddb
    GEO_ENABLED = True
except ImportError:
    print("[WARN] maxminddb library missing. Geo-Routing disabled.")
    GEO_ENABLED = False

fleet_id = os.environ.get("FLEET_ID", "UNKNOWN").upper()

if fleet_id == "POINTSAV":
    wordmark = "POINTSAV DIGITAL SYSTEMS AG"
    entity_role = "VENDOR (ENGINEERING & SYSTEM LOGIC)"
elif fleet_id == "WOODFINE":
    wordmark = "WOODFINE MANAGEMENT CORP."
    entity_role = "CUSTOMER (OPERATIONAL EXECUTION & ASSET LEDGERS)"
else:
    sys.exit(1)

base_path = "."
csv_path = os.path.join(base_path, "assets", "ledger_telemetry.csv")
outbox_path = os.path.join(base_path, "outbox")
mmdb_files = glob.glob(os.path.join(base_path, "vendors-maxmind", "*.mmdb"))
mmdb_path = mmdb_files[0] if mmdb_files else None

date_str = os.environ.get("EXEC_DATE", datetime.now().strftime("%Y-%m-%d"))
exec_date = datetime.strptime(date_str, "%Y-%m-%d")
out_file = os.path.join(outbox_path, f"REPORT_{fleet_id}_{date_str}.md")
os.makedirs(outbox_path, exist_ok=True)

def format_table(headers, rows):
    table = [headers] + rows
    widths = [max(len(str(item)) for item in col) for col in zip(*table)]
    res = f"| {' | '.join(str(v).ljust(w) for v, w in zip(table[0], widths))} |\n"
    res += f"| {' | '.join(':' + '-'*(w-1) for w in widths)} |\n"
    for row in table[1:]:
        res += f"| {' | '.join(str(v).ljust(w) for v, w in zip(row, widths))} |\n"
    return res + "\n"

def calc_density(count, total):
    return "0.0%" if total == 0 else f"{(count / total) * 100:.1f}%"

def generate_matrix(title, headers, counter_data, total_events, top_n=None):
    items = counter_data.most_common(top_n) if top_n else counter_data.most_common()
    rows = []
    for item, count in items:
        row_name = " | ".join(item) if isinstance(item, tuple) else str(item)
        rows.append([row_name, str(count), calc_density(count, total_events)])
    return f"## {title}\n" + format_table(headers + ["Volume", "Density %"], rows)

def parse_ua(ua_string):
    ua_lower = ua_string.lower()
    os_name = "Unknown OS"
    if "windows" in ua_lower: os_name = "Windows"
    elif "mac os x" in ua_lower or "macos" in ua_lower: os_name = "macOS"
    elif "linux" in ua_lower: os_name = "Linux"
    elif "android" in ua_lower: os_name = "Android"
    elif "iphone" in ua_lower or "ipad" in ua_lower: os_name = "iOS"
    
    device_type = "Mobile / Tablet" if ("mobi" in ua_lower or "android" in ua_lower or "iphone" in ua_lower) else "Desktop / Server"
    return os_name, device_type

try:
    with open(csv_path, mode='r', encoding='utf-8-sig') as f:
        rows = list(csv.reader(f))
        total_events = len(rows)
        if total_events == 0: sys.exit(0)

        geo_global_data, geo_metro_data, uris_data, devices_data, systems_data, uas_data = [], [], [], [], [], []
        t_matrix = { k: {"ev": 0, "ip": set()} for k in ["Yesterday", "7_Days", "30_Days", "60_Days", "90_Days", "YTD", "Inception"] }
        geo_reader = maxminddb.open_database(mmdb_path) if (GEO_ENABLED and mmdb_path) else None

        for row in rows:
            if len(row) < 4: continue
            ip, ts_raw, uri, ua = row[0], row[1], row[2], row[3]
            try:
                event_date = datetime.strptime(ts_raw[:19], "%Y-%m-%dT%H:%M:%S")
                delta_days = (exec_date - event_date).days
            except ValueError:
                event_date, delta_days = exec_date, 0
            
            t_matrix["Inception"]["ev"] += 1; t_matrix["Inception"]["ip"].add(ip)
            if delta_days <= 1: t_matrix["Yesterday"]["ev"] += 1; t_matrix["Yesterday"]["ip"].add(ip)
            if delta_days <= 7: t_matrix["7_Days"]["ev"] += 1; t_matrix["7_Days"]["ip"].add(ip)
            if delta_days <= 30: t_matrix["30_Days"]["ev"] += 1; t_matrix["30_Days"]["ip"].add(ip)
            if delta_days <= 60: t_matrix["60_Days"]["ev"] += 1; t_matrix["60_Days"]["ip"].add(ip)
            if delta_days <= 90: t_matrix["90_Days"]["ev"] += 1; t_matrix["90_Days"]["ip"].add(ip)
            if event_date.year == exec_date.year: t_matrix["YTD"]["ev"] += 1; t_matrix["YTD"]["ip"].add(ip)
            
            os_name, device_type = parse_ua(ua)
            uris_data.append(uri); devices_data.append(device_type); systems_data.append(os_name); uas_data.append(ua[:75])
            country, region, city = "Unknown", "Unknown", "Unknown City"
            if geo_reader:
                try:
                    match = geo_reader.get(ip)
                    if match:
                        country = match.get('country', {}).get('names', {}).get('en', 'Unknown')
                        subdivisions = match.get('subdivisions', [])
                        if subdivisions: region = subdivisions[0].get('names', {}).get('en', 'Unknown')
                        city = match.get('city', {}).get('names', {}).get('en', 'Unknown City')
                except Exception: pass
            geo_global_data.append((country, region)); geo_metro_data.append(city)

        if geo_reader: geo_reader.close()

        md = f"# 📊 FLEET TELEMETRY LEDGER | {fleet_id}\n**Engine Version:** 6.4.0 (Typographic Core)\n**Generated:** {datetime.utcnow().strftime('%Y-%m-%dT%H:%M:%SZ')}\n**Standard:** Sovereign Data Protocol (DS-ADR-06)\n\n---\n\n"
        
        t_headers = ["Metric", "Yesterday", "7 Days", "30 Days", "60 Days", "90 Days", "YTD", "Inception"]
        t_r1 = ["Total Network Events", str(t_matrix["Yesterday"]["ev"]), str(t_matrix["7_Days"]["ev"]), str(t_matrix["30_Days"]["ev"]), str(t_matrix["60_Days"]["ev"]), str(t_matrix["90_Days"]["ev"]), str(t_matrix["YTD"]["ev"]), str(t_matrix["Inception"]["ev"])]
        t_r2 = ["Unique Terminals", str(len(t_matrix["Yesterday"]["ip"])), str(len(t_matrix["7_Days"]["ip"])), str(len(t_matrix["30_Days"]["ip"])), str(len(t_matrix["60_Days"]["ip"])), str(len(t_matrix["90_Days"]["ip"])), str(len(t_matrix["YTD"]["ip"])), str(len(t_matrix["Inception"]["ip"]))]
        md += "## ⏱️ TIME MATRIX\n" + format_table(t_headers, [t_r1, t_r2])

        md += generate_matrix("🌍 GLOBAL ROUTING MATRIX", ["Country", "Region / State"], Counter(geo_global_data), total_events)
        md += generate_matrix("🏙️ METRO REGION MATRIX", ["Metro Area"], Counter(geo_metro_data), total_events)
        md += generate_matrix("📡 CONTENT MATRIX (TARGET URI)", ["Routing Target"], Counter(uris_data), total_events)
        md += generate_matrix("📱 DEVICE FORM FACTOR MATRIX", ["Hardware Type"], Counter(devices_data), total_events)
        md += generate_matrix("💻 OPERATING SYSTEM MATRIX", ["Platform"], Counter(systems_data), total_events)
        md += generate_matrix("🖥️ RAW ARCHITECTURE SIGNATURES (TOP 5)", ["Terminal Signature"], Counter(uas_data), total_events, top_n=5)

        with open(out_file, mode='w', encoding='utf-8') as out: out.write(md)
        print(f"[SUCCESS] {fleet_id} V6.4 Typographic Report forged at {out_file}")

except FileNotFoundError: sys.exit(1)
