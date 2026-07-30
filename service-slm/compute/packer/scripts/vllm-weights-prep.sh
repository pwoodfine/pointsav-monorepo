#!/usr/bin/env bash
# Yo-Yo first-boot weights bootstrap.
#
# Two-mode logic:
#   Path A (warm/normal):  Q4_K_M GGUF already exists in our GCS bucket
#                          → gsutil cp to /data/weights/, verify sha256, exit 0.
#   Path B (cold/first-boot ever): bucket has no canonical artifact yet
#                          → fetch HF token from instance metadata, download
#                            AllenAI safetensors, run llama.cpp convert →
#                            GGUF fp16 → Q4_K_M, upload Q4_K_M + sha256 to GCS,
#                            delete intermediates, exit 0.
#
# After Path B runs once on the first Yo-Yo VM ever, every subsequent VM (this
# one or future zone-migrated replacements) takes Path A and is ready in ~5 min.
#
# This script is idempotent: re-running it after success is a no-op (Path A
# detects the file already present at the right sha256 and returns).
#
# Sovereignty: HF appears in the chain only at first-boot bootstrap, as the
# transport for AllenAI's authoritative safetensors. The quantization step is
# performed on this VM with our pinned llama.cpp. The canonical artifact ends
# up in our GCS bucket. Runtime is HF-free.

set -euo pipefail

# ── Configuration ───────────────────────────────────────────────────────────
WEIGHTS_DIR="/data/weights"
WEIGHTS_FILE="${WEIGHTS_DIR}/olmo-3-32b-think-q3.gguf"
TOKENIZER_DIR="${WEIGHTS_DIR}/tokenizer"
STAGING_DIR="${WEIGHTS_DIR}/staging"
LOG_FILE="/var/log/yoyo-weights-prep.log"

HF_REPO="${HF_REPO:-allenai/Olmo-3-32B-Think}"
LLAMA_CPP_DIR="/opt/llama.cpp"

mkdir -p "${WEIGHTS_DIR}" "${TOKENIZER_DIR}"
exec >> >(tee -a "${LOG_FILE}") 2>&1

log() { echo "[vllm-weights-prep $(date -u +'%Y-%m-%dT%H:%M:%SZ')] $*"; }

log "Session start. HF_REPO=${HF_REPO}"

# ── Read GCS bucket from instance metadata ──────────────────────────────────
GCS_BUCKET=$(curl -fsS --max-time 5 -H 'Metadata-Flavor: Google' \
    "http://metadata.google.internal/computeMetadata/v1/instance/attributes/weights-gcs-bucket" 2>/dev/null || echo "")
if [[ -z "${GCS_BUCKET}" ]]; then
    log "FATAL: instance metadata weights-gcs-bucket not set. Abort."
    exit 2
fi
log "GCS bucket: gs://${GCS_BUCKET}/"

GCS_WEIGHTS_URL="gs://${GCS_BUCKET}/base-models/olmo-3-32b-think-q3.gguf"
GCS_SHA256_URL="gs://${GCS_BUCKET}/base-models/olmo-3-32b-think-q3.gguf.sha256"
GCS_TOKENIZER_PREFIX="gs://${GCS_BUCKET}/base-models/tokenizer/"

# ── Helpers ─────────────────────────────────────────────────────────────────

# Create /data/weights/model/ — a directory containing config.json + a symlink
# to the GGUF.  vLLM reads config.json from the directory path (avoiding the
# transformers GGUF architecture check for olmo2/olmo3) and discovers the GGUF
# by scanning for *.gguf files in the same directory.
MODEL_DIR="${WEIGHTS_DIR}/model"
setup_model_dir() {
    mkdir -p "${MODEL_DIR}"
    cp "${TOKENIZER_DIR}/config.json" "${MODEL_DIR}/" 2>/dev/null || true
    cp "${TOKENIZER_DIR}/generation_config.json" "${MODEL_DIR}/" 2>/dev/null || true
    ln -sf "${WEIGHTS_FILE}" "${MODEL_DIR}/$(basename "${WEIGHTS_FILE}")" 2>/dev/null || true
    log "vLLM model directory ready at ${MODEL_DIR} (config.json + GGUF symlink)"
}

verify_sha256() {
    local file="$1" expected="$2"
    local actual
    actual=$(sha256sum "${file}" | awk '{print $1}')
    if [[ "${actual}" != "${expected}" ]]; then
        log "FATAL: sha256 mismatch on ${file}. Expected=${expected} actual=${actual}"
        return 1
    fi
    log "sha256 verified: ${file}"
    return 0
}

