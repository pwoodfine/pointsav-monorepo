#!/bin/bash
TARGET_DIR="/opt/deployments/pointsav-fleet-deployment/media-marketing-landing"
DATE_STAMP=$(date +"%Y%m%d")

if [ -f "$TARGET_DIR/assets/ledger_telemetry.csv" ]; then
    mv $TARGET_DIR/assets/ledger_telemetry.csv $TARGET_DIR/assets/ledger_telemetry_$DATE_STAMP.csv
fi

systemctl restart pointsav-telemetry
cd $TARGET_DIR && ./telemetry-synthesizer

find $TARGET_DIR/assets/ -name "ledger_telemetry_*.csv" -type f -mtime +365 -delete
find $TARGET_DIR/outbox/ -name "REPORT_TELEMETRY_*.md" -type f -mtime +30 -delete
