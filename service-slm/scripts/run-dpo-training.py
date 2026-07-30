#!/usr/bin/env python3
"""
run-dpo-training.py — LoRA DPO fine-tuning for the apprenticeship adapter.

Reads DPO feedback pairs from a local directory (or GCS-synced path),
fine-tunes OLMo 7B Instruct with LoRA using HuggingFace TRL DPOTrainer,
and saves the adapter to the output path (optionally uploading to GCS).

Requirements (on trainer VM):
  pip install trl>=0.8 peft>=0.10 transformers>=4.40 datasets bitsandbytes

Usage:
  python3 run-dpo-training.py --corpus /path/to/feedback/
  python3 run-dpo-training.py --dry-run   # inspect corpus without training

GCS variant (Yo-Yo trainer VM, picking up marker):
  SLM_YOYO_WEIGHTS_GCS_BUCKET=woodfine-node-gcp-free-foundry-substrate \\
    python3 run-dpo-training.py --from-gcs --adapter apprenticeship-pointsav

Notes:
  - Default base model: /data/weights/olmo-3-7b-think-hf (OLMo 3 7B Think, pre-loaded on
    persistent weights disk by vllm-weights-prep.sh — no re-download needed each run)
  - adapter output goes to ./adapters/<adapter_name>-wip/ by default; daily cycle overrides
    to /data/weights/adapters/<name>/ (persistent disk, survives all VM cycles)
  - Workspace VM pulls adapter via rsync after training; workspace uploads to GCS (yoyo-batch
    lacks ADC — cannot write to GCS directly)
  - OLMo-only policy: never substitute a non-OLMo base model
"""

import argparse
import glob
import json
import os
import subprocess
import sys
import time

from pathlib import Path


FOUNDRY_ROOT = os.environ.get("FOUNDRY_ROOT", "/srv/foundry")
GCS_BUCKET = os.environ.get("SLM_YOYO_WEIGHTS_GCS_BUCKET", "")

# LoRA hyperparameters for OLMo 7B
# r=32/alpha=64: research on 7B extraction tasks shows meaningful gains over r=16 (clinical
# extraction: 10-20pt F1 improvement at r=32; r=16 underfits narrow-task preference signal).
# Adapter size doubles (~200 MB → ~400 MB) but stays well within persistent disk budget.
LORA_R = 32
LORA_ALPHA = 64
LORA_DROPOUT = 0.05
LORA_TARGET_MODULES = ["att_proj", "ff_proj", "ff_out", "attn_out"]
MAX_PROMPT_LENGTH = 512
BATCH_SIZE = 2
GRAD_ACCUM = 8   # raised 4→8; effective batch 16; damps gradient noise at low per-device batch
LEARNING_RATE = 2e-6   # lowered from 1e-5; 12-25× too hot vs OLMo 2 reference recipe (Tülu 3 = 8e-7..2e-6)
NUM_EPOCHS = 1   # lowered from 3; 3 epochs on single-task corpus → over-reinforcement collapse risk (Opus audit §17)
BETA = 0.1  # DPO default. Prior 0.5 justification (empty-"[]" rejected) is obsolete — those pairs are now filtered.


# Minimum rejected side length for DIFF pairs. Pairs below this are template stubs
# that teach the model "longer = better" rather than quality (Jun-14 audit finding).
MIN_REJECTED_CHARS_DIFF = 80

# Minimum rejected side length for ENRICHMENT pairs (source_type=datagraph-enrichment).
# Enrichment rejected sides are JSON entity arrays, not diffs — naturally short.
# A rejected side of `[]` is already caught by the empty check above; a rejected side
# of `[{"classification":"Account","entity_name":"outbox"}]` is ~55 chars and IS a
# valid training signal. A floor of 10 covers the minimum meaningful JSON entity object
# while excluding genuine empty/stub values.
# Jun-18 audit finding: 80-char floor was silently dropping ALL enrichment pairs,
# zeroing out the extraction-quality training signal reaching the LoRA trainer.
MIN_REJECTED_CHARS_ENTITIES = 10

