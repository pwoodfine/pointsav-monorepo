#!/usr/bin/env bash
# run-micro-sandbox.sh — run a command inside a Micro-class cgroup sandbox.
#
# Simulates the $7/mo e2-micro fleet default: 1 GiB RAM, 25% vCPU.
# Uses systemd-run --user so no root required. Primary use: verify that
# service-slm and service-content start and behave correctly under the
# Micro-node resource ceiling (DOCTRINE.md claims #49, #54).
#
# Usage:
#   ./scripts/run-micro-sandbox.sh <command> [args...]
#
# Examples:
#   ./scripts/run-micro-sandbox.sh cargo test --workspace
#   ./scripts/run-micro-sandbox.sh cargo run -p slm-doorman-server
#
# Environment:
#   TOTEBOX_NODE_CLASS=micro is exported automatically so services detect
#   the correct node class without needing real cgroup values.
#
# Requirements:
#   systemd ≥ 235 (--user transient units); cgroup v2 preferred.

set -euo pipefail

if [[ $# -eq 0 ]]; then
    echo "usage: $0 <command> [args...]" >&2
    exit 1
fi

if ! command -v systemd-run &>/dev/null; then
    echo "error: systemd-run not found; cannot create cgroup sandbox" >&2
    exit 1
fi

echo "Launching Micro-class cgroup sandbox (MemoryMax=1G, CPUQuota=25%)..."
echo "Command: $*"
echo ""

exec systemd-run \
    --user \
    --wait \
    --collect \
    --pty \
    -p MemoryMax=1G \
    -p CPUQuota=25% \
    -E TOTEBOX_NODE_CLASS=micro \
    -- "$@"
