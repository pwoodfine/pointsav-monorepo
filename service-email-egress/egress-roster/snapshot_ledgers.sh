#!/bin/bash
set -euo pipefail

TIMESTAMP=$(date +"%Y-%m-%d_%H%M%S")
LEDGER_DIR="../data-ledgers"
SNAPSHOT_DIR="$LEDGER_DIR/snapshots"

mkdir -p "$SNAPSHOT_DIR"
echo "  -> Securing Immutable Snapshot: $TIMESTAMP..."

if [ -f "$LEDGER_DIR/master_metadata_ledger.jsonl" ]; then
    cp "$LEDGER_DIR/master_metadata_ledger.jsonl" "$SNAPSHOT_DIR/master_metadata_ledger_$TIMESTAMP.jsonl"
fi
if [ -f "$LEDGER_DIR/crm_contacts.csv" ]; then
    cp "$LEDGER_DIR/crm_contacts.csv" "$SNAPSHOT_DIR/crm_contacts_$TIMESTAMP.csv"
fi
