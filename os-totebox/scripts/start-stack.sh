#!/bin/bash
# Start the os-totebox service stack in dependency order.
# Run inside an os-totebox VM after binaries are installed.
#
# Env overrides:
#   TOTEBOX_BIN_DIR      — directory containing installed binaries (default: /usr/local/bin)
#   TOTEBOX_DATA_DIR     — data root passed to services (default: /var/lib/pointsav)
#   TOTEBOX_LOG_DIR      — log directory (default: /var/log/pointsav)
#   TOTEBOX_PID_DIR      — PID file directory (default: /run/pointsav)
#   TOTEBOX_ARCHIVE      — deployment archive name (default: cluster-totebox-local)
#   TOTEBOX_MODULE_ID    — module identifier passed to services (default: local)
#
# For dev on the workspace host, set TOTEBOX_BIN_DIR to the cargo target:
#   TOTEBOX_BIN_DIR=$(cargo build --release -q 2>&1; echo target/release) \
#     ./os-totebox/scripts/start-stack.sh

set -euo pipefail

BIN_DIR="${TOTEBOX_BIN_DIR:-/usr/local/bin}"
DATA_DIR="${TOTEBOX_DATA_DIR:-/var/lib/pointsav}"
LOG_DIR="${TOTEBOX_LOG_DIR:-/var/log/pointsav}"
PID_DIR="${TOTEBOX_PID_DIR:-/run/pointsav}"
TOTEBOX_ARCHIVE="${TOTEBOX_ARCHIVE:-cluster-totebox-local}"
TOTEBOX_MODULE_ID="${TOTEBOX_MODULE_ID:-local}"

HEALTH_TIMEOUT=15   # seconds to wait for each HTTP service
HEALTH_INTERVAL=1   # poll interval in seconds

mkdir -p "$LOG_DIR" "$PID_DIR"

log() { echo "[start-stack] $*"; }
die() { echo "[start-stack] FATAL: $*" >&2; exit 1; }

wait_http() {
    local name="$1" url="$2"
    local elapsed=0
    log "  waiting for $name at $url ..."
    while ! curl -sf "$url" >/dev/null 2>&1; do
        sleep "$HEALTH_INTERVAL"
        elapsed=$(( elapsed + HEALTH_INTERVAL ))
        if [ "$elapsed" -ge "$HEALTH_TIMEOUT" ]; then
            die "$name did not become healthy within ${HEALTH_TIMEOUT}s"
        fi
    done
    log "  $name ready"
}

start_svc() {
    local name="$1"; shift
    local pid_file="$PID_DIR/$name.pid"
    if [ -f "$pid_file" ] && kill -0 "$(cat "$pid_file")" 2>/dev/null; then
        log "$name already running (pid $(cat "$pid_file"))"
        return 0
    fi
    log "starting $name ..."
    "$@" >> "$LOG_DIR/$name.log" 2>&1 &
    echo $! > "$pid_file"
    log "$name started (pid $!)"
}

log "============================================================"
log " os-totebox service stack — startup"
log "============================================================"

# ── Tier 0: verify binaries present ──────────────────────────────────────────
for bin in slm-doorman-server service-content server service-extraction service-fs service-input; do
    [ -x "$BIN_DIR/$bin" ] || die "binary not found: $BIN_DIR/$bin"
done
log "all binaries found"

# ── Tier 1: service-slm (Doorman — inference gateway) ────────────────────────
# Must start before service-content (content uses Doorman for Tier C drafts).
start_svc service-slm \
    env SLM_BIND_ADDR="127.0.0.1:9080" \
        SLM_LOCAL_ENDPOINT="http://127.0.0.1:8080" \
        SLM_MODULE_ID="data" \
        "$BIN_DIR/slm-doorman-server"
wait_http service-slm "http://127.0.0.1:9080/health"

