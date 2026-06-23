#!/usr/bin/env bash
# deploy-gate.sh — Phase D deploy gate: base-vs-adapter output delta probe.
#
# GAP-4 / D3 remediation: proves the adapter is NOT a no-op before any
# service restart that loads it via --lora-scaled.
#
# Approach (two-run protocol):
#   llama-server exposes a runtime hot-swap endpoint at POST /lora-adapters
#   (llama.cpp; NOT a Doorman endpoint). We use this to toggle the adapter in
#   and out on the SAME running instance rather than spinning up two servers.
#   Protocol:
#     Run 1 (baseline)  — clear any loaded adapter, collect N outputs.
#     Run 2 (adapted)   — POST /lora-adapters to load the adapter, collect
#                         the same N outputs, then clear again.
#   Delta: for each probe pair, compare outputs. "Non-trivial" = the two
#   outputs differ after stripping leading/trailing whitespace.
#
# Single-GPU fallback:
#   If POST /lora-adapters returns 404 (llama.cpp build without dynamic-lora
#   support, or the endpoint is gated by compile flag), we fall back to a
#   two-pass sequential approach that requires the operator to have run the
#   script ONCE with SLM_BASELINE_FILE pre-populated (export mode), and once
#   more after restarting with --lora-scaled (compare mode).
#   In that case, exit code 2 with a clear OPERATOR REQUIRED message.
#
# Requirements:
#   - llama-server running and reachable at SLM_LOCAL_ENDPOINT (default
#     http://127.0.0.1:8080)
#   - curl, python3 (stdlib only), jq (optional — falls back to python3)
#   - /srv/foundry/data/adapters/ writable by this user
#
# Usage:
#   deploy-gate.sh --adapter-path <path> [--base-model <path>]
#                  [--probes <N>] [--endpoint <url>] [--dry-run]
#
# Exit codes:
#   0   PASS  — adapter produces non-trivial delta on >= 15/20 probes
#   1   FAIL  — adapter is a no-op (null delta on >= 6 probes) or prereq error
#   2   DEFER — dynamic lora endpoint unavailable; operator two-run required
#   3   Error — missing required argument or unreachable endpoint

set -uo pipefail

# ── Defaults ─────────────────────────────────────────────────────────────────

FOUNDRY_ROOT="${FOUNDRY_ROOT:-/srv/foundry}"
ADAPTER_PATH=""
BASE_MODEL="${FOUNDRY_ROOT}/data/adapters"   # informational; not used directly
PROBES=20
ENDPOINT="${SLM_LOCAL_ENDPOINT:-http://127.0.0.1:8080}"
DRY_RUN=0
RESULT_FILE="${FOUNDRY_ROOT}/data/adapters/deploy-gate-result.json"
PASS_THRESHOLD=15        # out of PROBES
FAIL_THRESHOLD=6         # null-delta count triggers FAIL (>=)
PROBE_MAX_TOKENS=128
PROBE_TEMPERATURE="0.0"  # greedy — deterministic outputs required for comparison
LORA_SCALE="1.0"

# ── Probe prompts (20 diverse short prompts for a coding-domain base model) ──
# Fixed set ensures reproducibility across runs. Prompts are intentionally
# simple so the base model always produces something; the adapter should steer
# the output noticeably for coding/diff tasks.

PROBE_PROMPTS=(
    "Write a Rust function that returns the sum of a Vec<i32>."
    "Explain what a LoRA adapter does in one sentence."
    "Show a minimal Python HTTP server using http.server."
    "What is the output of: println!(\"{}\", 1 + 1);"
    "Write a bash one-liner to count lines in a file."
    "Describe the difference between SFT and DPO fine-tuning."
    "Show a git diff header for a renamed file."
    "Write a Rust match statement on an Option<String>."
    "What does --no-repack do in llama-server?"
    "Show a systemd drop-in that sets an environment variable."
    "Write a curl command that POSTs JSON to localhost:8080."
    "What is the purpose of a LoRA rank parameter?"
    "Show a Cargo.toml workspace member declaration."
    "Explain gradient checkpointing in one sentence."
    "Write a Python function that reads lines from a JSONL file."
    "What is the OLMo model architecture?"
    "Show a Rust struct with serde Serialize and Deserialize."
    "Write a bash function that logs with a timestamp prefix."
    "What does ctx-size control in llama.cpp?"
    "Show a minimal axum GET handler that returns plain text."
)

