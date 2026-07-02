#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-or-later
# SPDX-FileCopyrightText: 2026 Woodfine Capital Projects Inc.

# deploy-gate.sh — Phase D deploy gate: base-vs-adapter output delta probe.
#
# GAP-4 / D3 remediation: proves the adapter is NOT a no-op before any
# service restart that loads it via --lora-scaled.
#
# Protocol (scratch-server scale-toggle — rewritten 2026-07-01):
#   llama-server's POST /lora-adapters endpoint ONLY re-scales an adapter that
#   is already loaded at server STARTUP via --lora/--lora-init-without-apply.
#   It never accepts a runtime path to a newly trained adapter — earlier
#   versions of this script assumed otherwise (see git history), which meant
#   every prior gate run compared the fixed default endpoint against itself,
#   guaranteeing a null result regardless of adapter quality.
#
#   Corrected flow:
#     1. Convert the PEFT (safetensors) adapter to GGUF via llama.cpp's
#        convert_lora_to_gguf.py (CPU-only, no GPU needed).
#     2. Start a dedicated SCRATCH llama-server (never the production
#        endpoint) with the adapter pre-loaded but INACTIVE
#        (--lora-init-without-apply).
#     3. POST /lora-adapters [{"id":N,"scale":0.0}] -> collect baseline probes.
#     4. POST /lora-adapters [{"id":N,"scale":1.0}] -> collect adapter probes.
#     5. Kill the scratch server (trap-guaranteed, same discipline as
#        test-mode.sh's VM stop trap) and compute the delta.
#
# Requirements:
#   - convert_lora_to_gguf.py + a built llama-server at LLAMA_CPP_DIR
#     (default /opt/llama.cpp) — CPU-only, does not touch the GPU/yoyo-batch.
#   - A base GGUF matching base-registry.yaml's served_gguf, readable locally
#     (default resolved under the Tier A weights dir).
#   - curl, python3 (stdlib only).
#   - /srv/foundry/data/adapters/ writable by this user.
#
# Usage:
#   deploy-gate.sh --adapter-path <path> [--base-model <path>]
#                  [--probes <N>] [--dry-run]
#
# Exit codes:
#   0   PASS  — adapter produces non-trivial delta on >= 15/20 probes
#   1   FAIL  — adapter is a no-op (null delta on >= 6 probes)
#   3   Error — missing required argument, unreachable endpoint, conversion
#               failure, or unexpected llama.cpp build (no /lora-adapters route)

set -uo pipefail

# ── Defaults ─────────────────────────────────────────────────────────────────

FOUNDRY_ROOT="${FOUNDRY_ROOT:-/srv/foundry}"
ADAPTER_PATH=""
BASE_MODEL="${SLM_GATE_BASE_MODEL:-}"   # resolved from base-registry.yaml if unset
PROBES=20
DRY_RUN=0
RESULT_FILE="${RESULT_FILE:-${FOUNDRY_ROOT}/data/adapters/deploy-gate-result.json}"
PASS_THRESHOLD=15        # out of PROBES
FAIL_THRESHOLD=6         # null-delta count triggers FAIL (>=)
PROBE_MAX_TOKENS=128
PROBE_TEMPERATURE="0.0"  # greedy — deterministic outputs required for comparison

# Scratch-server config — deliberately never the production endpoint (127.0.0.1:8080).
SCRATCH_PORT="${SLM_GATE_SCRATCH_PORT:-8090}"
ENDPOINT="http://127.0.0.1:${SCRATCH_PORT}"
LLAMA_CPP_DIR="${SLM_GATE_LLAMA_CPP_DIR:-/opt/llama.cpp}"
CONVERT_VENV="${SLM_GATE_CONVERT_VENV:-${FOUNDRY_ROOT}/data/adapters/.gguf-convert-venv}"
BASE_REGISTRY="${SLM_GATE_BASE_REGISTRY:-$(cd "$(dirname "$0")/.." && pwd)/data/base-registry.yaml}"
_SCRATCH_PID=""

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
        --dry-run)         DRY_RUN=1 ;;
        --help|-h)
            sed -n '2,42p' "$0"
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

# ── Logging + cleanup trap ────────────────────────────────────────────────────

log() { printf '[deploy-gate %s] %s\n' "$(date -u +'%Y-%m-%dT%H:%M:%SZ')" "$*"; }

cleanup() {
    if [[ -n "${_SCRATCH_PID}" ]] && kill -0 "${_SCRATCH_PID}" 2>/dev/null; then
        log "Stopping scratch llama-server (pid ${_SCRATCH_PID})..."
        kill "${_SCRATCH_PID}" 2>/dev/null || true
        wait "${_SCRATCH_PID}" 2>/dev/null || true
    fi
}
trap cleanup EXIT INT TERM

