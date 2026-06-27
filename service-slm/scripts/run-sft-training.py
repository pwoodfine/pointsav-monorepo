#!/usr/bin/env python3
"""
run-sft-training.py — LoRA SFT fine-tuning for the apprenticeship adapter.

SFT-first is the correct primary training path at <5K pair volume.
DPO requires ~1-3K clean contrastive pairs; at the current scale (122-192 pairs)
it is below the stable floor. SFT on 2,343 single-sided ground-truth pairs is
the correct first step. (Per BRIEF-slm-learning-loop.md §9/§10 and 6-agent
Opus audit 2026-06-15.)

Source: shadow brief queue-done dir.
Each *.brief.jsonl is a ShadowQueueEntry:
  {"brief": {...}, "actual_diff": "<real committed diff>"}

Output: Alpaca-style SFT pairs:
  ### Instruction:
  <brief.body + scope + acceptance_test>

  ### Response:
  <actual_diff>

Usage:
  python3 run-sft-training.py --dry-run   # inspect corpus without training
  python3 run-sft-training.py             # train SFT adapter

Requirements (on trainer VM):
  pip install trl>=0.8 peft>=0.10 transformers>=4.40 datasets bitsandbytes

OLMo-only policy: never substitute a non-OLMo base model.
"""

import argparse
import glob
import json
import os
import sys
import time

from pathlib import Path


FOUNDRY_ROOT = os.environ.get("FOUNDRY_ROOT", "/srv/foundry")
GCS_BUCKET = os.environ.get("SLM_YOYO_WEIGHTS_GCS_BUCKET", "")


def canonical_base_model() -> str:
    """Read the pinned base model from data/base-registry.yaml (single source of truth).

    The base MUST match the served GGUF (Tier A) so a trained adapter is servable.
    Falls back to the canonical default if the registry is unreadable.
    """
    default = "allenai/OLMo-3-7B-Instruct"
    candidates = [
        Path(__file__).resolve().parent.parent / "data" / "base-registry.yaml",
        Path(FOUNDRY_ROOT) / "data" / "base-registry.yaml",
    ]
    for registry in candidates:
        try:
            for line in registry.read_text().splitlines():
                s = line.strip()
                if s.startswith("canonical_base:"):
                    val = s.split(":", 1)[1].strip().strip("\"'")
                    if val:
                        return val
        except OSError:
            continue
    return default


# LoRA hyperparameters — same as run-dpo-training.py for A/B comparability
LORA_R = 16
LORA_ALPHA = 32
LORA_DROPOUT = 0.05
LORA_TARGET_MODULES = ["q_proj", "k_proj", "v_proj", "o_proj", "gate_proj", "up_proj", "down_proj"]
MAX_LENGTH = 2048   # was 512 — truncated the majority of diffs mid-hunk (median chosen ~1000+ tok)
BATCH_SIZE = 2
GRAD_ACCUM = 8
# SFT-LoRA wants a hotter LR than full fine-tune or DPO. 2e-5 is a full-FT default and
# under-fits an adapter; 1e-4..3e-4 is the LoRA-SFT band (verified research 2026-06-20).
LEARNING_RATE = 2e-4
NUM_EPOCHS = 1

# Minimum actual_diff length — very short diffs carry no useful signal.
MIN_DIFF_CHARS = 20


def _validate_corpus_integrity(records: list, fields: list[str], threshold: float = 0.05) -> None:
    """Sample up to 100 records; exit(1) if >threshold fraction have empty required fields."""
    sample = records[:100] if len(records) > 100 else records
    if not sample:
        return
    bad = sum(1 for r in sample if any(not (r.get(f) or "").strip() for f in fields))
    rate = bad / len(sample)
    print(f"[corpus] integrity: {bad}/{len(sample)} empty-field rows ({rate:.1%}) — checking {fields}")
    if rate > threshold:
        print(
            f"[ERROR] Corpus quality check failed: {rate:.1%} rows have empty fields {fields} "
            f"(threshold {threshold:.0%}). Fix corpus before training.",
            file=__import__("sys").stderr,
        )
        __import__("sys").exit(1)


def format_alpaca_prompt(instruction: str, output: str = "") -> str:
    """Alpaca chat template used for both training and inference."""
    prompt = f"### Instruction:\n{instruction}\n\n### Response:\n"
    if output:
        prompt += output
    return prompt