# ── Argument parse ────────────────────────────────────────────────────────────

while [[ $# -gt 0 ]]; do
    case "$1" in
        --adapter-path=*)  ADAPTER_PATH="${1#--adapter-path=}" ;;
        --adapter-path)    ADAPTER_PATH="$2"; shift ;;
        --base-model=*)    BASE_MODEL="${1#--base-model=}" ;;
        --base-model)      BASE_MODEL="$2"; shift ;;
        --probes=*)        PROBES="${1#--probes=}" ;;
        --probes)          PROBES="$2"; shift ;;
        --endpoint=*)      ENDPOINT="${1#--endpoint=}" ;;
        --endpoint)        ENDPOINT="$2"; shift ;;
        --dry-run)         DRY_RUN=1 ;;
        --help|-h)
            sed -n '2,50p' "$0"
            exit 0
            ;;
        *)
            echo "ERROR: unknown argument: $1" >&2; exit 3 ;;
    esac
    shift
done

if [[ -z "${ADAPTER_PATH}" ]]; then
    echo "ERROR: --adapter-path is required" >&2
    echo "Usage: $0 --adapter-path <path> [--probes N]" >&2
    exit 3
fi

# ── Prereq checks ─────────────────────────────────────────────────────────────

log() { printf '[deploy-gate %s] %s\n' "$(date -u +'%Y-%m-%dT%H:%M:%SZ')" "$*"; }

log "deploy-gate.sh starting"
log "  adapter_path:  ${ADAPTER_PATH}"
log "  probes:        ${PROBES}"
log "  endpoint:      ${ENDPOINT}"
log "  result_file:   ${RESULT_FILE}"

# Validate adapter directory.
if [[ ! -d "${ADAPTER_PATH}" ]]; then
    log "ERROR: adapter directory not found: ${ADAPTER_PATH}"
    exit 3
fi
if [[ ! -f "${ADAPTER_PATH}/adapter_config.json" ]]; then
    log "ERROR: adapter_config.json not found in ${ADAPTER_PATH}"
    log "       This is not a valid PEFT LoRA checkpoint."
    exit 3
fi

# Validate llama-server is reachable.
if ! curl -sS --connect-timeout 5 "${ENDPOINT}/health" >/dev/null 2>&1; then
    log "ERROR: llama-server not reachable at ${ENDPOINT}/health"
    log "       Ensure local-slm.service is running: systemctl status local-slm"
    exit 3
fi
log "llama-server reachable at ${ENDPOINT}"

# Ensure result directory exists.
mkdir -p "$(dirname "${RESULT_FILE}")"

# python3 for JSON output construction (stdlib only).
if ! command -v python3 >/dev/null 2>&1; then
    log "ERROR: python3 required (stdlib only)"
    exit 3
fi

# ── Dynamic LoRA endpoint probe ───────────────────────────────────────────────
# llama.cpp exposes POST /lora-adapters to hot-swap adapters at runtime.
# Test whether this endpoint is available before choosing the protocol.
#
# Note: we call with an empty list to clear any existing adapters first.
# A 200 or 204 means the endpoint is available. 404 means not compiled in.

log "probing /lora-adapters endpoint availability..."
_LORA_CLEAR_STATUS=$(curl -sS -o /dev/null -w "%{http_code}" \
    --connect-timeout 5 \
    -X POST "${ENDPOINT}/lora-adapters" \
    -H "Content-Type: application/json" \
    -d '[]' 2>/dev/null || echo "000")

log "  /lora-adapters clear status: ${_LORA_CLEAR_STATUS}"

