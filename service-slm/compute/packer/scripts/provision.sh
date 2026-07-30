#!/usr/bin/env bash
# Packer provisioner for the slm-yoyo GCE image.
# Installs: CUDA 12, Python 3.12, vLLM >= 0.12, Nginx TLS reverse proxy,
# llama.cpp (built from source for our own quantization), training libs
# (torch/peft/bitsandbytes/transformers/huggingface_hub/accelerate),
# google-cloud-sdk + gcsfuse, and the Yo-Yo lifecycle systemd units
# (vllm-weights-prep, lora-training [disabled], adapter-publish).
# Idempotent -- safe to re-run.
set -euo pipefail

VLLM_PORT=${VLLM_PORT:-8000}
LLAMA_CPP_REF=${LLAMA_CPP_REF:-master}   # pinned commit can be set via env

echo "==> Installing system packages"
sudo apt-get update -qq
sudo apt-get install -y --no-install-recommends \
    curl gnupg ca-certificates jq \
    nginx openssl \
    python3 python3-pip python3-venv \
    cmake git \
    apt-transport-https

# -- Kernel headers + build tools (required for DKMS / NVIDIA module compile) --
echo "==> Installing kernel headers and build tools"
sudo apt-get install -y --no-install-recommends \
    linux-headers-$(uname -r) \
    build-essential \
    dkms

# -- google-cloud-sdk (for gcloud + gsutil + gcsfuse) --------------------------
echo "==> Installing google-cloud-sdk + gcsfuse"
echo "deb [signed-by=/usr/share/keyrings/cloud.google.gpg] https://packages.cloud.google.com/apt cloud-sdk main" \
    | sudo tee /etc/apt/sources.list.d/google-cloud-sdk.list
echo "deb [signed-by=/usr/share/keyrings/cloud.google.gpg] https://packages.cloud.google.com/apt gcsfuse-noble main" \
    | sudo tee /etc/apt/sources.list.d/gcsfuse.list
curl -fsSL https://packages.cloud.google.com/apt/doc/apt-key.gpg \
    | sudo gpg --dearmor -o /usr/share/keyrings/cloud.google.gpg
sudo apt-get update -qq
sudo apt-get install -y --no-install-recommends google-cloud-sdk gcsfuse

# -- CUDA ----------------------------------------------------------------------
echo "==> Installing CUDA keyring"
CUDA_KEYRING_URL="https://developer.download.nvidia.com/compute/cuda/repos/ubuntu2404/x86_64/cuda-keyring_1.1-1_all.deb"
curl -fsSL "${CUDA_KEYRING_URL}" -o /tmp/cuda-keyring.deb
sudo dpkg -i /tmp/cuda-keyring.deb
rm /tmp/cuda-keyring.deb

echo "==> Installing CUDA drivers + build toolkit (L4 / Ada Lovelace)"
sudo apt-get update -qq
sudo apt-get install -y --no-install-recommends \
    cuda-drivers \
    cuda-nvcc-12-6 \
    libcublas-dev-12-6

# -- vLLM + training libs ------------------------------------------------------
# Single venv at /opt/vllm carries both vLLM (inference) and the training
# stack (torch + peft + bitsandbytes + transformers + huggingface_hub + accelerate).
# vLLM pins its own torch; pip resolves a compatible set.
echo "==> Installing vLLM + training libraries into /opt/vllm venv"
sudo python3 -m venv /opt/vllm
sudo /opt/vllm/bin/pip install --upgrade pip wheel
sudo /opt/vllm/bin/pip install \
    "vllm>=0.12" \
    "peft>=0.14" \
    "bitsandbytes>=0.45" \
    "transformers>=4.46" \
    "huggingface_hub>=0.25" \
    "accelerate>=1.0" \
    "trl>=0.12.0" \
    "datasets>=3.0.0" \
    "sentencepiece" \
    "protobuf"

# -- llama.cpp (built from source, CUDA-enabled) ------------------------------
# We build both llama-quantize (weights bootstrap) and llama-server (inference).
# CUDA (SM 89 / L4 Ada Lovelace) is required for llama-server GPU inference.
# nvcc compiles without a GPU present; the GPU is only needed at runtime.
echo "==> Cloning + building llama.cpp at ref=${LLAMA_CPP_REF}"
sudo git clone https://github.com/ggerganov/llama.cpp /opt/llama.cpp
sudo git -C /opt/llama.cpp checkout "${LLAMA_CPP_REF}"
sudo cmake -B /opt/llama.cpp/build -S /opt/llama.cpp \
    -DGGML_CUDA=ON -DLLAMA_CURL=OFF \
    -DCMAKE_CUDA_ARCHITECTURES=89 \
    -DCMAKE_BUILD_TYPE=Release \
    -DCMAKE_CUDA_COMPILER=/usr/local/cuda-12.6/bin/nvcc
sudo cmake --build /opt/llama.cpp/build \
    --target llama-quantize llama-server -j"$(nproc)"

# Make binaries available on PATH
sudo install -m 755 /opt/llama.cpp/build/bin/llama-quantize /usr/local/bin/llama-quantize
sudo install -m 755 /opt/llama.cpp/build/bin/llama-server /usr/local/bin/llama-server

# Register CUDA 12.6 libs with ldconfig so libcublas.so.12 is found at runtime.
echo '/usr/local/cuda-12.6/targets/x86_64-linux/lib' \
    | sudo tee /etc/ld.so.conf.d/cuda-12-6.conf > /dev/null
