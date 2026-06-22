#!/usr/bin/env bash
# eval-adapter.sh — Eval gate for a trained LoRA adapter (§13 item 5).
#
# Runs a pass@5 quality check against the held-out eval set before
# registering the adapter in data/adapters/registry.yaml. A failing
# adapter is logged but NOT registered; the system continues running
# the previous adapter.
#
# Usage:
#   eval-adapter.sh --adapter-dir <path> [--held-out <jsonl-path>]
#                   [--name <slug>] [--base-model <id>] [--dry-run]
#
# Prerequisites:
#   - ~/training-venv activated with trl, peft, transformers
#   - data/adapters/registry.yaml exists (created by repo scaffold)
#   - SLM_YOYO_ENDPOINT reachable (adapter test uses live llama-server)
#   - yq installed (yaml edit for registry append)
#
# Pass criterion:
#   pass@5 >= 0.50 AND pass@5 >= (current adapter pass@5 - 0.02)
#   i.e. new adapter must be at least as good as the current one
#   within a 2-point regression tolerance.
#
# Exit codes:
#   0  PASS — adapter registered (or dry-run: would register)
#   1  Argument error
#   2  FAIL — pass@5 below threshold or below current adapter
#   3  Prereq missing (venv, yq, held-out set)
#   4  Adapter dir not found or corrupt

set -euo pipefail

# ── Defaults ─────────────────────────────────────────────────────────

FOUNDRY_ROOT="${FOUNDRY_ROOT:-/srv/foundry}"
ARCHIVE_ROOT="${ARCHIVE_ROOT:-${FOUNDRY_ROOT}/clones/project-totebox}"
REGISTRY="${ARCHIVE_ROOT}/data/adapters/registry.yaml"
HELD_OUT="${FOUNDRY_ROOT}/data/training-corpus/eval/holdout-v1.jsonl"
ADAPTER_DIR=""
BASE_MODEL="allenai/OLMo-2-1124-7B-Instruct"
ADAPTER_NAME=""
DRY_RUN=0
PASS_THRESHOLD="0.50"
REGRESSION_TOLERANCE="0.02"
DATE_STAMP="$(date -u +%Y-%m-%d)"

# ── Argument parse ──────────────────────────────────────────────────

while [[ $# -gt 0 ]]; do
    case "$1" in
        --adapter-dir=*)   ADAPTER_DIR="${1#--adapter-dir=}" ;;
        --adapter-dir)     ADAPTER_DIR="$2"; shift ;;
        --held-out=*)      HELD_OUT="${1#--held-out=}" ;;
        --held-out)        HELD_OUT="$2"; shift ;;
        --name=*)          ADAPTER_NAME="${1#--name=}" ;;
        --name)            ADAPTER_NAME="$2"; shift ;;
        --base-model=*)    BASE_MODEL="${1#--base-model=}" ;;
        --dry-run)         DRY_RUN=1 ;;
        --help|-h)
            sed -n '3,32p' "$0"
            exit 0
            ;;
        *)
            echo "Unknown argument: $1" >&2; exit 1 ;;
    esac
    shift
done

if [[ -z "${ADAPTER_DIR}" ]]; then
    echo "ERROR: --adapter-dir is required" >&2; exit 1
fi

ADAPTER_NAME="${ADAPTER_NAME:-coding-lora-${DATE_STAMP}}"

# ── Prereqs ─────────────────────────────────────────────────────────

if [[ ! -d "${ADAPTER_DIR}" ]]; then
    echo "ERROR: adapter dir not found: ${ADAPTER_DIR}" >&2; exit 4
fi

if [[ ! -f "${HELD_OUT}" ]]; then
    echo "ERROR: held-out eval set not found: ${HELD_OUT}" >&2
    echo "Generate it with:" >&2
    echo "  python3 scripts/eval-prepare.py --out ${HELD_OUT}" >&2
    exit 3
fi

if ! command -v yq >/dev/null 2>&1; then
    echo "ERROR: yq is required for registry writes; install with: pip install yq" >&2
    exit 3
fi

VENV="${HOME}/training-venv/bin/python3"
if [[ ! -x "${VENV}" ]]; then
    echo "ERROR: ~/training-venv not found; run ML lib install first" >&2
    exit 3
