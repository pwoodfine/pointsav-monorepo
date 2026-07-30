#!/usr/bin/env python3
"""
© 2026 PointSav Digital Systems AG
Module: SEC EDGAR Mandate Extractor V11.1 (Micro-Batch Deep Scan)
Description: Bypasses the top 10,000 mega-caps to execute a 90-second targeted scan using the Two-Key Regulatory Matrix.
"""

import urllib.request
import urllib.error
import json
import re
import sys
import time

# SEC API Compliance
USER_AGENT = "WoodfineManagementCorp admin@woodfinemanagement.com"
HEADERS = {'User-Agent': USER_AGENT}

# The Matrix Intersection: Real Estate SICs required for general PE funds
REAL_ESTATE_SICS = ["6798", "6500", "6510", "6512", "6513", "6519"]

# Hardcoded Blacklist of Mega-REIT CIKs (from V6 payload)
MEGA_REIT_BLACKLIST = set([
    "0001297996", "0001063761", "0000920522", "0001823945", "0000906345", 
    "0000034903", "0001632970", "0001581068", "0001590717", "0001035443", 
    "0000751364", "0000921825", "0001465128", "0001728951", "0001360604", 
    "0001626115", "0001618563", "0001286043", "0000899689", "0001556593", 
    "0001492298", "0001561894", "0001570827", "0001466085", "0001455863", 
    "0001061630", "0001287865", "0000910108", "0001418121", "0000899629", 
    "0001650132", "0000826675", "0001585389", "0001428205", "0001253986", 
    "0001075415", "0001541401", "0001467760", "0001474098", "0001500217", 
    "0001518621", "0001567925", "0001620393", "0001577670"
])

def fetch_edgar_tickers():
    """Retrieves the master list of CIKs from the SEC."""
    url = "https://www.sec.gov/files/company_tickers.json"
    req = urllib.request.Request(url, headers=HEADERS)
    try:
        with urllib.request.urlopen(req) as response:
            return json.loads(response.read().decode())
    except Exception as e:
        print(f"[ERROR] Failed to fetch master ticker list: {e}")
        sys.exit(1)

def extract_industry_group_from_xml(cik_str, accession_number, primary_doc):
    """
    Downloads raw Form D XML and extracts the <industryGroupType>.
    """
    if not primary_doc.endswith('.xml'):
        return None
        
    cik_clean = cik_str.lstrip('0')
    acc_clean = accession_number.replace('-', '')
    
    url = f"https://www.sec.gov/Archives/edgar/data/{cik_clean}/{acc_clean}/{primary_doc}"
    req = urllib.request.Request(url, headers=HEADERS)
    
    try:
        with urllib.request.urlopen(req) as response:
            xml_data = response.read().decode('utf-8')
            match = re.search(r'<industryGroupType>(.*?)</industryGroupType>', xml_data)
            if match:
                return match.group(1).strip()
        return None
    except Exception:
        return None

def process_funds():
    """Main execution engine using the Regulatory Matrix and Deep Slicing."""
    print("[SYSTEM] Initiating SEC EDGAR Extraction Module V11.1 (Micro-Batch Deep Scan)...")
    tickers_data = fetch_edgar_tickers()
    
    # Convert SEC dictionary to a list so we can slice it
    tickers_list = list(tickers_data.items())
    
    # Define the deep slice boundaries
    start_index = 10000
    end_index = 10500
    
    print(f"[SYSTEM] Bypassing indices 0 through {start_index} (Mega-Cap filtering)...")
    print(f"[SYSTEM] Scanning entities {start_index} to {end_index} for verified Deal Funders...")
    
    target_batch = tickers_list[start_index:end_index]
    valid_funders = []
    processed = 0
    
    try:
        for key, data in target_batch:
            cik = str(data['cik_str']).zfill(10)
            title = data['title']
            processed += 1
            
            # The Terminal Heartbeat (scaled down for the micro-batch)
            if processed % 50 == 0:
                print(f"[SYSTEM] Heartbeat: Scanned {processed}/500 entities in micro-batch...")
                
            if cik in MEGA_REIT_BLACKLIST:
                continue
                
            # Step 1: Fetch SEC Submissions to find SIC and Form D
            submissions_url = f"https://data.sec.gov/submissions/CIK{cik}.json"
            req = urllib.request.Request(submissions_url, headers=HEADERS)
            
            try:
                with urllib.request.urlopen(req) as response:
                    sub_data = json.loads(response.read().decode())
                    sic = str(sub_data.get("sic", ""))
                    filings = sub_data.get("filings", {}).get("recent", {})
                    forms = filings.get("form", [])
                    
                    if "D" in forms:
                        d_index = forms.index("D")
                        accession = filings.get("accessionNumber", [])[d_index]
                        primary_doc = filings.get("primaryDocument", [])[d_index]
                        
                        # Step 2: Extract Industry Group from XML
                        time.sleep(0.15) # SEC Rate limit
                        industry_group = extract_industry_group_from_xml(cik, accession, primary_doc)
                        
                        if not industry_group:
                            continue
                            
                        # Step 3: The Two-Key Validation Logic
                        classification = None
                        
                        # Path A: Direct Real Estate Declaration
                        if industry_group in ["Commercial Real Estate", "REITS and Finance", "Residential Real Estate"]:
                            classification = "Direct Real Estate Funder"
                            
                        # Path B: The Matrix Intersection (PE Fund + RE SIC)
                        elif industry_group in ["Private Equity Fund", "Pooled Investment Fund", "Other Investment Fund"]:
                            if sic in REAL_ESTATE_SICS:
                                classification = "Real Estate PE Funder"
                                
                        # If a match was found, commit to ledger
                        if classification:
                            print(f"[VALIDATED] {title} (CIK: {cik} | Ind: {industry_group} | SIC: {sic})")
                            valid_funders.append({
                                "entity_name": title,
                                "cik": cik,
                                "sic_code": sic,
                                "form_d_industry_group": industry_group,
                                "woodfine_classification": classification,
                                "regulatory_source": "Form D (Item 4)"
                            })
                            
            except urllib.error.HTTPError:
                pass
            except Exception:
                pass
                
            # Strict SEC Rate Limit Compliance (max 10 req/s)
            time.sleep(0.15)
            
    except KeyboardInterrupt:
        print(f"\n[WARNING] Keyboard Interrupt (Ctrl+C) detected. Halting scan at entity {processed}.")
        print("[SYSTEM] Recovering ledger data extracted prior to termination...")
        
    finally:
        output_path = "/home/mathew/Foundry/factory-pointsav/pointsav-monorepo/tool-edgar-extractor/extracted_funds.json"
        with open(output_path, 'w') as f:
            json.dump(valid_funders, f, indent=4)
            
        print(f"[SUCCESS] Extraction finalized. {len(valid_funders)} verified Deal Funders mapped.")
        print(f"[SYSTEM] Asset ledger data written to: {output_path}")

if __name__ == "__main__":
    process_funds()