# ── Path A: GCS canonical artifact exists → fast path ──────────────────────
log "Checking for canonical artifact at ${GCS_SHA256_URL}..."
EXPECTED_SHA=$(gcloud storage cat "${GCS_SHA256_URL}" 2>/dev/null | awk '{print $1}' || echo "")

if [[ -n "${EXPECTED_SHA}" ]]; then
    log "Path A: canonical artifact found in GCS. Pulling..."
    if [[ -f "${WEIGHTS_FILE}" ]] && verify_sha256 "${WEIGHTS_FILE}" "${EXPECTED_SHA}" 2>/dev/null; then
        log "Path A: ${WEIGHTS_FILE} already present and matches expected sha256. No-op."
    else
        log "Path A: downloading ${GCS_WEIGHTS_URL} to ${WEIGHTS_FILE}..."
        gcloud storage cp "${GCS_WEIGHTS_URL}" "${WEIGHTS_FILE}"
        verify_sha256 "${WEIGHTS_FILE}" "${EXPECTED_SHA}"
    fi

    # Pull tokenizer files too (vllm.service points to /data/weights/tokenizer)
    log "Path A: pulling tokenizer files from ${GCS_TOKENIZER_PREFIX}..."
    gcloud storage cp -r "${GCS_TOKENIZER_PREFIX}*" "${TOKENIZER_DIR}/" 2>/dev/null || \
        log "WARN: tokenizer files not in GCS (older bootstrap?); proceeding without"

    log "Path A: complete. ${WEIGHTS_FILE} ready (sha256=${EXPECTED_SHA})."
    setup_model_dir
    exit 0
fi

# ── Path B: First-boot ever — derive canonical artifact from authoritative source ──
log "Path B: GCS canonical artifact absent. Bootstrapping from AllenAI source."
log "This is a one-time operation; subsequent boots take Path A."

# Fetch HF token from instance metadata (project-level). Optional — the
# canonical AllenAI repo (allenai/Olmo-3-32B-Think) is PUBLIC + ungated, so
# anonymous download works. The token path is preserved in case future
# Foundry-specific gated models need it (e.g., a private mirror).
HF_TOKEN=$(curl -fsS --max-time 5 -H 'Metadata-Flavor: Google' \
    "http://metadata.google.internal/computeMetadata/v1/project/attributes/hf-token" 2>/dev/null || echo "")
if [[ -z "${HF_TOKEN}" ]]; then
    log "No HF token in project metadata — proceeding with anonymous download (HF_REPO=${HF_REPO} is public)."
else
    log "HF token found in project metadata; will authenticate before download."
fi

# Disk capacity check — peak disk during convert step is ~128GB
# (safetensors 64GB + intermediate fp16 GGUF 64GB before safetensors cleanup).
# 256GB pd-balanced disk has ~250GB usable; we need ≥140GB to be safe.
AVAIL_KB=$(df -k --output=avail "${WEIGHTS_DIR}" | tail -1)
AVAIL_GB=$((AVAIL_KB / 1024 / 1024))
log "Disk available at ${WEIGHTS_DIR}: ${AVAIL_GB} GB"
if [[ "${AVAIL_GB}" -lt 140 ]]; then
    log "FATAL: insufficient disk for safetensors+intermediate+final (need ≥140 GB free, have ${AVAIL_GB} GB)."
    exit 4
fi

# Step 1: download AllenAI safetensors
log "Step 1/5: downloading safetensors from ${HF_REPO} (~80 min, ~64 GB)..."
mkdir -p "${STAGING_DIR}"
# huggingface_hub >= 0.26 deprecated `huggingface-cli` and renamed it to `hf`.
# We use `hf download`. HF_TOKEN env is read transparently if set; pass an
# explicit --token only when set so anonymous downloads work for public repos.
HF_DOWNLOAD_ARGS=(download "${HF_REPO}"
    --local-dir "${STAGING_DIR}"
    --include="*.safetensors"
    --include="*.json"
    --include="tokenizer*"
    --include="*.txt")
[[ -n "${HF_TOKEN}" ]] && HF_DOWNLOAD_ARGS+=(--token "${HF_TOKEN}")
hf "${HF_DOWNLOAD_ARGS[@]}"