# ── Tier 2: service-content (DataGraph / LadybugDB) ──────────────────────────
start_svc service-content \
    env SERVICE_CONTENT_HTTP_BIND="127.0.0.1:9081" \
        SERVICE_CONTENT_BASE_DIR="$DATA_DIR/$TOTEBOX_ARCHIVE" \
        "$BIN_DIR/service-content"
wait_http service-content "http://127.0.0.1:9081/health"

# ── Tier 3: service-people (personnel ledger HTTP API) ───────────────────────
start_svc service-people \
    env SERVICE_PEOPLE_PORT="9091" \
        SERVICE_PEOPLE_LEDGER_PATH="$DATA_DIR/service-people/ledger_personnel.json" \
        "$BIN_DIR/server"
wait_http service-people "http://127.0.0.1:9091/v1/people"

# ── Tier 0.5: service-fs (WORM storage gatekeeper — Envelope A std/axum) ─────
start_svc service-fs \
    env FS_BIND_ADDR="127.0.0.1:9100" \
        FS_MODULE_ID="${TOTEBOX_MODULE_ID}" \
        FS_LEDGER_ROOT="$DATA_DIR/$TOTEBOX_ARCHIVE/service-fs/worm" \
        FS_WATCH_DROP_DIR="$DATA_DIR/$TOTEBOX_ARCHIVE/service-extraction/watch" \
        "$BIN_DIR/service-fs"
wait_http service-fs "http://127.0.0.1:9100/healthz"

# ── Tier 4: service-extraction (filesystem watcher — no HTTP surface) ────────
# service-extraction tails the watch dir dropped by service-fs and writes CORPUS
# files to service-content/ledgers/ for DataGraph ingestion.
# No health endpoint; process start is the signal.
start_svc service-extraction \
    env EXTRACTION_BASE_DIR="$DATA_DIR" \
        EXTRACTION_WATCH_DIR="$DATA_DIR/$TOTEBOX_ARCHIVE/service-extraction/watch" \
        EXTRACTION_EMIT_CORPUS_DIR="$DATA_DIR/$TOTEBOX_ARCHIVE/service-content/ledgers" \
        EXTRACTION_CORPUS_MODULE_ID="${TOTEBOX_MODULE_ID}" \
        "$BIN_DIR/service-extraction"
sleep 1
pid_file="$PID_DIR/service-extraction.pid"
kill -0 "$(cat "$pid_file")" 2>/dev/null \
    || die "service-extraction exited immediately; check $LOG_DIR/service-extraction.log"
log "service-extraction running"

# ── Tier 5: service-input (Input Machine — file ingest, migration, calibration)
start_svc service-input \
    env SERVICE_INPUT_BIND="127.0.0.1:9106" \
        SERVICE_INPUT_MODULE_ID="${TOTEBOX_MODULE_ID}" \
        SERVICE_INPUT_FS_ENDPOINT="http://127.0.0.1:9100" \
        SERVICE_INPUT_DEST_ARCHIVE="${TOTEBOX_ARCHIVE}" \
        SERVICE_INPUT_LEDGER="$DATA_DIR/$TOTEBOX_ARCHIVE/service-input/ledger.jsonl" \
        "$BIN_DIR/service-input"
wait_http service-input "http://127.0.0.1:9106/healthz"

log "============================================================"
log " stack ready  (archive: ${TOTEBOX_ARCHIVE}, module: ${TOTEBOX_MODULE_ID})"
log "   service-slm         http://127.0.0.1:9080   (Doorman — inference gateway)"
log "   service-content     http://127.0.0.1:9081   (DataGraph / LadybugDB)"
log "   service-people      http://127.0.0.1:9091   (personnel ledger)"
log "   service-extraction  (background watcher — CORPUS emitter)"
log "   service-fs          http://127.0.0.1:9100   (WORM storage — Envelope A)"
log "   service-input       http://127.0.0.1:9106   (Input Machine — migration / calibration)"
log "PID files: $PID_DIR/"
log "Logs:      $LOG_DIR/"
log "============================================================"
