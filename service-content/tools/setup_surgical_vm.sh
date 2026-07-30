#!/usr/bin/env bash
# Purpose: Prepare environment and reconfigure GCP repos to pull ONLY logic/DNA.
NODE_ROOT="$HOME/node-gcp-free/factory-pointsav"
mkdir -p "$NODE_ROOT"
echo "🔬 INITIATING SURGICAL ENVIRONMENT PREP..."

# 1. Install System Prerequisites
sudo apt-get update -y
sudo apt-get install -y git curl build-essential pkg-config libssl-dev

# 2. Install Rust/Cargo if missing
if ! command -v cargo &> /dev/null; then
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
    source "$HOME/.cargo/env"
fi
source "$HOME/.cargo/env" 2>/dev/null || true

cd "$NODE_ROOT"

# 3. Sparse Pull: DNA
if [ ! -d "pointsav-design-system/.git" ]; then
    git clone --filter=blob:none --no-checkout https://github.com/pointsav/pointsav-design-system.git
    cd pointsav-design-system
    git sparse-checkout init --cone
    git sparse-checkout set tokens/linguistic
    git checkout main
    cd ..
fi

# 4. Sparse Pull: Muscle (Workspace-Aware)
if [ ! -d "pointsav-monorepo/.git" ]; then
    git clone --filter=blob:none --no-checkout https://github.com/pointsav/pointsav-monorepo.git
    cd pointsav-monorepo
    git sparse-checkout init --cone
    git sparse-checkout set service-content system-security system-audit system-resolution system-verification os-infrastructure system-network-interface Cargo.toml
    git checkout main
    cd ..
fi

# 5. Build
echo "⚙️  Building Engine..."
cd "$NODE_ROOT/pointsav-monorepo/service-content"
mkdir -p input outbox
cargo build --release -p service-content
chmod +x tools/trigger_extraction.sh
echo "✅ SURGICAL NODE READY."