# Maximum chosen/rejected length ratio. Must match corpus_gate.rs MAX_LENGTH_RATIO (8.0).
# Was 5.0 — that was too strict: dropped 77% of valid pairs (565/730). SimPO's length
# normalisation mitigates the ratio artifact so 8× is safe (Jun-15 6-agent audit).
MAX_LENGTH_RATIO = 8.0

# Template-echo prefixes on the rejected side indicating OLMo never executed.
# "<unified diff:" (with colon) is OLMo's placeholder stub — always rejected.
# "<unified diff" without colon may legitimately wrap a real diff; the
# _REAL_DIFF_MARKERS check below handles that case.
# Must stay in sync with corpus_gate.rs TEMPLATE_ECHO_PREFIXES.
TEMPLATE_ECHO_PREFIXES = (
    "<no diff provided",
    "<no changes",
    "<insert diff",
    "auto-reject: olmo-attempt-below-senior-standard",
    "auto-reject:",
    "<unified diff:",  # colon = OLMo stub (Jun-15 audit: 324/861 pairs; 85 passed all gates)
)

# Markers that indicate the rejected field contains real diff content even if it
# starts with a template prefix like "<unified diff>".
_REAL_DIFF_MARKERS = ("diff --git", "--- a/", "+++ b/", "@@ ")


def load_feedback_files(corpus_path: str) -> list[dict]:
    """Load DPO pairs from corpus_path.

    Reads all pair files:
    - apprenticeship-*.jsonl: git-commit shadow captures (chosen=operator diff, rejected=OLMo diff)
    - enrichment-*.jsonl: DataGraph disagreement pairs (chosen=Tier B, rejected=Tier A)

    Filters applied (in order):
    - Skips pairs where rejected is empty ("[]") — degenerate.
    - Skips pairs where rejected is a template-echo placeholder.
    - Skips pairs where rejected is shorter than MIN_REJECTED_CHARS.
    - Skips pairs where chosen/rejected length ratio exceeds MAX_LENGTH_RATIO.
    - Skips pairs where auto_verdict=False (explicitly rejected by pipeline).
    """
    files = []
    for pat in ["apprenticeship-*.jsonl", "enrichment-*.jsonl"]:
        files.extend(glob.glob(os.path.join(corpus_path, pat)))
    # service-content writes enrichment pairs to SERVICE_CONTENT_BASE_DIR — a
    # different directory from the apprenticeship corpus path. Scan it too so
    # enrichment DPO pairs (Tier B entity disagreements) reach the trainer.
    # Confirmed split-brain by 6-agent audit 2026-06-15: 8 enrichment pairs orphaned.
    sc_base = os.environ.get(
        "SERVICE_CONTENT_BASE_DIR",
        "/home/mathew/deployments/woodfine-fleet-deployment/cluster-totebox-jennifer/service-fs/data",
    )
    enrichment_dir = os.path.join(sc_base, "training-corpus", "feedback")
    if os.path.isdir(enrichment_dir) and os.path.abspath(enrichment_dir) != os.path.abspath(corpus_path):
        extra = glob.glob(os.path.join(enrichment_dir, "enrichment-*.jsonl"))
        if extra:
            print(f"[corpus] +{len(extra)} enrichment pairs from SERVICE_CONTENT_BASE_DIR")
            files.extend(extra)
    files = sorted(set(files))
    print(f"[corpus] found {len(files)} pair files total")
    records = []
    skipped_format = 0
    skipped_empty_rejected = 0
    skipped_template_echo = 0
    skipped_too_short = 0
    skipped_ratio = 0
    skipped_verdict = 0
    ratio_sum = 0.0
    ratio_count = 0
    for f in files:
        try:
            d = json.load(open(f))
        except Exception as e:
            print(f"[WARN] skip {f}: {e}", file=sys.stderr)
            skipped_format += 1
            continue
        if not d.get("prompt") or not d.get("chosen"):
            skipped_format += 1
            continue
        rejected = d.get("rejected", "")
        chosen = d.get("chosen", "")
        # Skip degenerate pairs where rejected returned nothing
        if not rejected or rejected == "[]":
            skipped_empty_rejected += 1
            continue
        # Skip template-echo placeholders (OLMo never executed).
        # Rule 1: hard prefix match on known sentinel strings.
        # Rule 2: "<unified diff" prefix is ONLY a placeholder when no real diff
        # markers follow; OLMo legitimately wraps real diffs with that header.
        rejected_lc = rejected.strip().lower()
        is_echo = any(rejected_lc.startswith(p) for p in TEMPLATE_ECHO_PREFIXES)
        if not is_echo and rejected_lc.startswith("<unified diff"):
            is_echo = not any(m in rejected for m in _REAL_DIFF_MARKERS)
        if is_echo:
            skipped_template_echo += 1
            continue
        # Skip pairs where the rejected side is too short to carry preference signal.
        # Enrichment pairs (entity JSON arrays) use a lower floor than diff pairs.
        source_type = d.get("source_type", "")
        is_enrichment = source_type == "datagraph-enrichment"
        min_chars = MIN_REJECTED_CHARS_ENTITIES if is_enrichment else MIN_REJECTED_CHARS_DIFF
        if len(rejected) < min_chars:
            skipped_too_short += 1
            continue
        # Skip pairs with extreme length ratio (teaches length, not quality)
        if len(rejected) > 0:
            ratio = len(chosen) / len(rejected)
            ratio_sum += ratio
            ratio_count += 1
            if ratio > MAX_LENGTH_RATIO:
                skipped_ratio += 1
                continue
        # Skip pairs explicitly rejected by verdict pipeline
        verdict = d.get("auto_verdict")
        if verdict is not None and verdict is not True:
            skipped_verdict += 1
            continue
        # Conversational format: TRL applies OLMo-2 chat template correctly, avoiding
        # the "Mismatch between tokenized prompt and start of tokenized prompt+chosen"
        # warnings that fire with raw-string format (EOS token handling in standalone vs
        # concatenated tokenization differs, breaking DPO loss boundary detection).
        records.append({
            "prompt":   [{"role": "user",      "content": d["prompt"]}],
            "chosen":   [{"role": "assistant", "content": chosen}],
            "rejected": [{"role": "assistant", "content": rejected}],
        })
    avg_ratio = ratio_sum / ratio_count if ratio_count > 0 else 0.0
    print(
        f"[corpus] loaded {len(records)} DPO pairs "
        f"(format-skip={skipped_format} empty={skipped_empty_rejected} "
        f"template-echo={skipped_template_echo} too-short={skipped_too_short} "
        f"ratio>{MAX_LENGTH_RATIO:.0f}x={skipped_ratio} verdict={skipped_verdict}) "
        f"avg_ratio={avg_ratio:.1f}x"
    )
    return records