if [[ "${_LORA_CLEAR_STATUS}" == "404" || "${_LORA_CLEAR_STATUS}" == "000" ]]; then
    # Dynamic lora hot-swap not available.
    # Fall back to two-run sequential protocol.
    log ""
    log "OPERATOR REQUIRED — two-run sequential protocol:"
    log ""
    log "  The running llama-server binary does not support the dynamic"
    log "  /lora-adapters endpoint (compiled without LLAMA_SERVER_LORA_HOTSWAP,"
    log "  or using a llama.cpp build before PR #7634)."
    log ""
    log "  To complete Phase D validation, run this script TWICE:"
    log ""
    log "  Run 1 (baseline — no adapter):"
    log "    SLM_GATE_MODE=baseline $0 --adapter-path ${ADAPTER_PATH} --probes ${PROBES}"
    log "    This saves outputs to: ${RESULT_FILE%.json}-baseline.json"
    log ""
    log "  Then restart llama-server with --lora-scaled:"
    log "    See scripts/lora-scaled-dropin.sh --adapter-path ${ADAPTER_PATH} --apply"
    log "    sudo systemctl daemon-reload && sudo systemctl restart local-slm"
    log ""
    log "  Run 2 (adapter active):"
    log "    SLM_GATE_MODE=compare $0 --adapter-path ${ADAPTER_PATH} --probes ${PROBES}"
    log "    This loads baseline from disk and computes delta."
    log ""

    # Check if we are in a sequential mode explicitly.
    _MODE="${SLM_GATE_MODE:-}"
    if [[ "${_MODE}" == "baseline" ]]; then
        log "MODE=baseline — collecting baseline outputs..."
        # Fall through to baseline collection below (no adapter active).
        _USE_HOTSWAP=0
        _SEQUENTIAL_BASELINE=1
        _SEQUENTIAL_COMPARE=0
    elif [[ "${_MODE}" == "compare" ]]; then
        log "MODE=compare — loading baseline and computing delta..."
        _USE_HOTSWAP=0
        _SEQUENTIAL_BASELINE=0
        _SEQUENTIAL_COMPARE=1
    else
        # Not in a sequential mode; exit with DEFER.
        _TS="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
        python3 - <<PYEOF
import json, sys
result = {
    "passed": False,
    "probes_run": 0,
    "delta_count": 0,
    "null_count": 0,
    "timestamp": "${_TS}",
    "protocol": "deferred",
    "reason": "dynamic /lora-adapters endpoint not available; two-run sequential required",
    "adapter_path": "${ADAPTER_PATH}",
}
with open("${RESULT_FILE}", "w") as f:
    json.dump(result, f, indent=2)
print(json.dumps(result, indent=2))
PYEOF
        exit 2
    fi
else
    _USE_HOTSWAP=1
    _SEQUENTIAL_BASELINE=0
    _SEQUENTIAL_COMPARE=0
fi

# ── Helper: send one completions probe ────────────────────────────────────────

# Outputs the model's response text to stdout.
# Args: $1 = prompt string
probe_completions() {
    local _prompt="$1"
    local _payload
    # Build JSON payload with python3 for safe quoting.
    _payload="$(python3 -c "
import json, sys
print(json.dumps({
    'prompt': sys.argv[1],
    'max_tokens': ${PROBE_MAX_TOKENS},
    'temperature': ${PROBE_TEMPERATURE},
}))
" "${_prompt}" 2>/dev/null)"

    if [[ -z "${_payload}" ]]; then
        echo ""
        return
    fi

    curl -sS --connect-timeout 10 --max-time 60 \
        -X POST "${ENDPOINT}/v1/completions" \
        -H "Content-Type: application/json" \
        -d "${_payload}" 2>/dev/null \
    | python3 -c "
import sys, json
try:
    d = json.load(sys.stdin)
    text = d.get('choices', [{}])[0].get('text', '')
    print(text.strip())
except Exception:
    print('')
" 2>/dev/null || echo ""
}

# ── Collect baseline outputs (no adapter) ─────────────────────────────────────

# In hotswap mode: adapter should already be cleared (we just POSTed []).
# In sequential baseline mode: adapter should not be loaded (user responsibility).