sudo ldconfig

# `hf` (huggingface_hub >= 0.26 CLI; supersedes the deprecated `huggingface-cli`)
# is provided by huggingface_hub in the venv; symlink for global access.
sudo ln -sf /opt/vllm/bin/hf /usr/local/bin/hf

# -- systemd units -------------------------------------------------------------
echo "==> Installing systemd units"
sudo install -m 644 /tmp/vllm.service /etc/systemd/system/vllm.service
sudo install -m 644 /tmp/vllm-weights-prep.service /etc/systemd/system/vllm-weights-prep.service
sudo install -m 644 /tmp/llama-server.service /etc/systemd/system/llama-server.service
sudo install -m 644 /tmp/lora-training.service /etc/systemd/system/lora-training.service
sudo install -m 644 /tmp/adapter-publish.service /etc/systemd/system/adapter-publish.service

echo "==> Installing lifecycle scripts"
sudo install -m 755 /tmp/vllm-weights-prep.sh /usr/local/bin/vllm-weights-prep.sh
sudo install -m 755 /tmp/lora-training.sh /usr/local/bin/lora-training.sh
sudo install -m 755 /tmp/adapter-publish.sh /usr/local/bin/adapter-publish.sh

# Enable units that should run at boot.
# vllm-weights-prep downloads weights before llama-server starts (Requires/After).
# vllm.service is installed but NOT enabled — llama-server.service handles inference.
# lora-training is intentionally NOT enabled — it activates after Master
# ratifies the Yo-Yo-runs-LoRA-training Doctrine claim.
# adapter-publish is on-demand (no [Install] section).
sudo systemctl enable vllm-weights-prep.service
sudo systemctl enable llama-server.service

# -- Nginx TLS -----------------------------------------------------------------
echo "==> Generating self-signed TLS certificate for Nginx"
sudo mkdir -p /etc/nginx/ssl
sudo openssl req -x509 -nodes -newkey rsa:4096 -days 3650 \
    -keyout /etc/nginx/ssl/yoyo.key \
    -out    /etc/nginx/ssl/yoyo.crt \
    -subj   "/CN=yoyo-tier-b.internal"

echo "==> Installing nginx-yoyo.conf"
sudo install -m 644 /tmp/nginx-yoyo.conf /etc/nginx/conf.d/yoyo.conf
sudo rm -f /etc/nginx/sites-enabled/default

# map_hash_bucket_size in its own file so rc.local can overwrite yoyo-auth-map.conf
# at boot (with the real bearer token) without losing the directive.
sudo tee /etc/nginx/conf.d/map-hash-bucket.conf > /dev/null << 'BUCKETEOF'
map_hash_bucket_size 128;
BUCKETEOF

# Default deny-all auth map — rc.local overwrites this at boot with the real token.
sudo tee /etc/nginx/conf.d/yoyo-auth-map.conf > /dev/null << 'MAPEOF'
map $http_authorization $auth_ok {
    default 0;
}
MAPEOF

sudo systemctl enable nginx

# -- Data disk mount points ----------------------------------------------------
echo "==> Creating /data/weights and /training mount points"
sudo mkdir -p /data/weights /training /srv/foundry-substrate

# Startup script: mount the persistent weights disk on boot.
# /training disk (if attached) and /srv/foundry-substrate (gcsfuse) are
# best-effort; absent disks/buckets do not block boot.
sudo tee /etc/rc.local > /dev/null << 'EOF'
#!/usr/bin/env bash
# Mount persistent weights disk on boot (attached by start-yoyo.sh / OpenTofu).
DEVICE=/dev/disk/by-id/google-yoyo-weights
MOUNTPOINT=/data/weights
if [ -b "${DEVICE}" ] && ! mountpoint -q "${MOUNTPOINT}"; then
    if ! blkid "${DEVICE}" > /dev/null 2>&1; then
        mkfs.ext4 -F "${DEVICE}"
    fi
    mount "${DEVICE}" "${MOUNTPOINT}"
fi

# Optional: mount /training scratch disk if present (best-effort).
TRAIN_DEVICE=/dev/disk/by-id/google-yoyo-training
TRAIN_MOUNTPOINT=/training
if [ -b "${TRAIN_DEVICE}" ] && ! mountpoint -q "${TRAIN_MOUNTPOINT}"; then
    if ! blkid "${TRAIN_DEVICE}" > /dev/null 2>&1; then
        mkfs.ext4 -F "${TRAIN_DEVICE}"
    fi
    mount "${TRAIN_DEVICE}" "${TRAIN_MOUNTPOINT}" || true
fi

# Bearer token: retrieve from instance metadata, write to nginx auth map.
TOKEN=$(curl -sf -H "Metadata-Flavor: Google" \
    "http://metadata.google.internal/computeMetadata/v1/instance/attributes/bearer-token" || true)
if [ -n "${TOKEN}" ]; then
    printf 'map $http_authorization $auth_ok {\n  default 0;\n  "Bearer %s" 1;\n}\n' "${TOKEN}" \
        > /etc/nginx/conf.d/yoyo-auth-map.conf
    systemctl reload nginx || true
fi

exit 0
EOF
sudo chmod +x /etc/rc.local

echo "==> provision.sh complete"
