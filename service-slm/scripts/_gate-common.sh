#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-or-later
# SPDX-FileCopyrightText: 2026 Woodfine Capital Projects Inc.

# _gate-common.sh — shared scratch-server infrastructure for deploy-gate.sh and
# score-gate.sh. Not directly executable; sourced by both.
#
# Provides, once sourced and `gate_common_init` is called:
#   - log()                     timestamped log line, prefixed with the caller's tag
#   - cleanup() + EXIT/INT/TERM trap  kills the scratch llama-server on any exit path
#   - gate_common_init          prereq checks + BASE_MODEL resolution + GGUF conversion
#                                + scratch server boot + adapter-id discovery
#   - probe_completions <prompt>       POST /v1/completions, prints the completion text
#   - set_adapter_scale <0.0|1.0>      POST /lora-adapters to toggle the adapter
#
# Callers must set before sourcing/calling gate_common_init:
#   ADAPTER_PATH, RESULT_FILE, DRY_RUN (0|1), and optionally BASE_MODEL (empty = auto-resolve)
# gate_common_init sets (for the caller to use):
#   BASE_MODEL, _ADAPTER_GGUF, _CANONICAL_BASE, _ADAPTER_ID, ENDPOINT

FOUNDRY_ROOT="${FOUNDRY_ROOT:-/srv/foundry}"
SCRATCH_PORT="${SLM_GATE_SCRATCH_PORT:-8090}"
ENDPOINT="http://127.0.0.1:${SCRATCH_PORT}"
LLAMA_CPP_DIR="${SLM_GATE_LLAMA_CPP_DIR:-/opt/llama.cpp}"
CONVERT_VENV="${SLM_GATE_CONVERT_VENV:-${FOUNDRY_ROOT}/data/adapters/.gguf-convert-venv}"
BASE_REGISTRY="${SLM_GATE_BASE_REGISTRY:-$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)/data/base-registry.yaml}"
PROBE_MAX_TOKENS="${PROBE_MAX_TOKENS:-128}"
PROBE_TEMPERATURE="${PROBE_TEMPERATURE:-0.0}"
_SCRATCH_PID=""

_GATE_TAG="${_GATE_TAG:-gate}"
log() { printf '[%s %s] %s\n' "${_GATE_TAG}" "$(date -u +'%Y-%m-%dT%H:%M:%SZ')" "$*"; }

cleanup() {
    if [[ -n "${_SCRATCH_PID}" ]] && kill -0 "${_SCRATCH_PID}" 2>/dev/null; then
        log "Stopping scratch llama-server (pid ${_SCRATCH_PID})..."
        kill "${_SCRATCH_PID}" 2>/dev/null || true
        wait "${_SCRATCH_PID}" 2>/dev/null || true
    fi
}
trap cleanup EXIT INT TERM

# gate_common_init — prereqs, base-model resolution, GGUF conversion, scratch-server
# boot, adapter-id discovery. Exits 3 on any failure (matches deploy-gate.sh's existing
# exit-code contract). No-ops the server/conversion steps when DRY_RUN=1.
gate_common_init() {
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

    # ── Host-contention pre-flight (non-blocking) ─────────────────────────────
    # Production Tier A (llama-server :8080) competes for CPU with this script's
    # scratch server + probes. A busy host doesn't invalidate the run, but it can
    # produce empty/timed-out probes or inflated generation times that look like
    # an adapter-quality finding when it's actually contention. Warn, don't abort.
    _gc_load1="$(cut -d' ' -f1 /proc/loadavg 2>/dev/null || echo "?")"
    if [[ "${_gc_load1}" != "?" ]] && [[ "${_gc_load1%.*}" -ge 4 ]] 2>/dev/null; then
        log "WARNING: host load average ${_gc_load1} — production Tier A may be busy;"
        log "         probe timing/empty-result rates in this run may reflect contention,"
        log "         not adapter quality. Re-run when the host is quieter for a clean read."
    fi

    # ── GGUF conversion (CPU-only, no GPU) ────────────────────────────────────
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

    # ── Start scratch llama-server with the adapter pre-loaded but inactive ──
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
        # 180s, not 60s: this workspace VM also runs the production Tier A extraction
        # queue (often 1000+ deep), and scratch-server model loading competes for CPU
        # with it — confirmed live (load average 5.75, queue_pending 1157) causing a
        # 60s window to expire while /lora-adapters was still legitimately 503.
        _LORA_LIST=""
        _LORA_WAITED=0
        while [[ "${_LORA_WAITED}" -lt 180 ]]; do
            _LORA_HTTP_CODE=$(curl -sS -o /tmp/gate-lora-list.json -w "%{http_code}" \
                --connect-timeout 5 "${ENDPOINT}/lora-adapters" 2>/dev/null || echo "000")
            if [[ "${_LORA_HTTP_CODE}" == "200" ]]; then
                _LORA_LIST="$(cat /tmp/gate-lora-list.json 2>/dev/null)"
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
            log "ERROR: GET /lora-adapters did not return 200 within 180s (last HTTP ${_LORA_HTTP_CODE:-???})"
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
}

# probe_completions <prompt> — POST /v1/completions, print the completion text (or "").
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

    curl -sS --connect-timeout 10 --max-time "${PROBE_CURL_MAX_TIME:-60}" \
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

# set_adapter_scale <0.0|1.0> — POST /lora-adapters to toggle the loaded adapter's scale.
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
