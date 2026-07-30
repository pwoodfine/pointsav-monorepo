#!/usr/bin/env bash
# Purpose: Setup Desktop Hot-Zone and symlink the pointsav-input command.
echo "🏛️  POINTSAV CONSOLE: INSTALLING..."
SCRIPT_SRC="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" &> /dev/null && pwd)"
mkdir -p ~/Desktop/service-content/{input/processed,output,logs}
sudo ln -sf "${SCRIPT_SRC}/app-console-input.sh" /usr/local/bin/pointsav-input
echo "✅ SUCCESS: 'pointsav-input' command installed."