log "deploy-gate.sh starting"
log "  adapter_path:  ${ADAPTER_PATH}"
log "  probes:        ${PROBES}"
log "  endpoint:      ${ENDPOINT} (scratch — never production :8080)"
log "  result_file:   ${RESULT_FILE}"

# ── Prereq checks ─────────────────────────────────────────────────────────────

if [[ ! -d "${ADAPTER_PATH}" ]]; then
    log "ERROR: adapter directory not found: ${ADAPTER_PATH}"
    exit 3
fi
if [[ ! -f "${ADAPTER_PATH}/adapter_config.json" ]]; then
    log "ERROR: adapter_config.json not found in ${ADAPTER_PATH}"
    log "       This is not a valid PEFT LoRA checkpoint."
    exit 3
fi

mkdir -p "$(dirname "${RESULT_FILE}")"

if ! command -v python3 >/dev/null 2>&1; then
    log "ERROR: python3 required (stdlib only)"
    exit 3
fi

if [[ ! -x "${LLAMA_CPP_DIR}/build/bin/llama-server" ]]; then
    log "ERROR: llama-server binary not found at ${LLAMA_CPP_DIR}/build/bin/llama-server"
    exit 3
fi
if [[ ! -f "${LLAMA_CPP_DIR}/convert_lora_to_gguf.py" ]]; then
    log "ERROR: convert_lora_to_gguf.py not found at ${LLAMA_CPP_DIR}"
    exit 3
fi

# Resolve BASE_MODEL from base-registry.yaml if not explicitly overridden —
# canonical_base / served_gguf is the single source of truth for SFT base ==
# DPO base == ref_model == served GGUF (base-registry.yaml's own header comment).
_CANONICAL_BASE=""
if [[ -z "${BASE_MODEL}" ]]; then
    if [[ ! -f "${BASE_REGISTRY}" ]]; then
        log "ERROR: base-registry.yaml not found at ${BASE_REGISTRY} and --base-model not given"
        exit 3
    fi
    _SERVED_GGUF="$(grep -E '^served_gguf:' "${BASE_REGISTRY}" | sed 's/^served_gguf:[[:space:]]*//' | tr -d '"'"'"'\r')"
    _CANONICAL_BASE="$(grep -E '^canonical_base:' "${BASE_REGISTRY}" | sed 's/^canonical_base:[[:space:]]*//' | tr -d '"'"'"'\r')"
    if [[ -z "${_SERVED_GGUF}" ]]; then
        log "ERROR: could not read served_gguf: from ${BASE_REGISTRY}"
        exit 3
    fi
    BASE_MODEL="/var/lib/local-slm/weights/${_SERVED_GGUF}"
    log "  base_model:    ${BASE_MODEL} (resolved from base-registry.yaml served_gguf)"

    # /var/lib/local-slm/ is local-slm:local-slm drwxr-x--- — not traversable by this
    # user even though the file itself is other-readable. Cache a copy once under a
    # mathew-owned dir rather than running the scratch server as root.
    _BASE_CACHE_DIR="${FOUNDRY_ROOT}/data/adapters/.gate-base-model"
    _BASE_CACHE_PATH="${_BASE_CACHE_DIR}/${_SERVED_GGUF}"
    if [[ ! -f "${_BASE_CACHE_PATH}" ]]; then
        mkdir -p "${_BASE_CACHE_DIR}"
        log "  Caching base GGUF (one-time, ~4.5GB) to ${_BASE_CACHE_PATH}..."
        if ! sudo cp "${BASE_MODEL}" "${_BASE_CACHE_PATH}"; then
            log "ERROR: could not read/copy ${BASE_MODEL} via sudo"
            exit 3
        fi
        sudo chown "$(id -u):$(id -g)" "${_BASE_CACHE_PATH}"
    fi
    BASE_MODEL="${_BASE_CACHE_PATH}"
else
    log "  base_model:    ${BASE_MODEL} (explicit --base-model override)"
fi
if [[ ! -f "${BASE_MODEL}" ]]; then
    log "ERROR: base model GGUF not found: ${BASE_MODEL}"
    exit 3
fi
if [[ -z "${_CANONICAL_BASE}" && -f "${BASE_REGISTRY}" ]]; then
    _CANONICAL_BASE="$(grep -E '^canonical_base:' "${BASE_REGISTRY}" | sed 's/^canonical_base:[[:space:]]*//' | tr -d '"'"'"'\r')"
fi
if [[ -z "${_CANONICAL_BASE}" ]]; then
    log "ERROR: could not resolve canonical_base (HF repo id) for GGUF conversion — pass --base-model and set SLM_GATE_BASE_MODEL_ID"
    exit 3