# Step 2: convert HF → GGUF fp16
# Use the venv python so transformers + safetensors imports succeed.
INTERMEDIATE_GGUF="${STAGING_DIR}/olmo-3-32b-think-fp16.gguf"
VENV_PYTHON="${VENV_PYTHON:-/opt/vllm/bin/python3}"
log "Step 2/5: converting HF safetensors → GGUF fp16 (~10 min)..."
"${VENV_PYTHON}" "${LLAMA_CPP_DIR}/convert_hf_to_gguf.py" \
    "${STAGING_DIR}" \
    --outfile "${INTERMEDIATE_GGUF}" \
    --outtype f16

# Step 3: quantize fp16 → Q3_K_M (fits within L4 22 GiB VRAM with llama-server)
log "Step 3/5: quantizing fp16 GGUF → Q3_K_M (~5 min)..."
"${LLAMA_CPP_DIR}/build/bin/llama-quantize" \
    "${INTERMEDIATE_GGUF}" \
    "${WEIGHTS_FILE}" \
    Q3_K_M

# Step 4: compute sha256, upload Q4_K_M + sha256 to GCS
ACTUAL_SHA=$(sha256sum "${WEIGHTS_FILE}" | awk '{print $1}')
log "Step 4/5: uploading Q3_K_M (sha256=${ACTUAL_SHA}) to ${GCS_WEIGHTS_URL}..."
gcloud storage cp "${WEIGHTS_FILE}" "${GCS_WEIGHTS_URL}"
echo "${ACTUAL_SHA}  olmo-3-32b-think-q3.gguf" | gcloud storage cp - "${GCS_SHA256_URL}"

# Save tokenizer files locally + upload to GCS so future Path A runs can grab them
log "Saving tokenizer files locally + uploading to GCS..."
for f in "${STAGING_DIR}"/tokenizer* "${STAGING_DIR}"/*.json; do
    [[ -e "${f}" ]] || continue
    cp "${f}" "${TOKENIZER_DIR}/"
    gcloud storage cp "${f}" "${GCS_TOKENIZER_PREFIX}$(basename "${f}")" 2>/dev/null || true
done

# Cleanup intermediates
log "Cleanup: removing staging dir + intermediate fp16 GGUF (frees disk for adapters + checkpoints)..."
rm -rf "${STAGING_DIR}"

# Step 5/5: OLMo 3 7B Think safetensors for QLoRA training (Tier A adapter production)
# 7B safetensors (~14 GB) fit cleanly in QLoRA 4-bit on L4 (24 GB VRAM).
TRAIN_HF_DIR="${WEIGHTS_DIR}/olmo-3-7b-think-hf"
GCS_TRAIN_PREFIX="gs://${GCS_BUCKET}/base-models/olmo-3-7b-think-hf"

if [[ -f "${TRAIN_HF_DIR}/.complete" ]]; then
    log "Step 5/5: 7B training weights already present at ${TRAIN_HF_DIR}."
else
    mkdir -p "${TRAIN_HF_DIR}"
    if gcloud storage ls "${GCS_TRAIN_PREFIX}/config.json" &>/dev/null; then
        log "Step 5/5: pulling 7B training weights from GCS (~5 min, ~14 GB)..."
        gcloud storage cp -r "${GCS_TRAIN_PREFIX}/*" "${TRAIN_HF_DIR}/"
    else
        log "Step 5/5: first-boot — downloading OLMo 3 7B Think from AllenAI (~20 min, ~14 GB)..."
        HF_DOWNLOAD_ARGS=(download "allenai/OLMo-3-1125-7B-Think"
            --local-dir "${TRAIN_HF_DIR}"
            --include="*.safetensors"
            --include="config.json"
            --include="tokenizer*")
        [[ -n "${HF_TOKEN}" ]] && HF_DOWNLOAD_ARGS+=(--token "${HF_TOKEN}")
        hf "${HF_DOWNLOAD_ARGS[@]}"
        gcloud storage cp -r "${TRAIN_HF_DIR}/" "${GCS_TRAIN_PREFIX}/"
        log "Step 5/5: 7B weights uploaded to GCS for future Yo-Yos."
    fi
    touch "${TRAIN_HF_DIR}/.complete"
    log "Step 5/5: 7B training weights ready at ${TRAIN_HF_DIR}."
fi

# Final verification
verify_sha256 "${WEIGHTS_FILE}" "${ACTUAL_SHA}"

setup_model_dir
log "Path B complete. Canonical Q3_K_M now in GCS; future boots take Path A."
log "Final state: ${WEIGHTS_FILE} ready (sha256=${ACTUAL_SHA}), tokenizer at ${TOKENIZER_DIR}/."
exit 0