def sync_from_gcs(adapter_name: str, local_corpus: str) -> str:
    """Sync apprenticeship corpus from GCS to local path. Returns local corpus path."""
    if not GCS_BUCKET:
        print("[ERROR] SLM_YOYO_WEIGHTS_GCS_BUCKET not set", file=sys.stderr)
        sys.exit(1)
    feedback_path = os.path.join(local_corpus, "feedback")
    os.makedirs(feedback_path, exist_ok=True)
    print(f"[gcs] syncing corpus from gs://{GCS_BUCKET}/training-corpus/apprenticeship/ ...")
    subprocess.run(
        ["gcloud", "storage", "cp", "-r",
         f"gs://{GCS_BUCKET}/training-corpus/apprenticeship/",
         os.path.join(local_corpus, "apprenticeship/")],
        check=True,
    )
    # Feedback pairs are under apprenticeship/../feedback/ on GCS
    subprocess.run(
        ["gcloud", "storage", "cp", "-r",
         f"gs://{GCS_BUCKET}/training-corpus/feedback/",
         feedback_path + "/"],
        check=True,
    )
    return feedback_path


def upload_adapter_to_gcs(adapter_path: str, adapter_name: str) -> None:
    """Upload adapter to GCS.

    yoyo-batch VMs do not have GCS write permissions — only the workspace VM
    holds ADC with cloud-platform scope. The startup script on yoyo-batch
    must rsync the adapter to the workspace VM which then runs this upload,
    OR the workspace pulls the adapter via rsync and uploads directly.

    When run directly on the workspace VM (e.g. during testing), this works
    without any special setup.
    """
    if not GCS_BUCKET:
        return
    gcs_dest = f"gs://{GCS_BUCKET}/adapters/{os.path.basename(adapter_path)}/"
    print(f"[gcs] uploading adapter → {gcs_dest}")
    try:
        subprocess.run(
            ["gcloud", "storage", "rsync", adapter_path + "/", gcs_dest],
            check=True,
        )
        print(f"[gcs] adapter uploaded: {gcs_dest}")
    except subprocess.CalledProcessError as e:
        print(f"[gcs] upload failed (likely no ADC on this VM): {e}")
        print(f"[gcs] adapter saved locally at: {adapter_path}")
        print(f"[gcs] pull from workspace: rsync -a yoyo-batch:{adapter_path}/ /tmp/adapter/ then upload")