fi

# ── GGUF conversion (CPU-only, no GPU) ────────────────────────────────────────

_ADAPTER_GGUF="${ADAPTER_PATH%/}.gguf"

if [[ "${DRY_RUN}" -eq 1 ]]; then
    log "(dry-run — skipping GGUF conversion + scratch server)"
elif [[ -f "${_ADAPTER_GGUF}" && "${_ADAPTER_GGUF}" -nt "${ADAPTER_PATH}/adapter_model.safetensors" ]]; then
    log "GGUF adapter already converted and up to date: ${_ADAPTER_GGUF}"
else
    log ""
    log "=== Phase 0: GGUF conversion ==="

    if [[ ! -x "${CONVERT_VENV}/bin/python3" ]] || \
       ! "${CONVERT_VENV}/bin/python3" -c "import torch, transformers, safetensors, huggingface_hub, requests" >/dev/null 2>&1; then
        log "  Bootstrapping CPU-only conversion venv at ${CONVERT_VENV}..."
        python3 -m venv "${CONVERT_VENV}"
        "${CONVERT_VENV}/bin/pip" install --quiet \
            torch --index-url https://download.pytorch.org/whl/cpu
        "${CONVERT_VENV}/bin/pip" install --quiet \
            transformers safetensors huggingface_hub requests
    fi

    log "  Converting ${ADAPTER_PATH} -> ${_ADAPTER_GGUF} (base-model-id=${_CANONICAL_BASE})..."
    if ! "${CONVERT_VENV}/bin/python3" "${LLAMA_CPP_DIR}/convert_lora_to_gguf.py" \
            --outtype f16 \
            --base-model-id "${_CANONICAL_BASE}" \
            --outfile "${_ADAPTER_GGUF}" \
            "${ADAPTER_PATH}"; then
        log "ERROR: convert_lora_to_gguf.py failed"
        exit 3
    fi
    if [[ ! -f "${_ADAPTER_GGUF}" ]]; then
        log "ERROR: conversion reported success but ${_ADAPTER_GGUF} does not exist"
        exit 3
    fi
    log "  Conversion OK: $(du -h "${_ADAPTER_GGUF}" | cut -f1) at ${_ADAPTER_GGUF}"
fi

# ── Start scratch llama-server with the adapter pre-loaded but inactive ──────

if [[ "${DRY_RUN}" -eq 0 ]]; then
    log ""
    log "=== Phase 1: scratch llama-server (adapter loaded, scale 0) ==="
    "${LLAMA_CPP_DIR}/build/bin/llama-server" \
        --host 127.0.0.1 --port "${SCRATCH_PORT}" \
        --model "${BASE_MODEL}" \
        --lora "${_ADAPTER_GGUF}" --lora-init-without-apply \
        --ctx-size 2048 --threads 4 --no-jinja \
        >>"${RESULT_FILE%.json}-scratch-server.log" 2>&1 &
    _SCRATCH_PID=$!
    log "  scratch llama-server pid=${_SCRATCH_PID}, port=${SCRATCH_PORT}"

    _WAITED=0
    _READY=0
    while [[ "${_WAITED}" -lt 60 ]]; do
        if curl -sS --connect-timeout 2 "${ENDPOINT}/health" >/dev/null 2>&1; then
            _READY=1
            break
        fi
        if ! kill -0 "${_SCRATCH_PID}" 2>/dev/null; then
            log "ERROR: scratch llama-server exited during startup — see ${RESULT_FILE%.json}-scratch-server.log"
            exit 3
        fi
        sleep 2
        _WAITED=$((_WAITED + 2))
    done
    if [[ "${_READY}" -ne 1 ]]; then
        log "ERROR: scratch llama-server did not become healthy within 60s"
        exit 3
    fi
    log "  scratch llama-server healthy after ${_WAITED}s"

    # Discover the adapter id (don't hardcode 0) and confirm /lora-adapters exists.
    # /health can return OK while tensor loading (mmap, lazy) is still in progress —
    # /lora-adapters returns 503 during that window, not a fixed "unsupported" signal.
    # Poll with retries rather than a single attempt.
    _LORA_LIST=""
    _LORA_WAITED=0
    while [[ "${_LORA_WAITED}" -lt 60 ]]; do
        _LORA_HTTP_CODE=$(curl -sS -o /tmp/deploy-gate-lora-list.json -w "%{http_code}" \
            --connect-timeout 5 "${ENDPOINT}/lora-adapters" 2>/dev/null || echo "000")
        if [[ "${_LORA_HTTP_CODE}" == "200" ]]; then
            _LORA_LIST="$(cat /tmp/deploy-gate-lora-list.json 2>/dev/null)"
            break
        fi
        if [[ "${_LORA_HTTP_CODE}" == "404" ]]; then
            log "ERROR: GET /lora-adapters returned 404 — unexpected llama.cpp build (no hotswap route)"
            exit 3
        fi
        sleep 2
        _LORA_WAITED=$((_LORA_WAITED + 2))
    done
    if [[ -z "${_LORA_LIST}" ]]; then
        log "ERROR: GET /lora-adapters did not return 200 within 60s (last HTTP ${_LORA_HTTP_CODE:-???})"
        exit 3
    fi
    _ADAPTER_ID="$(python3 -c "
