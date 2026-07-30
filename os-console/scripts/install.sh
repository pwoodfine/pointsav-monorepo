#!/bin/bash
# install.sh — install os-console on macOS 10.13 Intel or Linux x86_64
#
# What this script does:
#   1. Downloads the pre-built binary from software.pointsav.com
#   2. Writes ~/.config/os-console/config.toml
#   3. Creates a desktop launcher (double-click to open)
#
# Run with:
#   curl -fsSL https://software.pointsav.com/os-console/install.sh | bash
#   — or —
#   bash install.sh
#
# No Rust, no compiler, no git, no SSH key required.

set -e

SOURCE_URL="https://software.pointsav.com/releases/os-console/latest"

echo "=== os-console install ==="
echo ""

# ── Step 1: Detect platform ─────────────────────────────────────────────────

case "$(uname -s)-$(uname -m)" in
  Linux-x86_64)   PLATFORM="linux-x86_64" ;;
  Darwin-x86_64)  PLATFORM="darwin-x86_64" ;;
  Darwin-arm64)   PLATFORM="darwin-arm64" ;;
  *)
    echo "ERROR: Unsupported platform: $(uname -s) $(uname -m)"
    exit 1
    ;;
esac

echo "Platform: $PLATFORM"

# ── Step 2: Download binary ─────────────────────────────────────────────────

mkdir -p "$HOME/bin"

echo "Downloading os-console from software.pointsav.com..."
curl -fsSL --location \
    "$SOURCE_URL/$PLATFORM" \
    -o "$HOME/bin/os-console"
chmod +x "$HOME/bin/os-console"
echo "Installed: $HOME/bin/os-console"

# ── Step 3: Detect SSH key for embedded tunnel ──────────────────────────────

GCE_IP="34.53.65.203"
GCE_USER="mathew"
SSH_KEY=""

for CANDIDATE in \
    "$HOME/.ssh/google_compute_engine" \
    "$HOME/.ssh/id_ed25519" \
    "$HOME/.ssh/id_rsa" \
    "$HOME/.ssh/id_rsa_foundry"
do
    if [ -f "$CANDIDATE" ]; then
        if ssh -i "$CANDIDATE" \
               -o BatchMode=yes \
               -o ConnectTimeout=8 \
               -o StrictHostKeyChecking=no \
               "$GCE_USER@$GCE_IP" true 2>/dev/null; then
            SSH_KEY="$CANDIDATE"
            echo "SSH key: $SSH_KEY"
            break
        fi
    fi
done

if [ -z "$SSH_KEY" ]; then
    echo ""
    echo "WARNING: No SSH key found that can reach the build server ($GCE_IP)."
    echo "The binary was installed but the embedded tunnel will not connect."
    echo ""
    echo "Ask the system administrator to authorize your public key."
    echo "If you don't have a key yet, run:"
    echo "  ssh-keygen -t ed25519 -f ~/.ssh/id_ed25519 -N ''"
    echo "Then share:  cat ~/.ssh/id_ed25519.pub"
    SSH_KEY="$HOME/.ssh/id_ed25519"
fi

# ── Step 4: config.toml ────────────────────────────────────────────────────

CONFIG_DIR="$HOME/.config/os-console"
mkdir -p "$CONFIG_DIR"

cat > "$CONFIG_DIR/config.toml" << EOF
[profile]
username = "jennifer"
tenant   = "woodfine"

totebox_host     = "127.0.0.1"
totebox_ssh_port = 2222
ingest_endpoint  = "http://127.0.0.1:9100"

# Embedded SSH tunnel — binary connects to gce_host at startup
gce_host     = "$GCE_IP"
gce_user     = "$GCE_USER"
ssh_key_path = "$SSH_KEY"
EOF

echo "Config: $CONFIG_DIR/config.toml"

# ── Step 5: Desktop launcher ────────────────────────────────────────────────

case "$(uname -s)" in
  Linux)
    DESKTOP_FILE="$HOME/Desktop/OS Console.desktop"
    cat > "$DESKTOP_FILE" << 'DEOF'
[Desktop Entry]
Name=OS Console
Comment=Totebox Console
Exec=bash -c 'exec ~/bin/os-console'
Terminal=true
Type=Application
Icon=utilities-terminal
StartupNotify=false
DEOF
    chmod +x "$DESKTOP_FILE"
    echo "Launcher: $DESKTOP_FILE"
    echo "Right-click → 'Allow Launching' once, then double-click."
    ;;

  Darwin)
    LAUNCHER="$HOME/Desktop/OS Console.command"
    cat > "$LAUNCHER" << 'DEOF'
#!/bin/bash
exec ~/bin/os-console
DEOF
    chmod +x "$LAUNCHER"
    echo "Launcher: $LAUNCHER"
    echo "First open: right-click → Open → Open anyway."
    ;;
esac

# ── Done ────────────────────────────────────────────────────────────────────

echo ""
echo "=== Done ==="
echo ""
echo "Double-click 'OS Console' on your desktop."
echo ""
echo "To update:  curl -fsSL https://software.pointsav.com/releases/os-console/install.sh | bash"
