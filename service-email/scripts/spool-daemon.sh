#!/usr/bin/env bash
set -euo pipefail

if [ "$#" -ne 2 ]; then
    echo "[ERROR] Usage: ./spool-daemon.sh <TOTEBOX_ROOT> <PATH_TO_SOVEREIGN_SPLINTER_BIN>"
    exit 1
fi

TOTEBOX_ROOT="$1"
ENGINE_BIN="$2"

# Strict Leapfrog 2030 Paths (No 'personnel-' prefix)
NEW_DIR="$TOTEBOX_ROOT/service-email/maildir/new"
CUR_DIR="$TOTEBOX_ROOT/service-email/maildir/cur"

# Ensure physical WORM boundaries exist
mkdir -p "$NEW_DIR" "$CUR_DIR"

echo "[SYSTEM] Sovereign Spool Daemon armed. Watching: $NEW_DIR"
echo "--------------------------------------------------------"

# Lightweight continuous polling loop
while true; do
    # Find all .eml files in the new/ directory safely
    for EML_FILE in "$NEW_DIR"/*.eml; do
        if [ -f "$EML_FILE" ]; then
            FILENAME=$(basename "$EML_FILE")
            echo "[DETECTED] Inbound asset: $FILENAME"
            
            # 1. Fire the Rust Sovereign Splinter
            "$ENGINE_BIN" "$EML_FILE" "$TOTEBOX_ROOT"
            
            # 2. Vault the asset (Relocate to cur/)
            mv "$EML_FILE" "$CUR_DIR/"
            echo "[VAULTED] $FILENAME secured in immutable storage."
            echo "--------------------------------------------------------"
        fi
    done
    sleep 5
done