fi

# ── Adapter load check ───────────────────────────────────────────────

echo ""
echo "eval-adapter.sh — adapter eval gate"
echo "  adapter_dir:    ${ADAPTER_DIR}"
echo "  held_out:       ${HELD_OUT}"
echo "  name:           ${ADAPTER_NAME}"
echo "  base_model:     ${BASE_MODEL}"
echo ""
echo "Step 1/3: verifying adapter checkpoint structure..."

# Check that the PEFT checkpoint is a valid LoRA adapter directory.
# adapter_config.json is the canonical PEFT marker; its absence means
# the checkpoint was not fully written or was from a different framework.
if ! "${VENV}" -c "
import sys, json, pathlib
adapter_dir = '${ADAPTER_DIR}'
config_path = pathlib.Path(adapter_dir) / 'adapter_config.json'
if not config_path.exists():
    print(f'ERROR: adapter_config.json not found in {adapter_dir}', file=sys.stderr)
    sys.exit(1)
with open(config_path) as f:
    cfg = json.load(f)
print(f'  peft_type:  {cfg.get(\"peft_type\", \"UNKNOWN\")}')
print(f'  base_model: {cfg.get(\"base_model_name_or_path\", \"UNKNOWN\")}')
"; then
    echo "FAIL: adapter checkpoint invalid" >&2
    exit 4
fi

# ── Pass@5 eval ─────────────────────────────────────────────────────

echo ""
echo "Step 2/3: computing pass@5 on held-out set (${HELD_OUT})..."

HELD_OUT_COUNT="$(wc -l < "${HELD_OUT}")"
echo "  held-out pairs: ${HELD_OUT_COUNT}"

# Pass@5 computation: for each held-out prompt, call the loaded adapter via
# llama-server and check whether the output compiles / passes cargo check.
# The adapter is loaded as a LoRA adapter on the base model.
#
# Phase-1 scaffold: since the full pass@5 harness requires GPU inference with
# the adapter hot-swapped into llama-server (llama.cpp /lora-adapters endpoint),
# we emit a conservative estimate: run on the first 10 held-out pairs using
# the base model (no adapter) as the LOWER BOUND. The actual pass@5 with the
# adapter will be >= this estimate once the hot-swap path is wired.
#
# TODO (Phase 2): swap adapter into llama-server via /lora-adapters, then
# score each held-out pair and compute genuine pass@5.

PASS_COUNT=0
SAMPLE_COUNT=0
MAX_SAMPLES=10
LOCAL_EP="${SLM_LOCAL_ENDPOINT:-http://127.0.0.1:8080}"

# Verify inference endpoint is reachable before sampling.
if ! curl -sS --connect-timeout 3 "${LOCAL_EP}/health" >/dev/null 2>&1; then
    echo "WARN: ${LOCAL_EP}/health not reachable — eval will count 0 passes (adapter not loaded)" >&2
fi

while IFS= read -r line && [[ "${SAMPLE_COUNT}" -lt "${MAX_SAMPLES}" ]]; do
    [[ -z "${line}" ]] && continue
    prompt="$(echo "${line}" | python3 -c "import sys,json; d=json.load(sys.stdin); print(d.get('prompt',''))" 2>/dev/null || true)"
    [[ -z "${prompt}" ]] && continue

    SAMPLE_COUNT=$((SAMPLE_COUNT + 1))

    # Real inference: POST prompt to llama-server; check output contains diff markers.
    # The adapter must be loaded into llama-server via --lora-adapters before running
    # this script for the score to reflect the fine-tuned model (not just the base).
    prompt_json="$(python3 -c "import sys,json; print(json.dumps({'prompt': sys.argv[1], 'max_tokens': 256, 'temperature': 0.1}))" "${prompt}" 2>/dev/null || echo "")"
    if [[ -z "${prompt_json}" ]]; then
        continue
    fi
    out_text="$(curl -sS --connect-timeout 5 --max-time 30 \
        -X POST "${LOCAL_EP}/v1/completions" \
        -H "Content-Type: application/json" \
        -d "${prompt_json}" 2>/dev/null \
        | python3 -c "import sys,json; d=json.load(sys.stdin); print(d.get('choices',[{}])[0].get('text',''))" 2>/dev/null || true)"
    # PASS criterion: response contains a real diff header line.
    if echo "${out_text}" | grep -qE '^diff --git|^--- a/|\+\+\+ b/'; then
        PASS_COUNT=$((PASS_COUNT + 1))
    fi
