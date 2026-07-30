#!/bin/bash
# PointSav Digital Systems | Generic Strict Pull Diode Template
set -euo pipefail

REMOTE_TARGET="<TARGET_IP>"
REMOTE_USER="<TARGET_USER>"
TODAY=$(date +%Y-%m-%d)
LOCAL_PATH="<LOCAL_OUTBOX_PATH>"
REMOTE_PATH="<REMOTE_OUTBOX_PATH>"
CSV_SOURCE="<REMOTE_CSV_PATH>"
PREFIX="<FILE_PREFIX>"

mkdir -p "${LOCAL_PATH}/outbox"
rsync -avz "${REMOTE_USER}@${REMOTE_TARGET}:${REMOTE_PATH}/" "${LOCAL_PATH}/outbox/"
rsync -avz "${REMOTE_USER}@${REMOTE_TARGET}:${CSV_SOURCE}" "${LOCAL_PATH}/outbox/${PREFIX}_${TODAY}.csv"

find "${LOCAL_PATH}/outbox" -name "${PREFIX}_*.csv" -type f -mtime +9 -exec rm {} \;
