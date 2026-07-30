#!/usr/bin/env bash
# Print recent workbench activity logs for Claude to analyze.
# Usage: ./analyze-activity.sh [N_files]   default: last 3 log files
LOG_DIR="/srv/foundry/infrastructure/local-workbench/logs"
N="${1:-3}"
files=$(ls -t "$LOG_DIR"/activity-*.jsonl 2>/dev/null | head -n "$N")
if [ -z "$files" ]; then echo "No activity logs found in $LOG_DIR"; exit 1; fi
echo "=== Workbench Activity Log (last $N day-files) ==="
for f in $(echo "$files" | tac); do
  echo ""
  echo "--- $f ---"
  cat "$f"
done