_EFFECTIVE_PROBES="${PROBES}"
if [[ "${_EFFECTIVE_PROBES}" -gt "${#PROBE_PROMPTS[@]}" ]]; then
    _EFFECTIVE_PROBES="${#PROBE_PROMPTS[@]}"
    log "WARN: requested ${PROBES} probes but only ${#PROBE_PROMPTS[@]} prompts available; capping at ${_EFFECTIVE_PROBES}"
fi

log ""
log "=== Phase 1: baseline outputs (no adapter) ==="

declare -a BASELINE_OUTPUTS=()
_i=0
while [[ "${_i}" -lt "${_EFFECTIVE_PROBES}" ]]; do
    _prompt="${PROBE_PROMPTS[${_i}]}"
    log "  probe $((${_i}+1))/${_EFFECTIVE_PROBES}: ${_prompt:0:60}..."
    if [[ "${DRY_RUN}" -eq 1 ]]; then
        BASELINE_OUTPUTS+=("baseline-dry-run-output-${_i}")
    else
        _out="$(probe_completions "${_prompt}")"
        BASELINE_OUTPUTS+=("${_out}")
        log "    baseline[${_i}]: ${_out:0:60}..."
    fi
    _i=$((_i + 1))
done

# In sequential baseline mode, save to disk and exit.
if [[ "${_SEQUENTIAL_BASELINE}" -eq 1 ]]; then
    _BASELINE_FILE="${RESULT_FILE%.json}-baseline.json"
    # Write baseline outputs to a temp file for safe JSON serialisation.
    _BASELINE_TMP="$(mktemp /tmp/deploy-gate-baseline-XXXXXX.txt)"
    for _o in "${BASELINE_OUTPUTS[@]}"; do printf '%s\n' "${_o}"; done > "${_BASELINE_TMP}"
    python3 - "${_BASELINE_TMP}" "${_BASELINE_FILE}" "${_EFFECTIVE_PROBES}" <<'PYEOF'
import json, sys

tmp_path   = sys.argv[1]
out_path   = sys.argv[2]
probes     = int(sys.argv[3])

with open(tmp_path) as f:
    outputs = [line.rstrip("\n") for line in f]

with open(out_path, "w") as f:
    json.dump({"baseline_outputs": outputs, "probes": probes}, f, indent=2)

print(f"Baseline saved to {out_path}")
print("Next: restart llama-server with --lora-scaled, then run with SLM_GATE_MODE=compare")
PYEOF
    rm -f "${_BASELINE_TMP}"
    exit 0
fi

