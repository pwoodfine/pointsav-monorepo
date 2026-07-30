#!/bin/bash
# launch-console.sh — Linux Mint launcher for os-console
#
# Starts the GCE VM service tunnel if not running, opens os-console,
# then kills the tunnel when the console exits.
#
# Usage: ./launch-console.sh
# Desktop: set this as the Exec= target in OS Console.desktop
#
# Requires: ~/.ssh/config entry "foundry-services" with LocalForward lines
#           Run install.sh once to configure the SSH entry and desktop launcher.

set -e

BINARY="$HOME/bin/os-console"

if [ ! -x "$BINARY" ]; then
    echo "os-console binary not found at $BINARY"
    echo "Run os-console/scripts/install.sh first."
    exit 1
fi

TUNNEL_PID=""
if ! pgrep -f "ssh -N foundry-services" > /dev/null 2>&1; then
    ssh -N foundry-services &
    TUNNEL_PID=$!
    sleep 2
fi

"$BINARY"

if [ -n "$TUNNEL_PID" ]; then
    kill "$TUNNEL_PID" 2>/dev/null || true
fi
