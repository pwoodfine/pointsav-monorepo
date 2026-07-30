#!/usr/bin/env bash
# Yo-Yo LoRA training trigger.
#
# Watches /srv/foundry/data/training-pending/ for *.json marker files dropped
# by the workspace's corpus-threshold.py. When a marker lands, runs QLoRA via
# accelerate against the corpus referenced in the marker, writes adapter
# artifacts to /data/weights/adapters/<tenant>/<role>/v<n>/, and triggers
# adapter-publish.service to upload the result to GCS.
#
# This unit is **disabled by default**. Master ratifies the first task-type
# promotion (Doctrine claim around apprenticeship + LoRA training threshold —
# currently a Doctrine gap surfaced separately to operator/Master). When that
# happens, `systemctl enable --now lora-training.service` activates the loop.
#
# Until activated, this script does not exist on the runtime path. The unit
# file is present so promotion is a single systemctl command, not an image
# rebuild.

set -euo pipefail

PENDING_DIR="${PENDING_DIR:-/srv/foundry/data/training-pending}"
ADAPTER_BASE="${ADAPTER_BASE:-/data/weights/adapters}"
LOG_FILE="/var/log/yoyo-lora-training.log"
BASE_MODEL="${BASE_MODEL:-/data/weights/olmo-3-32b-think-q4.gguf}"

mkdir -p "${ADAPTER_BASE}"
exec >> >(tee -a "${LOG_FILE}") 2>&1

log() { echo "[lora-training $(date -u +'%Y-%m-%dT%H:%M:%SZ')] $*"; }

log "Lora training watcher started. Pending dir: ${PENDING_DIR}"

while true; do
    # Skip if pending dir doesn't exist yet (workspace mount issue, etc.)
    if [[ ! -d "${PENDING_DIR}" ]]; then
        log "WARN: ${PENDING_DIR} absent. Sleeping 60s before retry."
        sleep 60
        continue
    fi

    # Find oldest unclaimed marker
    marker=$(find "${PENDING_DIR}" -maxdepth 1 -name '*.json' -not -name '*.claimed' \
        -printf '%T@ %p\n' 2>/dev/null | sort -n | head -1 | cut -d' ' -f2)

    if [[ -z "${marker}" ]]; then
        sleep 30
        continue
    fi

    # Claim it (atomic rename)
    claimed="${marker}.claimed"
    mv "${marker}" "${claimed}"
    log "Claimed marker: $(basename "${claimed}")"

    # Parse marker: { tenant, role, corpus_path, method (sft|dpo), tuple_count, version }
    tenant=$(jq -r '.tenant // empty' "${claimed}")
    role=$(jq -r '.role // empty' "${claimed}")
    corpus_path=$(jq -r '.corpus_path // empty' "${claimed}")
    method=$(jq -r '.method // "sft"' "${claimed}")
    version=$(jq -r '.version // 1' "${claimed}")

    if [[ -z "${tenant}" || -z "${role}" || -z "${corpus_path}" ]]; then
        log "ERROR: marker malformed. Skipping. Contents:"
        cat "${claimed}"
        mv "${claimed}" "${claimed}.invalid"
        continue
    fi

    out_dir="${ADAPTER_BASE}/${tenant}/${role}/v${version}"
    mkdir -p "${out_dir}"

    log "Training: tenant=${tenant} role=${role} method=${method} corpus=${corpus_path} → ${out_dir}"

    TRAIN_WEIGHTS_DIR="${TRAIN_WEIGHTS_DIR:-/data/weights/olmo-3-7b-think-hf}"
    GCS_BUCKET=$(curl -fsS --max-time 5 -H 'Metadata-Flavor: Google' \
        "http://metadata.google.internal/computeMetadata/v1/instance/attributes/weights-gcs-bucket" \
        2>/dev/null || echo "")

    # Pull corpus from GCS (workspace VM synced it before writing the marker)
    LOCAL_CORPUS="/tmp/training-corpus-${role}"
    mkdir -p "${LOCAL_CORPUS}"
    if [[ -n "${GCS_BUCKET}" ]]; then
        gcloud storage cp -r "gs://${GCS_BUCKET}/training-corpus/${role}/" "${LOCAL_CORPUS}/" \
            2>/dev/null || log "WARN: GCS corpus pull failed; using local if present"
    fi

    # Activate training venv (baked into image; pip fallback for first-run)
    source /opt/vllm/bin/activate
    python3 -c "import peft, bitsandbytes, accelerate, trl" 2>/dev/null || {
        log "Installing training deps (should be pre-baked; falling back to pip)..."
        pip install --quiet peft bitsandbytes accelerate trl datasets
    }

    case "${method}" in
        sft|dpo)
            log "QLoRA ${method}: model=${TRAIN_WEIGHTS_DIR} corpus=${LOCAL_CORPUS} → ${out_dir}"
            python3 - <<PYEOF