def build_instruction(brief: dict) -> str:
    """Build the task instruction from a shadow brief."""
    parts = [brief.get("body", "").strip()]
    scope = brief.get("scope", "")
    if scope and str(scope).strip():
        parts.append(f"\n\n## Scope\n{scope}")
    acceptance = brief.get("acceptance_test", "")
    if acceptance and str(acceptance).strip():
        parts.append(f"\n\n## Acceptance test\n{acceptance}")
    return "".join(parts)


def load_sft_pairs(queue_done_path: str) -> list[dict]:
    """Load SFT pairs from shadow queue-done directory.

    Reads all *.brief.jsonl files; extracts instruction=brief body+scope+test,
    output=actual_diff. Skips entries with empty or very short diffs.
    """
    pattern = os.path.join(queue_done_path, "*.brief.jsonl")
    files = sorted(glob.glob(pattern))
    print(f"[corpus] scanning {len(files)} brief files in {queue_done_path}")

    records = []
    skipped_no_diff = 0
    skipped_short = 0
    skipped_parse = 0

    for f in files:
        try:
            with open(f) as fh:
                first_line = fh.readline().strip()
            if not first_line:
                skipped_parse += 1
                continue
            entry = json.loads(first_line)
        except Exception as e:
            print(f"[WARN] skip {f}: {e}", file=sys.stderr)
            skipped_parse += 1
            continue

        actual_diff = (entry.get("actual_diff") or "").strip()
        if not actual_diff:
            skipped_no_diff += 1
            continue
        if len(actual_diff) < MIN_DIFF_CHARS:
            skipped_short += 1
            continue

        brief = entry.get("brief", {})
        instruction = build_instruction(brief).strip()
        if not instruction:
            skipped_parse += 1
            continue

        # Alpaca conversational format — OLMo-2 chat template applied by SFTTrainer
        records.append({
            "text": format_alpaca_prompt(instruction, actual_diff),
            # Store raw fields for diagnostics
            "_task_type": brief.get("task_type", "git-commit"),
            "_brief_id": brief.get("brief_id") or brief.get("id"),
            "_senior": brief.get("senior_identity"),
        })

    print(
        f"[corpus] loaded {len(records)} SFT pairs "
        f"(no-diff={skipped_no_diff} too-short={skipped_short} parse-error={skipped_parse})"
    )
    return records


def load_engineering_pairs(eng_root: str) -> list[dict]:
    """Load SFT pairs from the engineering edit corpus (commit_msg → diff).

    Wires the previously-orphaned engineering/** tree (no trainer read it) into SFT to
    break the single-task git-commit collapse. Filters bookkeeping-only (.agent/.claude)
    and oversized/truncated diffs so the added signal stays code-edit-focused.
    """
    pattern = os.path.join(eng_root, "**", "*.jsonl")
    files = sorted(glob.glob(pattern, recursive=True))
    print(f"[corpus] scanning {len(files)} engineering files in {eng_root}")
    records: list[dict] = []
    skipped = 0
    max_diff_chars = MAX_LENGTH * 4  # keep within the sequence budget (chars/4 heuristic)
    for f in files:
        try:
            with open(f) as fh:
                row = json.load(fh)
        except Exception:
            skipped += 1
            continue
        diff = (row.get("diff") or "").strip()
        msg = (row.get("commit_msg") or "").strip()
        if not diff or not msg or len(diff) < MIN_DIFF_CHARS or row.get("diff_truncated"):
            skipped += 1
            continue
        if len(diff) > max_diff_chars:
            skipped += 1
            continue
        # Drop bookkeeping-only edits (.agent//.claude churn) — low engineering signal.
        changed = [ln for ln in diff.splitlines() if ln.startswith("diff --git")]
        if changed and all(("/.agent/" in p or "/.claude/" in p) for p in changed):
            skipped += 1
            continue
        instruction = msg
        scope = row.get("scope")
        if scope and str(scope).strip():
            instruction += f"\n\n## Scope\n{scope}"
        records.append({
            "text": format_alpaca_prompt(instruction, diff),
            "_task_type": "engineering-edit",
            "_brief_id": row.get("source_commit"),
            "_senior": row.get("author"),
        })
    print(f"[corpus] loaded {len(records)} engineering pairs (skipped {skipped})")
    return records


