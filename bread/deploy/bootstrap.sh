#!/usr/bin/env bash
# bootstrap.sh — install Sourdough Tracker as a system service
# Run once from Command Session: sudo bash bread/deploy/bootstrap.sh
# Idempotent: safe to re-run.

set -euo pipefail
BREAD_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
WWW="/var/lib/local-bread/www"
DATA_DIR="/var/lib/local-bread"
VENV="$DATA_DIR/venv"

echo "==> Source dir: $BREAD_DIR"

# 1. Create service user
if ! id local-bread &>/dev/null; then
  useradd -r -m -d "$DATA_DIR" -s /usr/sbin/nologin local-bread
  echo "    created user local-bread"
else
  echo "    user local-bread already exists"
fi

# 2. Create directories
mkdir -p "$WWW" "$DATA_DIR"
chown -R local-bread:local-bread "$DATA_DIR"

# 3. Copy server files
cp "$BREAD_DIR/server.py"       "$DATA_DIR/server.py"
cp "$BREAD_DIR/requirements.txt" "$DATA_DIR/requirements.txt"

# 4. Copy PWA frontend
cp "$BREAD_DIR/tracker.html"  "$WWW/index.html"
cp "$BREAD_DIR/manifest.json" "$WWW/manifest.json"
cp "$BREAD_DIR/sw.js"         "$WWW/sw.js"
cp "$BREAD_DIR/icon-192.png"  "$WWW/icon-192.png"
cp "$BREAD_DIR/icon-512.png"  "$WWW/icon-512.png"

chown -R local-bread:local-bread "$DATA_DIR"

# 5. Python virtualenv + packages
if [ ! -f "$VENV/bin/uvicorn" ]; then
  python3 -m venv "$VENV"
  "$VENV/bin/pip" install --quiet -r "$DATA_DIR/requirements.txt"
  echo "    installed Python packages"
else
  "$VENV/bin/pip" install --quiet -r "$DATA_DIR/requirements.txt"
  echo "    Python packages up to date"
fi

# 6. Initialise data file if missing
if [ ! -f "$DATA_DIR/loaves.json" ]; then
  echo '[]' > "$DATA_DIR/loaves.json"
  chown local-bread:local-bread "$DATA_DIR/loaves.json"
  echo "    created empty loaves.json"
fi

# 7. systemd service
cp "$BREAD_DIR/deploy/local-bread.service" /etc/systemd/system/local-bread.service
systemctl daemon-reload
systemctl enable local-bread
systemctl restart local-bread
echo "    service started: $(systemctl is-active local-bread)"

# 8. nginx vhost
cp "$BREAD_DIR/deploy/nginx-bread.conf" /etc/nginx/sites-available/foodservice.woodfinegroup.com
ln -sf /etc/nginx/sites-available/foodservice.woodfinegroup.com \
        /etc/nginx/sites-enabled/foodservice.woodfinegroup.com

# 9. Test + reload nginx
nginx -t
systemctl reload nginx
echo ""
echo "==> Done. Next step (TLS):"
echo "    sudo certbot --nginx -d foodservice.woodfinegroup.com --non-interactive --agree-tos -m open.source@pointsav.com --redirect"
echo ""
echo "==> Service status:"
systemctl status local-bread --no-pager -l | head -20
