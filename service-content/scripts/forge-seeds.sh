#!/usr/bin/env bash
# Generates the JSON Seed Vault using live Woodfine/PointSav definitions

# Resolve target directory from env var or default to adjacent seeds/ directory
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
TARGET_DIR="${SERVICE_CONTENT_SEEDS_DIR:-${SCRIPT_DIR}/../seeds}"
mkdir -p "$TARGET_DIR"

cat << 'EOF' > "$TARGET_DIR/Archetypes.json"
{
  "archetypes": [
    { "id": "1", "name": "The Executive", "signature": "Strategic Direction", "gravity_keywords": ["Strategic Direction", "Stagnation", "Executive"] },
    { "id": "2", "name": "The Guardian", "signature": "Risk & Compliance", "gravity_keywords": ["Risk & Compliance", "Breach", "Guardian"] },
    { "id": "3", "name": "The Fiduciary", "signature": "Resource Integrity", "gravity_keywords": ["Resource Integrity", "Leakage", "Fiduciary"] },
    { "id": "4", "name": "The Architect", "signature": "System Design", "gravity_keywords": ["System Design", "Complexity", "Architect"] },
    { "id": "7", "name": "The Constructor", "signature": "Physical Realization", "gravity_keywords": ["Physical Realization", "Structural Gap", "Constructor"] }
  ]
}
EOF

cat << 'EOF' > "$TARGET_DIR/ChartOfAccounts.json"
{
  "chart_of_accounts": [
    { "id": "1", "profile": "Compliance", "sub_domain": "Legal Representation", "gravity_keywords": ["Compliance", "Counsel", "Legal Representation"] },
    { "id": "5", "profile": "Real Estate", "sub_domain": "Leasing", "gravity_keywords": ["Real Estate", "Leasing", "Office", "Industrial", "Retail"] },
    { "id": "9", "profile": "Construction", "sub_domain": "Control Architect", "gravity_keywords": ["Construction", "Collaborators", "Control Architect"] },
    { "id": "11", "profile": "Investor Relations", "sub_domain": "Portfolio Managers", "gravity_keywords": ["Investor Relations", "Finance", "Portfolio Managers"] }
  ]
}
EOF

cat << 'EOF' > "$TARGET_DIR/Domains.json"
{
  "domains": [
    {
      "category": "Corporate",
      "gravity_keywords": ["Qualified Investment", "Direct-Hold Solutions", "Perpetual Equity", "Flow-Through Taxation", "Institutional-Grade"]
    },
    {
      "category": "Projects",
      "gravity_keywords": ["Fixed Floor Plate", "Co-location", "Geometry of Sustainability", "Woodfine Professional Centres", "Vertical Warehouses"]
    },
    {
      "category": "Documentation",
      "gravity_keywords": ["PointSav Digital Systems", "Totebox Archive", "Sovereign Telemetry", "Capability-Based Manager", "Microkernel"]
    }
  ]
}
EOF

cat << 'EOF' > "$TARGET_DIR/Themes.json"
{
  "themes": [
    { "id": "THM-01", "name": "Co-Location Mandate Expansion", "gravity_keywords": ["Co-Location", "National Retailers", "Walmart", "Costco", "Regional Markets"] },
    { "id": "THM-02", "name": "Flow-Through Taxation Structuring", "gravity_keywords": ["Flow-Through", "Taxation", "Regulated Reporting Entities"] },
    { "id": "THM-03", "name": "Broadcom Driver Migration", "gravity_keywords": ["Broadcom", "Driver Migration", "FreeBSD", "Legacy Silicon"] },
    { "id": "THM-04", "name": "Q3 Capital Procurement", "gravity_keywords": ["Capital Procurement", "Narrow Bank Model", "Interest Coverage Ratio"] }
  ]
}
EOF

echo "[SUCCESS] Woodfine Seed Vault Forged in JSON."