def run_training(records: list[dict], base_model: str, output_dir: str, dry_run: bool,
                 max_runtime_seconds: int = 0, resume: bool = False,
                 loss_type: str = "simpo", simpo_gamma: float = 0.5) -> None:
    """Fine-tune base_model with DPO or SimPO on records, save adapter to output_dir.

    loss_type='simpo' (default): uses SimPOTrainer + SimPOConfig. Eliminates the
    reference-model log-probability term that causes length discrimination in standard
    DPO. SimPO directly maximises the average log-prob margin without normalising by
    the reference model, so the reward signal is insensitive to sequence length.

    loss_type='dpo': standard DPO with implicit reference (PEFT base model). Use for
    ablation comparisons against SimPO runs.
    """
    print(f"[train] base model: {base_model}")
    print(f"[train] output dir: {output_dir}")
    print(f"[train] pairs:      {len(records)}")
    print(f"[train] loss type:  {loss_type} (gamma={simpo_gamma} if simpo)")
    print(f"[train] LoRA r={LORA_R} alpha={LORA_ALPHA} beta={BETA}")
    if max_runtime_seconds:
        print(f"[train] runtime cap: {max_runtime_seconds}s ({max_runtime_seconds // 3600}h {(max_runtime_seconds % 3600) // 60}m)")
    if resume:
        import glob as _glob
        checkpoints = sorted(_glob.glob(os.path.join(output_dir, "checkpoint-*")))
        if checkpoints:
            print(f"[train] resuming from checkpoint: {checkpoints[-1]}")
        else:
            print(f"[train] --resume set but no checkpoint found in {output_dir} — starting fresh")

    if dry_run:
        print("[train] DRY-RUN — skipping actual training")
        return

    # Import training libraries (only needed at training time)
    try:
        import torch
        from datasets import Dataset
        from peft import LoraConfig, TaskType, get_peft_model
        from transformers import AutoModelForCausalLM, AutoTokenizer, BitsAndBytesConfig, TrainerCallback
        from trl import DPOConfig, DPOTrainer
        if loss_type == "simpo":
            try:
                from trl import SimPOConfig, SimPOTrainer
            except ImportError:
                print("[WARN] SimPOConfig not found in this trl version — falling back to DPO loss", file=sys.stderr)
                print("[WARN] To enable SimPO: pip install --upgrade trl>=1.4", file=sys.stderr)
                loss_type = "dpo"
    except ImportError as e:
        print(f"[ERROR] Missing training library: {e}", file=sys.stderr)
        print("Install: pip install trl peft transformers datasets bitsandbytes", file=sys.stderr)
        sys.exit(1)

    os.makedirs(output_dir, exist_ok=True)

    print(f"[train] CUDA available: {torch.cuda.is_available()}")
    if torch.cuda.is_available():
        print(f"[train] GPU: {torch.cuda.get_device_name(0)}")

    # 4-bit quantization for memory efficiency
    bnb_config = BitsAndBytesConfig(
        load_in_4bit=True,
        bnb_4bit_quant_type="nf4",
        bnb_4bit_compute_dtype=torch.bfloat16,
        bnb_4bit_use_double_quant=True,
    )

    print("[train] loading tokenizer ...")
    tokenizer = AutoTokenizer.from_pretrained(base_model, trust_remote_code=True)
    if tokenizer.pad_token is None:
        tokenizer.pad_token = tokenizer.eos_token
    tokenizer.padding_side = "right"  # prevents causal mask misalignment with OLMo-2 DPO batches

    print("[train] loading model (4-bit) ...")
    model = AutoModelForCausalLM.from_pretrained(
        base_model,
        quantization_config=bnb_config,
        device_map="auto",
        trust_remote_code=True,
    )
    model.config.use_cache = False

    # Startup assertion: verify target_modules exist in this model before
    # peft applies them. OLMo 2 uses att_proj/ff_proj/ff_out/attn_out;
    # LLaMA names (q_proj/v_proj etc.) silently attach to zero modules.
    _model_module_names = {name.split(".")[-1] for name, _ in model.named_modules()}
    _matched = [m for m in LORA_TARGET_MODULES if m in _model_module_names]
    if not _matched:
        print(
            f"[ERROR] LORA_TARGET_MODULES {LORA_TARGET_MODULES} matched 0 modules in model.\n"
            f"        Model leaf module names (sample): {sorted(_model_module_names)[:20]}\n"
            f"        Training would produce a no-op adapter. Aborting.",
            file=sys.stderr,
        )
        sys.exit(1)
    print(f"[train] LoRA target assertion: {len(_matched)}/{len(LORA_TARGET_MODULES)} modules matched: {_matched}")

    peft_config = LoraConfig(
        task_type=TaskType.CAUSAL_LM,
        r=LORA_R,
        lora_alpha=LORA_ALPHA,
        lora_dropout=LORA_DROPOUT,
        target_modules=LORA_TARGET_MODULES,
        bias="none",
    )

    # Runtime cap callback — saves checkpoint and stops training cleanly if wall-clock limit hit
    class RuntimeCapCallback(TrainerCallback):
        def __init__(self, max_seconds: int, output_dir: str) -> None:
            self._start = time.monotonic()
            self._max = max_seconds
            self._out = output_dir

        def on_step_end(self, args, state, control, **kwargs):
            if self._max and (time.monotonic() - self._start) >= self._max:
                elapsed = int(time.monotonic() - self._start)
                print(f"[train] runtime cap reached ({elapsed}s >= {self._max}s) — saving checkpoint and stopping")
                control.should_save = True
                control.should_training_stop = True

    dataset = Dataset.from_list(records)
    split = dataset.train_test_split(test_size=0.1, seed=42)

    # Memory guardrails: DPO double-forward (policy + ref) exhausts 24 GB L4 at MAX_LENGTH=1024
    # even for 7B in 4-bit (21.97 GiB observed). Apply gradient checkpointing + short sequences
    # for all model sizes; 32B gets smaller batch on top.
    # SimPO uses a single forward (no reference model) so memory is ~half of DPO — same cap
    # kept for safety since gradient checkpointing is cheap.
    is_32b = "32B" in base_model or "32b" in base_model
    _batch_size = 1 if is_32b else BATCH_SIZE
    _grad_accum = 4 if is_32b else GRAD_ACCUM
    _max_length = 512  # 1024 OOMs on L4 24 GB; 512 fits 7B DPO double-forward with grad ckpt
    if is_32b:
        print(f"[train] 32B memory mode: batch=1, grad_ckpt=True, max_len={_max_length}, grad_accum={_grad_accum}")
    else:
        print(f"[train] 7B memory mode: batch={_batch_size}, grad_ckpt=True, max_len={_max_length}")

    # expandable_segments avoids fragmentation-caused OOM on CUDA
    os.environ.setdefault("PYTORCH_CUDA_ALLOC_CONF", "expandable_segments:True")

    callbacks = []
    if max_runtime_seconds:
        callbacks.append(RuntimeCapCallback(max_runtime_seconds, output_dir))

    if loss_type == "simpo":
        # SimPO: no reference model needed; uses average log-prob per token with a margin
        # (gamma). Directly addresses the length-discrimination artifact in standard DPO
        # (Jun-14 audit finding: logps/chosen −1592 vs logps/rejected −238 = 6.7× gap).
        training_args = SimPOConfig(
            output_dir=output_dir,
            num_train_epochs=NUM_EPOCHS,
            per_device_train_batch_size=_batch_size,
            gradient_accumulation_steps=_grad_accum,
            gradient_checkpointing=True,
            gradient_checkpointing_kwargs={"use_reentrant": False},
            learning_rate=LEARNING_RATE,
            gamma=simpo_gamma,
            max_length=_max_length,
            logging_steps=5,
            save_steps=5,
            save_total_limit=2,
            eval_strategy="steps",
            eval_steps=5,
            report_to="none",
            bf16=torch.cuda.is_available(),
            remove_unused_columns=False,
        )
        trainer = SimPOTrainer(
            model=model,
            args=training_args,
            train_dataset=split["train"],
            eval_dataset=split["test"],
            processing_class=tokenizer,
            peft_config=peft_config,
            callbacks=callbacks or None,
        )
    else:
        training_args = DPOConfig(
            output_dir=output_dir,
            num_train_epochs=NUM_EPOCHS,
            per_device_train_batch_size=_batch_size,
            gradient_accumulation_steps=_grad_accum,
            gradient_checkpointing=True,  # required for 7B DPO on L4 24 GB; was OOMing without it
            gradient_checkpointing_kwargs={"use_reentrant": False},  # prevents silent zero-grad on transformers 5.x (TRL #2486)
            learning_rate=LEARNING_RATE,
            beta=BETA,
            loss_type="sigmoid_norm",  # length-normalised DPO — prevents "longer = better" bias
            # (chosen=populated JSON, rejected=[] creates extreme length imbalance;
            # sigmoid_norm divides log-prob by sequence length before computing loss.
            # Added per OLMo/Tülu 3 training playbook; native TRL support since v0.9.)
            max_length=_max_length,
            logging_steps=5,
            save_steps=5,        # checkpoint every 5 steps (corpus is small; 50 was never reached in 1 epoch)
            save_total_limit=2,  # keep only 2 most recent; avoids disk fill on spot VM across days
            eval_strategy="steps",
            eval_steps=5,
            report_to="none",
            bf16=torch.cuda.is_available(),
            remove_unused_columns=False,
        )
        trainer = DPOTrainer(
            model=model,
            ref_model=None,  # uses implicit reference (PEFT base model)
            args=training_args,
            train_dataset=split["train"],
            eval_dataset=split["test"],
            processing_class=tokenizer,
            peft_config=peft_config,
            callbacks=callbacks or None,
        )

    print(f"[train] starting DPO training on {len(split['train'])} pairs ...")
    resume_ckpt = None
    if resume:
        checkpoints = sorted(glob.glob(os.path.join(output_dir, "checkpoint-*")))
        if checkpoints:
            candidate = checkpoints[-1]
            # Staleness guard: if the checkpoint is from a completed run (epoch >= 1.0),
            # do NOT resume — that would skip training entirely (observed: train_loss=0
            # in 10ms when checkpoint-49 had epoch=1.0 from a prior completed cycle).
            # Only mid-run checkpoints (epoch < 1.0) are valid resume targets.
            state_file = os.path.join(candidate, "trainer_state.json")
            stale = False
            if os.path.exists(state_file):
                try:
                    import json as _json_local
                    with open(state_file) as _sf:
                        _state = _json_local.load(_sf)
                    ckpt_epoch = _state.get("epoch", 0)
                    if ckpt_epoch >= 1.0:
                        print(f"[train] checkpoint {os.path.basename(candidate)} is from a "
                              f"completed run (epoch={ckpt_epoch:.2f}) — starting fresh",
                              file=sys.stderr)
                        stale = True
                except Exception as _e:
                    print(f"[train] could not read trainer_state.json ({_e}) — starting fresh",
                          file=sys.stderr)
                    stale = True
            if not stale:
                resume_ckpt = candidate
                print(f"[train] resuming from checkpoint: {resume_ckpt}")
            else:
                print(f"[train] no valid resume checkpoint — starting fresh")
        else:
            print(f"[train] no checkpoint in {output_dir} — starting fresh")
    trainer.train(resume_from_checkpoint=resume_ckpt)

    print(f"[train] saving adapter to {output_dir}")
    trainer.save_model(output_dir)
    tokenizer.save_pretrained(output_dir)
    print("[train] done")