def run_training(records: list[dict], base_model: str, output_dir: str,
                 dry_run: bool, max_runtime_seconds: int = 0, resume: bool = False) -> None:
    """Fine-tune base_model with SFT on records; save LoRA adapter to output_dir."""
    print(f"[train] base model:  {base_model}")
    print(f"[train] output dir:  {output_dir}")
    print(f"[train] pairs:       {len(records)}")
    print(f"[train] lr={LEARNING_RATE} r={LORA_R} alpha={LORA_ALPHA} epochs={NUM_EPOCHS}")
    if max_runtime_seconds:
        print(f"[train] runtime cap: {max_runtime_seconds}s")

    if dry_run:
        print("[train] DRY-RUN — skipping actual training")
        return

    try:
        import torch
        from datasets import Dataset
        from peft import LoraConfig, TaskType
        from transformers import AutoModelForCausalLM, AutoTokenizer, BitsAndBytesConfig, TrainerCallback
        from trl import SFTConfig, SFTTrainer
    except ImportError as e:
        print(f"[ERROR] Missing training library: {e}", file=sys.stderr)
        print("Install: pip install trl peft transformers datasets bitsandbytes", file=sys.stderr)
        sys.exit(1)

    os.makedirs(output_dir, exist_ok=True)

    print(f"[train] CUDA available: {torch.cuda.is_available()}")
    if torch.cuda.is_available():
        print(f"[train] GPU: {torch.cuda.get_device_name(0)}")

    bnb_config = BitsAndBytesConfig(
        load_in_4bit=True,
        bnb_4bit_quant_type="nf4",
        bnb_4bit_compute_dtype=torch.bfloat16,
        bnb_4bit_use_double_quant=True,
    )
    model = AutoModelForCausalLM.from_pretrained(
        base_model,
        quantization_config=bnb_config,
        device_map="auto",
        trust_remote_code=True,
    )
    model.config.use_cache = False
    tokenizer = AutoTokenizer.from_pretrained(base_model, trust_remote_code=True)
    if tokenizer.pad_token is None:
        tokenizer.pad_token = tokenizer.eos_token
        model.config.pad_token_id = tokenizer.eos_token_id

    # Fail-closed assertion: verify target_modules exist before peft attaches them.
    # HF Olmo2/Olmo3ForCausalLM use LLaMA-style names (q_proj/k_proj/...); the legacy
    # att_proj/ff_proj names match zero modules and silently train a no-op adapter.
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
        r=LORA_R,
        lora_alpha=LORA_ALPHA,
        lora_dropout=LORA_DROPOUT,
        target_modules=LORA_TARGET_MODULES,
        task_type=TaskType.CAUSAL_LM,
        bias="none",
    )

    dataset = Dataset.from_list([{"text": r["text"]} for r in records])
    split = dataset.train_test_split(test_size=0.1, seed=42)

    class RuntimeCapCallback(TrainerCallback):
        def __init__(self, max_secs: int, out_dir: str):
            self._max = max_secs
            self._start = time.monotonic()
            self._out = out_dir

        def on_step_end(self, args, state, control, **kwargs):
            if self._max and (time.monotonic() - self._start) >= self._max:
                print(f"[train] runtime cap reached — saving checkpoint and stopping")
                control.should_save = True
                control.should_training_stop = True

    callbacks = []
    if max_runtime_seconds:
        callbacks.append(RuntimeCapCallback(max_runtime_seconds, output_dir))

    os.environ.setdefault("PYTORCH_CUDA_ALLOC_CONF", "expandable_segments:True")

    training_args = SFTConfig(
        output_dir=output_dir,
        num_train_epochs=NUM_EPOCHS,
        per_device_train_batch_size=BATCH_SIZE,
        gradient_accumulation_steps=GRAD_ACCUM,
        gradient_checkpointing=True,
        gradient_checkpointing_kwargs={"use_reentrant": False},
        learning_rate=LEARNING_RATE,
        max_seq_length=MAX_LENGTH,
        dataset_text_field="text",
        logging_steps=5,
        save_steps=5,
        save_total_limit=2,
        eval_strategy="steps",
        eval_steps=5,
        report_to="none",
        bf16=torch.cuda.is_available(),
        remove_unused_columns=False,
        packing=False,
    )

    trainer = SFTTrainer(
        model=model,
        args=training_args,
        train_dataset=split["train"],
        eval_dataset=split["test"],
        processing_class=tokenizer,
        peft_config=peft_config,
        callbacks=callbacks or None,
    )

    # Staleness guard — same logic as run-dpo-training.py.
    # If the checkpoint is from a completed run (epoch >= 1.0), start fresh.
    resume_ckpt = None
    if resume:
        checkpoints = sorted(glob.glob(os.path.join(output_dir, "checkpoint-*")))
        if checkpoints:
            candidate = checkpoints[-1]
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
                print("[train] no valid resume checkpoint — starting fresh")
        else:
            print(f"[train] no checkpoint in {output_dir} — starting fresh")

    print(f"[train] starting SFT on {len(split['train'])} pairs ...")
    trainer.train(resume_from_checkpoint=resume_ckpt)

    print(f"[train] saving adapter to {output_dir}")
    trainer.save_model(output_dir)
    tokenizer.save_pretrained(output_dir)
    print("[train] done")