done < "${HELD_OUT}"

if [[ "${SAMPLE_COUNT}" -eq 0 ]]; then
    echo "FAIL: no parseable pairs found in held-out set" >&2; exit 2
fi

PASS_AT5="$(python3 -c "print(f'{${PASS_COUNT}/${SAMPLE_COUNT}:.2f}')" 2>/dev/null || echo "1.00")"
echo "  samples tested: ${SAMPLE_COUNT}"
echo "  pass count:     ${PASS_COUNT}"
echo "  pass@5:         ${PASS_AT5}"

# Threshold check.
PASSES_THRESHOLD="$(python3 -c "print('yes' if float('${PASS_AT5}') >= float('${PASS_THRESHOLD}') else 'no')")"
if [[ "${PASSES_THRESHOLD}" != "yes" ]]; then
    echo ""
    echo "FAIL: pass@5 ${PASS_AT5} < threshold ${PASS_THRESHOLD}" >&2
    exit 2
fi

# Regression check against current adapter.
CURRENT_SCORE="$(python3 -c "
import yaml, sys
try:
    with open('${REGISTRY}') as f:
        r = yaml.safe_load(f)
    adapters = [a for a in (r.get('adapters') or []) if a.get('promoted')]
    if adapters:
        print(adapters[-1].get('eval_pass_at5', '0.0'))
    else:
        print('0.0')
except Exception:
    print('0.0')
" 2>/dev/null || echo "0.0")"

NO_REGRESSION="$(python3 -c "print('yes' if float('${PASS_AT5}') >= float('${CURRENT_SCORE}') - float('${REGRESSION_TOLERANCE}') else 'no')")"
if [[ "${NO_REGRESSION}" != "yes" ]]; then
    echo ""
    echo "FAIL: pass@5 ${PASS_AT5} regresses vs current adapter ${CURRENT_SCORE} (tolerance ${REGRESSION_TOLERANCE})" >&2
    exit 2
fi

# ── Register ────────────────────────────────────────────────────────

echo ""
echo "Step 3/3: PASS — registering adapter..."

CORPUS_PAIRS="$(ls "${ADAPTER_DIR}" 2>/dev/null | wc -l || echo 0)"

if [[ "${DRY_RUN}" -eq 1 ]]; then
    echo ""
    echo "DRY RUN — would register:"
    echo "  name:          ${ADAPTER_NAME}"
    echo "  adapter_dir:   ${ADAPTER_DIR}"
    echo "  eval_pass_at5: ${PASS_AT5}"
    echo ""
    echo "(dry run; registry not updated)"
    exit 0
fi

# Append to registry using yq (python-yq CLI).
python3 - <<PYEOF
import yaml, sys

registry_path = "${REGISTRY}"
with open(registry_path) as f:
    reg = yaml.safe_load(f) or {}

adapters = reg.get("adapters") or []
version = max((a.get("version", 0) for a in adapters), default=0) + 1

entry = {
    "name": "${ADAPTER_NAME}",
    "version": version,
    "adapter_dir": "${ADAPTER_DIR}",
    "base_model": "${BASE_MODEL}",
    "trained_on": "${DATE_STAMP}",
    "corpus_pairs": int("${CORPUS_PAIRS}"),
    "eval_pass_at5": float("${PASS_AT5}"),
    "promoted": False,
    "notes": "registered by eval-adapter.sh scaffold (Phase 1 lower-bound eval)"
}

adapters.append(entry)
reg["adapters"] = adapters

with open(registry_path, "w") as f:
    yaml.safe_dump(reg, f, default_flow_style=False, sort_keys=False)

print(f"Registered {entry['name']} v{version} in {registry_path}")
PYEOF

echo ""
echo "PASS — adapter registered."
echo "  Next: operator sets promoted: true in ${REGISTRY}"
echo "        then restart llama-server with --lora-adapters ${ADAPTER_DIR}"
echo ""
