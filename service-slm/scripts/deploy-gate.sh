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
# NOTE: this gate is direction-free — it proves the adapter CHANGED output,
# not that it IMPROVED. For a scored, direction-aware quality check against
# a curated held-out set, see score-gate.sh (shares this script's scratch-server
# infrastructure via _gate-common.sh).
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

_GATE_TAG="deploy-gate"
source "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/_gate-common.sh"

# ── Defaults specific to this script ──────────────────────────────────────────

ADAPTER_PATH=""
BASE_MODEL="${SLM_GATE_BASE_MODEL:-}"   # resolved from base-registry.yaml if unset
PROBES=20
DRY_RUN=0
RESULT_FILE="${RESULT_FILE:-${FOUNDRY_ROOT}/data/adapters/deploy-gate-result.json}"
PASS_THRESHOLD=15        # out of PROBES
FAIL_THRESHOLD=6         # null-delta count triggers FAIL (>=)

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
            sed -n '2,45p' "$0"
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

log "deploy-gate.sh starting"
log "  adapter_path:  ${ADAPTER_PATH}"
log "  probes:        ${PROBES}"
log "  endpoint:      ${ENDPOINT} (scratch — never production :8080)"
log "  result_file:   ${RESULT_FILE}"

gate_common_init

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
_BOTH_EMPTY_COUNT=0
_PROBES_RUN="${_EFFECTIVE_PROBES}"

_i=0
while [[ "${_i}" -lt "${_EFFECTIVE_PROBES}" ]]; do
    _base="${BASELINE_OUTPUTS[${_i}]:-}"
    _adpt="${ADAPTER_OUTPUTS[${_i}]:-}"

    # Non-trivial delta: outputs differ after stripping whitespace.
    # Both empty counts as null delta (adapter may not have responded) but is
    # tracked separately too — it usually means inference failed/timed out
    # under host contention, not that the adapter is a genuine no-op.
    if [[ -z "${_base}" && -z "${_adpt}" ]]; then
        log "  probe $((${_i}+1)): NULL (both empty — inference failed)"
        _NULL_COUNT=$((_NULL_COUNT + 1))
        _BOTH_EMPTY_COUNT=$((_BOTH_EMPTY_COUNT + 1))
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
log "  null_count:   ${_NULL_COUNT} (adapter output identical to base, of which ${_BOTH_EMPTY_COUNT} both-empty/inference-failed — see both_empty_count)"
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
    "both_empty_count": ${_BOTH_EMPTY_COUNT},
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