import json, sys
try:
    items = json.loads(sys.argv[1])
    print(items[0]['id'] if items else '')
except Exception:
    print('')
" "${_LORA_LIST}" 2>/dev/null)"
    if [[ -z "${_ADAPTER_ID}" ]]; then
        log "ERROR: /lora-adapters returned no entries — adapter did not load at startup"
        log "       response: ${_LORA_LIST}"
        exit 3
    fi
    log "  adapter id=${_ADAPTER_ID} loaded (inactive, scale 0)"
fi

# ── Helper: send one completions probe ────────────────────────────────────────

probe_completions() {
    local _prompt="$1"
    local _payload
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

# ── Helper: set adapter scale via POST /lora-adapters ─────────────────────────

set_adapter_scale() {
    local _scale="$1"
    local _payload
    _payload="$(python3 -c "
import json
print(json.dumps([{'id': ${_ADAPTER_ID}, 'scale': ${_scale}}]))
" 2>/dev/null)"
    local _status
    _status=$(curl -sS -o /dev/null -w "%{http_code}" \
        --connect-timeout 5 \
        -X POST "${ENDPOINT}/lora-adapters" \
        -H "Content-Type: application/json" \
        -d "${_payload}" 2>/dev/null || echo "000")
    log "  /lora-adapters scale=${_scale} status: ${_status}"
    if [[ "${_status}" != "200" && "${_status}" != "204" ]]; then
        log "ERROR: setting adapter scale=${_scale} failed (HTTP ${_status})"
        exit 3
    fi
}

# ── Collect baseline outputs (scale 0.0) ──────────────────────────────────────

_EFFECTIVE_PROBES="${PROBES}"
if [[ "${_EFFECTIVE_PROBES}" -gt "${#PROBE_PROMPTS[@]}" ]]; then
    _EFFECTIVE_PROBES="${#PROBE_PROMPTS[@]}"
    log "WARN: requested ${PROBES} probes but only ${#PROBE_PROMPTS[@]} prompts available; capping at ${_EFFECTIVE_PROBES}"
fi

if [[ "${DRY_RUN}" -eq 0 ]]; then
    set_adapter_scale "0.0"
fi

log ""
log "=== Phase 2: baseline outputs (adapter scale 0.0) ==="

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

# ── Collect adapter outputs (scale 1.0) ───────────────────────────────────────

if [[ "${DRY_RUN}" -eq 0 ]]; then
    set_adapter_scale "1.0"
fi

log ""
log "=== Phase 3: adapter outputs (adapter scale 1.0) ==="

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

# Scratch server is killed by the EXIT trap — no explicit unload phase needed.

# ── Compute delta ─────────────────────────────────────────────────────────────

log ""
log "=== Phase 4: delta computation ==="

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
        log "           - GGUF conversion produced a degenerate/empty adapter"
        log "           - adapter was undertrained (check optimizer step count vs corpus size)"
        log "           - LoRA scale 1.0 too low for this rank/alpha (check adapter_config.json)"
        log "           - base model mismatch (verify base-registry.yaml canonical_base matches"
        log "             the adapter's own base_model_name_or_path)"
    else
        log "RESULT: FAIL — insufficient delta: ${_DELTA_COUNT}/${_PROBES_RUN} < threshold ${PASS_THRESHOLD}"
    fi
fi

# ── Write result JSON ─────────────────────────────────────────────────────────

_TS="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
_PASSED_PY="False"
[[ "${_PASSED}" == "true" ]] && _PASSED_PY="True"

python3 - <<PYEOF
import json

result = {
    "passed": ${_PASSED_PY},
    "probes_run": ${_PROBES_RUN},
    "delta_count": ${_DELTA_COUNT},
    "null_count": ${_NULL_COUNT},
    "timestamp": "${_TS}",
    "protocol": "scratch-scale-toggle",
    "adapter_path": "${ADAPTER_PATH}",
    "adapter_gguf": "${_ADAPTER_GGUF}",
    "endpoint": "${ENDPOINT}",
    "base_model": "${BASE_MODEL}",
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