import json, sys, torch
from pathlib import Path
from datasets import Dataset
from transformers import AutoTokenizer, AutoModelForCausalLM, BitsAndBytesConfig
from peft import LoraConfig, get_peft_model, TaskType
from trl import SFTTrainer, SFTConfig

model_dir = "${TRAIN_WEIGHTS_DIR}"
corpus_dir = "${LOCAL_CORPUS}"
out_dir = "${out_dir}"

records = []
for fpath in sorted(Path(corpus_dir).rglob("*.jsonl")):
    try:
        with open(fpath) as f:
            for line in f:
                r = json.loads(line.strip())
                text = "\n".join(filter(None, [r.get("commit_msg"), r.get("actual_diff"),
                                               r.get("brief"), r.get("body")]))
                if text.strip():
                    records.append({"text": text[:2048]})
    except Exception as e:
        print(f"WARN: {fpath}: {e}")

if not records:
    print("No corpus records found — skipping training run.")
    sys.exit(0)

print(f"Corpus: {len(records)} tuples from {corpus_dir}")
bnb = BitsAndBytesConfig(load_in_4bit=True, bnb_4bit_quant_type="nf4",
    bnb_4bit_compute_dtype=torch.bfloat16, bnb_4bit_use_double_quant=True)
model = AutoModelForCausalLM.from_pretrained(model_dir, quantization_config=bnb,
    device_map="auto", trust_remote_code=True)
tokenizer = AutoTokenizer.from_pretrained(model_dir, trust_remote_code=True)
tokenizer.pad_token = tokenizer.eos_token
model.config.use_cache = False

lora = LoraConfig(r=16, lora_alpha=32, lora_dropout=0.05, bias="none",
    task_type=TaskType.CAUSAL_LM,
    target_modules=["q_proj","v_proj","k_proj","o_proj","gate_proj","up_proj","down_proj"])
model = get_peft_model(model, lora)
model.print_trainable_parameters()

trainer = SFTTrainer(
    model=model, tokenizer=tokenizer,
    train_dataset=Dataset.from_list(records),
    args=SFTConfig(
        output_dir=out_dir,
        num_train_epochs=2,
        per_device_train_batch_size=1,
        gradient_accumulation_steps=4,
        gradient_checkpointing=True,
        bf16=True, fp16=False,
        learning_rate=2e-4,
        warmup_ratio=0.03,
        max_seq_length=512,
        logging_steps=10,
        save_steps=100,
        report_to="none",
        dataloader_num_workers=0,
    )
)
trainer.train()
trainer.save_model(out_dir)
tokenizer.save_pretrained(out_dir)
print(f"Adapter saved → {out_dir}")
PYEOF
            ;;
        *)
            log "ERROR: unknown method '${method}' (expected sft|dpo)"
            mv "${claimed}" "${claimed}.invalid"
            continue
            ;;
    esac

    # Trigger adapter-publish to upload to GCS
    log "Triggering adapter-publish.service for ${out_dir}..."
    ADAPTER_OUT_DIR="${out_dir}" systemctl start adapter-publish.service || \
        log "WARN: adapter-publish trigger failed (unit may need to be configured)"

    # Mark marker as completed
    mv "${claimed}" "${claimed}.completed"
    log "Adapter v${version} for ${tenant}/${role} ready at ${out_dir}"
done
