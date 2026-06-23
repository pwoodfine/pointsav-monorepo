#!/usr/bin/env bash
# lora-scaled-dropin.sh — Generate (and optionally apply) a systemd drop-in
# that adds --lora-scaled <adapter-path> to the local-slm.service ExecStart.
#
# Usage:
#   lora-scaled-dropin.sh --adapter-path <path> [--apply] [--scale <float>]
#
# Without --apply (default — dry run):
#   Prints the drop-in content to stdout. No files are written.
#   Operator reviews the output before running with --apply.
#
# With --apply:
#   Writes the drop-in to:
#     /etc/systemd/system/local-slm.service.d/zz-lora-scaled.conf
#   using sudo tee, then runs sudo systemctl daemon-reload.
#   Does NOT restart the service — the operator must do that explicitly:
#     sudo systemctl restart local-slm
#
# The drop-in file uses the "clear + set" pattern required by systemd for
# multi-line ExecStart overrides:
#   ExecStart=           <- clears the value set by the base unit or other drop-ins
#   ExecStart=<full cmd> <- sets the new value
#
# The full ExecStart is reconstructed from the effective unit (systemctl
# cat local-slm.service) so it inherits any existing drop-in overrides
# (threads.conf, memory.conf) in the correct override hierarchy order.
# The --lora-scaled flag is appended at the end of the command.
#
# Prerequisite: the adapter GGUF file must exist. PEFT adapters (safetensors
# directories) must be converted first using convert_lora_to_gguf.py from
# the llama.cpp tree:
#   python3 convert_lora_to_gguf.py --base <gguf-model> <adapter-dir>
# This script does NOT run the conversion; it only wires the serving flag.
#
# Exit codes:
#   0  success (dry-run print or apply succeeded)
#   1  argument error or prereq failure
#   2  adapter path invalid

set -euo pipefail

# ── Defaults ──────────────────────────────────────────────────────────────────

ADAPTER_PATH=""
APPLY=0
LORA_SCALE="1.0"
DROPIN_DIR="/etc/systemd/system/local-slm.service.d"
DROPIN_FILE="${DROPIN_DIR}/zz-lora-scaled.conf"
SERVICE_NAME="local-slm.service"

# ── Argument parse ─────────────────────────────────────────────────────────────

while [[ $# -gt 0 ]]; do
    case "$1" in
        --adapter-path=*)  ADAPTER_PATH="${1#--adapter-path=}" ;;
        --adapter-path)    ADAPTER_PATH="$2"; shift ;;
        --apply)           APPLY=1 ;;
        --scale=*)         LORA_SCALE="${1#--scale=}" ;;
        --scale)           LORA_SCALE="$2"; shift ;;
        --help|-h)
            sed -n '2,55p' "$0"
            exit 0
            ;;
        *)
            echo "ERROR: unknown argument: $1" >&2
            echo "Usage: $0 --adapter-path <path> [--apply] [--scale <float>]" >&2
            exit 1 ;;
    esac
    shift
done

if [[ -z "${ADAPTER_PATH}" ]]; then
    echo "ERROR: --adapter-path is required" >&2
    echo "Usage: $0 --adapter-path <path> [--apply] [--scale <float>]" >&2
    exit 1
fi

# ── Adapter validation ─────────────────────────────────────────────────────────

# Adapter path must be either:
#   (a) a .gguf file (converted LoRA adapter)
#   (b) a directory with adapter_config.json (PEFT checkpoint — still needs
#       conversion before llama-server can use it, but we allow the path so
#       the drop-in is written in anticipation of conversion)

if [[ -f "${ADAPTER_PATH}" ]]; then
    # Case (a): .gguf file
    if [[ "${ADAPTER_PATH}" != *.gguf ]]; then
        echo "WARN: adapter-path is a file but does not have a .gguf extension." >&2
        echo "      llama-server --lora-scaled expects a GGUF-format LoRA adapter." >&2
        echo "      Proceeding; verify the file format before restarting the service." >&2
    fi
elif [[ -d "${ADAPTER_PATH}" ]]; then
    # Case (b): PEFT directory
    if [[ ! -f "${ADAPTER_PATH}/adapter_config.json" ]]; then
        echo "ERROR: adapter directory exists but does not contain adapter_config.json" >&2
        echo "       '${ADAPTER_PATH}' does not look like a valid PEFT LoRA checkpoint." >&2
        exit 2
    fi
    echo "WARN: adapter-path is a PEFT directory, not a GGUF file." >&2
    echo "      Convert first using convert_lora_to_gguf.py from the llama.cpp tree:" >&2
    echo "        python3 convert_lora_to_gguf.py --base <base.gguf> ${ADAPTER_PATH}" >&2
    echo "      The drop-in will be written with the directory path; update it after" >&2
    echo "      conversion." >&2
else
    echo "ERROR: adapter-path does not exist: ${ADAPTER_PATH}" >&2
    exit 2
fi

# ── Reconstruct the current effective ExecStart ───────────────────────────────
#
# Use 'systemctl show --property=ExecStart' which returns the single merged
# effective value after all drop-in overrides are applied. This is the only
# reliable approach when multiple drop-ins (memory.conf, threads.conf, etc.)
# use the clear-then-set ExecStart= pattern — parsing systemctl cat output
# across fragments is fragile and fails when overrides chain.
#
# systemctl show output format:
#   ExecStart={ path=/usr/local/bin/llama-server ; argv[]=... ; ... }
# We extract the argv[] value to reconstruct the full command line.