# In sequential compare mode, load baseline from disk.
if [[ "${_SEQUENTIAL_COMPARE}" -eq 1 ]]; then
    _BASELINE_FILE="${RESULT_FILE%.json}-baseline.json"
    if [[ ! -f "${_BASELINE_FILE}" ]]; then
        log "ERROR: baseline file not found: ${_BASELINE_FILE}"
        log "       Run with SLM_GATE_MODE=baseline first."
        exit 3
    fi
    log "Loading baseline from ${_BASELINE_FILE}"
    # Overwrite BASELINE_OUTPUTS from disk.
    mapfile -t BASELINE_OUTPUTS < <(python3 -c "
import json, sys
with open('${_BASELINE_FILE}') as f:
    d = json.load(f)
for o in d['baseline_outputs']:
    print(o)
" 2>/dev/null)
fi

# ── Load adapter via /lora-adapters (hotswap mode only) ───────────────────────

if [[ "${_USE_HOTSWAP}" -eq 1 ]]; then
    log ""
    log "=== Phase 2: loading adapter via /lora-adapters ==="

    # PEFT adapters trained with python PEFT produce a safetensors + adapter_config.json
    # directory. llama.cpp /lora-adapters accepts a path to a pre-converted .gguf adapter.
    # If the adapter is a PEFT directory (not a .gguf file), we look for a co-located
    # gguf conversion, or fall back to using the path directly (some llama.cpp builds
    # accept the PEFT directory path).
    #
    # Convention: if <adapter_path>/../<basename>.gguf exists, use that.
    _ADAPTER_GGUF="${ADAPTER_PATH}"
    _ADAPTER_BASENAME="$(basename "${ADAPTER_PATH}")"
    _ADAPTER_PARENT="$(dirname "${ADAPTER_PATH}")"
    if [[ -f "${_ADAPTER_PARENT}/${_ADAPTER_BASENAME}.gguf" ]]; then
        _ADAPTER_GGUF="${_ADAPTER_PARENT}/${_ADAPTER_BASENAME}.gguf"
        log "  using pre-converted GGUF: ${_ADAPTER_GGUF}"
    else
        log "  no .gguf conversion found; passing PEFT directory path to /lora-adapters"
        log "  (requires llama.cpp build with PEFT-directory support)"
    fi

    _LORA_LOAD_PAYLOAD="$(python3 -c "
import json, sys
print(json.dumps([{'path': sys.argv[1], 'scale': ${LORA_SCALE}}]))
" "${_ADAPTER_GGUF}" 2>/dev/null)"

    _LORA_LOAD_STATUS=$(curl -sS -o /tmp/deploy-gate-lora-load-resp.txt \
        -w "%{http_code}" \
        --connect-timeout 5 \
        -X POST "${ENDPOINT}/lora-adapters" \
        -H "Content-Type: application/json" \
        -d "${_LORA_LOAD_PAYLOAD}" 2>/dev/null || echo "000")

    log "  /lora-adapters load status: ${_LORA_LOAD_STATUS}"
    if [[ -f /tmp/deploy-gate-lora-load-resp.txt ]]; then
        log "  response: $(cat /tmp/deploy-gate-lora-load-resp.txt | head -c 200)"
    fi

    if [[ "${_LORA_LOAD_STATUS}" != "200" && "${_LORA_LOAD_STATUS}" != "204" ]]; then
        log "WARN: adapter load returned ${_LORA_LOAD_STATUS}; adapter probes may equal baseline"
        log "      Outputs will be compared anyway; null delta will trigger FAIL."
    else
        log "  adapter loaded at scale ${LORA_SCALE}"
    fi
fi

# ── Collect adapter outputs ────────────────────────────────────────────────────

log ""
log "=== Phase 3: adapter outputs ==="

declare -a ADAPTER_OUTPUTS=()
_i=0
while [[ "${_i}" -lt "${_EFFECTIVE_PROBES}" ]]; do
    _prompt="${PROBE_PROMPTS[${_i}]}"
    log "  probe $((${_i}+1))/${_EFFECTIVE_PROBES}: ${_prompt:0:60}..."
    if [[ "${DRY_RUN}" -eq 1 ]]; then
        # In dry-run, simulate non-trivial delta by appending adapter suffix.
        ADAPTER_OUTPUTS+=("adapter-dry-run-output-${_i} [different]")
    else
        _out="$(probe_completions "${_prompt}")"
        ADAPTER_OUTPUTS+=("${_out}")
        log "    adapter[${_i}]: ${_out:0:60}..."
    fi
    _i=$((_i + 1))
done

# ── Unload adapter (hotswap mode cleanup) ─────────────────────────────────────

if [[ "${_USE_HOTSWAP}" -eq 1 ]]; then
    log ""
    log "=== Phase 4: unloading adapter ==="
    _LORA_CLEAR_STATUS2=$(curl -sS -o /dev/null -w "%{http_code}" \
        --connect-timeout 5 \
        -X POST "${ENDPOINT}/lora-adapters" \
        -H "Content-Type: application/json" \
        -d '[]' 2>/dev/null || echo "000")
    log "  /lora-adapters clear status: ${_LORA_CLEAR_STATUS2}"
fi

# ── Compute delta ─────────────────────────────────────────────────────────────

log ""
log "=== Phase 5: delta computation ==="

_DELTA_COUNT=0
_NULL_COUNT=0
_PROBES_RUN="${_EFFECTIVE_PROBES}"

_i=0
while [[ "${_i}" -lt "${_EFFECTIVE_PROBES}" ]]; do
    _base="${BASELINE_OUTPUTS[${_i}]:-}"
    _adpt="${ADAPTER_OUTPUTS[${_i}]:-}"

    # Non-trivial delta: outputs differ after stripping whitespace.
    # Both empty counts as null delta (adapter may not have responded).
    if [[ -z "${_base}" && -z "${_adpt}" ]]; then
        log "  probe $((${_i}+1)): NULL (both empty — inference failed)"
        _NULL_COUNT=$((_NULL_COUNT + 1))
    elif [[ "${_base}" == "${_adpt}" ]]; then
        log "  probe $((${_i}+1)): NULL DELTA (identical output)"
        _NULL_COUNT=$((_NULL_COUNT + 1))
    else
        log "  probe $((${_i}+1)): DELTA (outputs differ)"
        _DELTA_COUNT=$((_DELTA_COUNT + 1))
    fi
    _i=$((_i + 1))
done

log ""
log "=== Results ==="
log "  probes_run:   ${_PROBES_RUN}"
log "  delta_count:  ${_DELTA_COUNT} (non-trivial base vs adapter difference)"
log "  null_count:   ${_NULL_COUNT} (adapter output identical to base)"
log "  pass_threshold: >= ${PASS_THRESHOLD} deltas required"
log "  fail_threshold: >= ${FAIL_THRESHOLD} null deltas = FAIL"

# ── Pass/fail decision ────────────────────────────────────────────────────────

_PASSED=false
_EXIT_CODE=0

if [[ "${_DELTA_COUNT}" -ge "${PASS_THRESHOLD}" && "${_NULL_COUNT}" -lt "${FAIL_THRESHOLD}" ]]; then
    _PASSED=true
    _EXIT_CODE=0
    log ""
    log "RESULT: PASS — adapter produces non-trivial delta on ${_DELTA_COUNT}/${_PROBES_RUN} probes"
else
    _PASSED=false
    _EXIT_CODE=1
    log ""
    if [[ "${_NULL_COUNT}" -ge "${FAIL_THRESHOLD}" ]]; then
        log "RESULT: FAIL — adapter is a no-op: null delta on ${_NULL_COUNT}/${_PROBES_RUN} probes"
        log "         Possible causes:"
        log "           - adapter was not actually loaded (check /lora-adapters response above)"
        log "           - PEFT adapter was not converted to GGUF before serving"
        log "           - LoRA scale ${LORA_SCALE} too low (try 1.0)"
        log "           - base model mismatch (verify base-registry.yaml)"
    else
        log "RESULT: FAIL — insufficient delta: ${_DELTA_COUNT}/${_PROBES_RUN} < threshold ${PASS_THRESHOLD}"
    fi
fi

# ── Write result JSON ─────────────────────────────────────────────────────────

_TS="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
_PROTOCOL="hotswap"
if [[ "${_USE_HOTSWAP}" -eq 0 && "${_SEQUENTIAL_COMPARE}" -eq 1 ]]; then
    _PROTOCOL="sequential-compare"
elif [[ "${_USE_HOTSWAP}" -eq 0 ]]; then
    _PROTOCOL="baseline-only"
fi

python3 - <<PYEOF
import json

result = {
    "passed": ${_PASSED},
    "probes_run": ${_PROBES_RUN},
    "delta_count": ${_DELTA_COUNT},
    "null_count": ${_NULL_COUNT},
    "timestamp": "${_TS}",
    "protocol": "${_PROTOCOL}",
    "adapter_path": "${ADAPTER_PATH}",
    "endpoint": "${ENDPOINT}",
    "pass_threshold": ${PASS_THRESHOLD},
    "fail_threshold": ${FAIL_THRESHOLD},
}

with open("${RESULT_FILE}", "w") as f:
    json.dump(result, f, indent=2)

print(json.dumps(result, indent=2))
PYEOF

log ""
log "Result written to: ${RESULT_FILE}"

if [[ "${DRY_RUN}" -eq 1 ]]; then
    log "(dry-run — no inference was performed)"
fi

exit "${_EXIT_CODE}"