def main() -> None:
    parser = argparse.ArgumentParser(description="LoRA DPO training for apprenticeship adapter")
    parser.add_argument("--corpus", default=os.path.join(FOUNDRY_ROOT, "data", "training-corpus", "feedback"),
                        help="Path to feedback/ directory containing apprenticeship-*.jsonl files")
    parser.add_argument("--base-model", default="/data/weights/olmo-3-7b-think-hf",
                        help="HuggingFace model ID or local path for the OLMo base model (OLMo-only policy). "
                             "Default points to OLMo 3 7B Think weights pre-loaded on the yoyo-batch "
                             "persistent weights disk by vllm-weights-prep.sh — avoids re-downloading.")
    parser.add_argument("--adapter-name", default="apprenticeship-pointsav",
                        help="Name for the output adapter")
    parser.add_argument("--output-dir", default=None,
                        help="Override output directory (default: ./adapters/<name>-<date>)")
    parser.add_argument("--from-gcs", action="store_true",
                        help="Sync corpus from GCS before training")
    parser.add_argument("--upload-gcs", action="store_true",
                        help="Upload trained adapter to GCS after training")
    parser.add_argument("--dry-run", action="store_true",
                        help="Load corpus and report counts without training")
    parser.add_argument("--max-runtime-seconds", type=int, default=7200,
                        help="Wall-clock training limit in seconds (default: 7200 = 2h). "
                             "Saves checkpoint and exits cleanly when reached. 0 = no cap.")
    parser.add_argument("--resume", action="store_true",
                        help="Resume training from the latest checkpoint in output_dir. "
                             "Pass on every daily run to accumulate training incrementally.")
    parser.add_argument("--loss-type", default="simpo", choices=["simpo", "dpo"],
                        help="Preference learning objective. 'simpo' (default) avoids the "
                             "reference-model length-normalisation bias that caused token-count "
                             "discrimination in the Jun-14 run. 'dpo' for ablation comparison.")
    parser.add_argument("--simpo-gamma", type=float, default=0.5,
                        help="SimPO margin (gamma). Default 0.5. Increase to widen the "
                             "reward margin between chosen and rejected; decrease if training "
                             "is unstable on small corpora. Ignored when --loss-type=dpo.")
    args = parser.parse_args()

    corpus_path = args.corpus
    if args.from_gcs:
        local_staging = "/tmp/foundry-training-corpus"
        corpus_path = sync_from_gcs(args.adapter_name, local_staging)

    records = load_feedback_files(corpus_path)
    if not records:
        print("[ERROR] No valid DPO pairs found — check corpus path and field names", file=sys.stderr)
        sys.exit(1)

    # Use a fixed -wip suffix so --resume finds the same checkpoint directory each day.
    # Only rename to a dated path when promoting the adapter to the registry.
    output_dir = args.output_dir or f"./adapters/{args.adapter_name}-wip"

    run_training(records, args.base_model, output_dir, dry_run=args.dry_run,
                 max_runtime_seconds=args.max_runtime_seconds,
                 resume=args.resume,
                 loss_type=args.loss_type,
                 simpo_gamma=args.simpo_gamma)

    if args.upload_gcs and not args.dry_run:
        upload_adapter_to_gcs(output_dir, args.adapter_name)

    if not args.dry_run:
        print(f"\n[done] adapter at: {output_dir}")
        if GCS_BUCKET and args.upload_gcs:
            print(f"[done] GCS: gs://{GCS_BUCKET}/adapters/{os.path.basename(output_dir)}/")


if __name__ == "__main__":
    main()