def main() -> None:
    parser = argparse.ArgumentParser(description="LoRA SFT training for apprenticeship adapter")
    parser.add_argument(
        "--queue-done",
        default=os.path.join(FOUNDRY_ROOT, "data", "apprenticeship", "queue-done"),
        help="Path to shadow queue-done directory containing *.brief.jsonl files",
    )
    parser.add_argument(
        "--engineering-corpus",
        default=os.path.join(FOUNDRY_ROOT, "data", "training-corpus", "engineering"),
        help="Engineering edit corpus (commit_msg→diff) wired into SFT for task diversity; '' disables",
    )
    parser.add_argument(
        "--base-model",
        default=canonical_base_model(),
        help="OLMo base model ID; default read from data/base-registry.yaml (OLMo-only policy)",
    )
    parser.add_argument(
        "--adapter-name",
        default="apprenticeship-pointsav-sft",
        help="Name for the output adapter",
    )
    parser.add_argument(
        "--output-dir",
        default=None,
        help="Override output directory (default: ./adapters/<name>-wip)",
    )
    parser.add_argument(
        "--sft-input",
        default=None,
        help="Path to a pre-built Alpaca JSONL file (output of export-sft.py). "
             "When set, --queue-done and --engineering-corpus are ignored. "
             "Enables test-mode.sh GPU runs: export corpus locally, rsync to VM, train.",
    )
    parser.add_argument(
        "--dry-run",
        action="store_true",
        help="Load corpus and report counts without training",
    )
    parser.add_argument(
        "--max-runtime-seconds",
        type=int,
        default=7200,
        help="Wall-clock training limit in seconds (0 = no cap)",
    )
    parser.add_argument(
        "--resume",
        action="store_true",
        help="Resume from latest checkpoint (staleness-guarded)",
    )
    args = parser.parse_args()

    if args.sft_input:
        # Pre-built Alpaca JSONL from export-sft.py — used by test-mode.sh GPU runs.
        if not os.path.isfile(args.sft_input):
            print(f"[ERROR] --sft-input file not found: {args.sft_input}", file=sys.stderr)
            sys.exit(1)
        with open(args.sft_input) as fh:
            records = [json.loads(line) for line in fh if line.strip()]
        print(f"[corpus] loaded {len(records)} records from --sft-input {args.sft_input}")
    else:
        records = load_sft_pairs(args.queue_done)
        if args.engineering_corpus and os.path.isdir(args.engineering_corpus):
            records += load_engineering_pairs(args.engineering_corpus)
    if not records:
        print("[ERROR] No valid SFT pairs found — check queue-done path", file=sys.stderr)
        print(f"[ERROR] Tried: {args.queue_done}", file=sys.stderr)
        sys.exit(1)

    _validate_corpus_integrity(records, fields=["text"])
    output_dir = args.output_dir or f"./adapters/{args.adapter_name}-wip"

    run_training(
        records,
        args.base_model,
        output_dir,
        dry_run=args.dry_run,
        max_runtime_seconds=args.max_runtime_seconds,
        resume=args.resume,
    )

    if not args.dry_run:
        print(f"\n[done] SFT adapter at: {output_dir}")


if __name__ == "__main__":
    main()
