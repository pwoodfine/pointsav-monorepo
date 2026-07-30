#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")"

echo "========================================================"
echo " 🧠 IGNITING THE TEACHER MODEL (QWEN2.5-CODER-1.5B)"
echo "========================================================"

# 1. Boot the model in the background
# We strictly limit context (-c 2048) and threads (-t 2) to protect the iMac's memory boundaries
./engine/llamafile -m ./engine/weights/qwen2.5-coder-1.5b.gguf --server --host 127.0.0.1 --port 8080 --nobrowser -c 2048 -t 2 > ./engine/engine.log 2>&1 &
ENGINE_PID=$!

echo "[SYSTEM] Allocating model weights into physical memory..."
# Wait for the API to come online
until curl -s http://127.0.0.1:8080/health > /dev/null; do
    sleep 2
done
echo "[SUCCESS] Cognitive API is active."

# 2. Execute the Distillation Python Script
cd src
python3 distill_knowledge.py

# 3. Kill the engine to free up RAM immediately
echo "[SYSTEM] Terminating Teacher Model to release memory boundaries..."
kill -9 $ENGINE_PID || true

echo "========================================================"
echo "[SUCCESS] The Forge has cooled. System RAM restored."