if ! command -v systemctl >/dev/null 2>&1; then
    echo "ERROR: systemctl not found; cannot read current unit definition" >&2
    exit 1
fi

_SHOW_RAW="$(systemctl show "${SERVICE_NAME}" --property=ExecStart 2>/dev/null)"
if [[ -z "${_SHOW_RAW}" ]]; then
    echo "ERROR: could not read ${SERVICE_NAME} via systemctl show" >&2
    exit 1
fi

# Extract binary path and full argv from the structured show output.
# Format: ExecStart={ path=/usr/local/bin/foo ; argv[]=/usr/local/bin/foo --flag val ; ... }
_EXECSTART_FULL="$(echo "${_SHOW_RAW}" | python3 -c "
import sys, re
raw = sys.stdin.read()
m = re.search(r'argv\[\]=([^;]+)', raw)
if m:
    print(m.group(1).strip().rstrip('}').strip())
")"

if [[ -z "${_EXECSTART_FULL}" ]]; then
    echo "ERROR: could not parse ExecStart argv from systemctl show output" >&2
    echo "       Raw output: ${_SHOW_RAW}" >&2
    exit 1
fi

# Strip any existing --lora-scaled or --lora-adapters flags so this script
# is idempotent — applying it twice does not double-add the flag.
_EXECSTART_CLEAN="$(echo "${_EXECSTART_FULL}" \
    | sed 's/--lora-scaled[[:space:]]*[^[:space:]]*//' \
    | sed 's/--lora-adapters[[:space:]]*[^[:space:]]*//' \
    | tr -s ' ')"

# Build the new ExecStart with --lora-scaled appended.
_NEW_EXECSTART="${_EXECSTART_CLEAN} --lora-scaled ${ADAPTER_PATH}"

# ── Build drop-in content ──────────────────────────────────────────────────────
#
# systemd drop-in format:
#   - The [Service] section header is required.
#   - ExecStart= (empty) clears all previous ExecStart values.
#   - The second ExecStart= sets the new value.
# This pattern is required because ExecStart is a list-type directive;
# without the clear, our new value would be ADDED to (not replace) the
# existing list, resulting in two llama-server processes.

_DROPIN_CONTENT="# zz-lora-scaled.conf — generated by lora-scaled-dropin.sh
# DO NOT EDIT MANUALLY — regenerate with:
#   scripts/lora-scaled-dropin.sh --adapter-path ${ADAPTER_PATH}
#
# adapter_path: ${ADAPTER_PATH}
# lora_scale:   ${LORA_SCALE}
# generated:    $(date -u +%Y-%m-%dT%H:%M:%SZ)
#
# To remove the adapter: delete this file and run daemon-reload + restart.
# To update the adapter path: regenerate with the new --adapter-path and --apply.

[Service]
ExecStart=
ExecStart=${_NEW_EXECSTART}"

# ── Output ─────────────────────────────────────────────────────────────────────

echo "=== Drop-in content for ${DROPIN_FILE} ==="
echo ""
echo "${_DROPIN_CONTENT}"
echo ""

if [[ "${APPLY}" -eq 0 ]]; then
    echo "--- DRY RUN (no files written) ---"
    echo ""
    echo "To apply, run:"
    echo "  $0 --adapter-path ${ADAPTER_PATH} --apply"
    echo ""
    echo "Then restart the service:"
    echo "  sudo systemctl restart ${SERVICE_NAME}"
    echo ""
    echo "Verify the adapter is loaded:"
    echo "  curl -s http://127.0.0.1:8080/v1/models"
    echo "  journalctl -u ${SERVICE_NAME} -n 50"
    echo ""
    echo "Run the deploy gate:"
    echo "  scripts/deploy-gate.sh --adapter-path ${ADAPTER_PATH} --probes 20"
    exit 0
fi

# ── Apply ──────────────────────────────────────────────────────────────────────

echo "--- APPLYING drop-in (requires sudo) ---"
echo ""

# Ensure the drop-in directory exists.
if [[ ! -d "${DROPIN_DIR}" ]]; then
    echo "Creating drop-in directory: ${DROPIN_DIR}"
    sudo mkdir -p "${DROPIN_DIR}"
fi

# Write the drop-in file via sudo tee.
echo "${_DROPIN_CONTENT}" | sudo tee "${DROPIN_FILE}" > /dev/null
echo "Written: ${DROPIN_FILE}"

# Reload systemd to pick up the new drop-in.
sudo systemctl daemon-reload
echo "daemon-reload: OK"

echo ""
echo "Drop-in applied. To activate, restart the service:"
echo "  sudo systemctl restart ${SERVICE_NAME}"
echo ""
echo "Verify with:"
echo "  systemctl cat ${SERVICE_NAME} | grep lora-scaled"
echo "  journalctl -u ${SERVICE_NAME} -f"
echo ""
echo "After restart, run the deploy gate to confirm adapter is active:"
echo "  scripts/deploy-gate.sh --adapter-path ${ADAPTER_PATH} --probes 20"
