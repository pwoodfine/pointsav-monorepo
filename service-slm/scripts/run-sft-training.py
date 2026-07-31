#!/usr/bin/env python3
# SPDX-License-Identifier: AGPL-3.0-or-later
# SPDX-FileCopyrightText: 2026 Woodfine Capital Projects Inc.

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


def assert_checkpoint_rank_compatible(checkpoint_dir: str, expected_r: int, expected_alpha: int) -> None:
    """Fail loudly if `checkpoint_dir`'s adapter_config.json rank/alpha don't match the values
    this run is about to construct the model with (same logic as run-dpo-training.py — kept in
    sync since both scripts can resume checkpoints from the same production adapter directory).
    """
    config_path = os.path.join(checkpoint_dir, "adapter_config.json")
    try:
        with open(config_path) as fh:
            saved = json.load(fh)
    except (OSError, json.JSONDecodeError) as e:
        print(f"[ERROR] could not read {config_path} to verify rank compatibility: {e}", file=sys.stderr)
        sys.exit(1)
    saved_r = saved.get("r")
    saved_alpha = saved.get("lora_alpha")
    if saved_r != expected_r or saved_alpha != expected_alpha:
        print(
            f"[ERROR] checkpoint rank mismatch: {checkpoint_dir} was saved with "
            f"r={saved_r} alpha={saved_alpha}, but this run is configured for "
            f"r={expected_r} alpha={expected_alpha}. Resuming would crash with a PEFT "
            f"size-mismatch on every layer. Either point --output-dir at a fresh directory "
            f"or align this run's LoRA rank with the existing checkpoint.",
            file=sys.stderr,
        )
        sys.exit(1)


# LoRA hyperparameters — target_modules/dropout shared with run-dpo-training.py for
# A/B comparability; r differs (SFT uses smaller r for L4 24GB headroom).
# 2026-07-04 correction: R1's alpha/r=0.5 (commit f85e6711, 2026-07-01) crashed every
# resume of apprenticeship-pointsav-incremental from 2026-07-04 onward — checkpoint-49
# was saved at r=16/alpha=32 by an earlier run, incompatible with alpha=8. Realigned to
# alpha=32 (ratio 2.0): preserves that checkpoint's resume progress, and matches current
# Unsloth/Raschka guidance (alpha/r >= 1.0, never below 1) over the earlier Databricks-
# derived 0.5 ratio. Must stay in lockstep with run-dpo-training.py's SFT_LORA_R/
# SFT_LORA_ALPHA — both scripts write/resume checkpoints in the same directory.
LORA_R = 16
LORA_ALPHA = 32
LORA_DROPOUT = 0.05
LORA_TARGET_MODULES = ["q_proj", "k_proj", "v_proj", "o_proj", "gate_proj", "up_proj", "down_proj"]
MAX_LENGTH = 2048   # matches export-sft.py's own _MAX_SEGMENT_CHARS=8000 (~2000 tokens) assumption
BATCH_SIZE = 1      # OOM at bs=2 on L4 with OLMo-3-7B-Instruct 4-bit; effective=GRAD_ACCUM*1
GRAD_ACCUM = 8      # effective batch size = 8
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
        from transformers import AutoModelForCausalLM, AutoTokenizer, TrainerCallback
        from trl import SFTConfig, SFTTrainer
    except ImportError as e:
        print(f"[ERROR] Missing training library: {e}", file=sys.stderr)
        print("Install: pip install trl peft transformers datasets bitsandbytes", file=sys.stderr)
        sys.exit(1)

    os.makedirs(output_dir, exist_ok=True)

    print(f"[train] CUDA available: {torch.cuda.is_available()}")
    if torch.cuda.is_available():
        print(f"[train] GPU: {torch.cuda.get_device_name(0)}")

    # Load in float16 (no 4-bit). 4-bit quantization OOMs on L4 24GB because:
    # 1. Loading stages model in bfloat16 (~14 GB), then PEFT's
    #    prepare_model_for_kbit_training casts lm_head to float32 (+1.5 GB peak).
    # Float16 full load: ~14 GB. L4 has 22 GB. LoRA+optimizer: ~200 MB.
    # Activations at bs=1/seq=512 with gradient_checkpointing: ~1 GB.
    # device_map="auto" is for multi-GPU/offload; on a single L4 it makes accelerate
    # dispatch some params to a "meta" placeholder device, which then crashes the
    # backward pass under non-reentrant gradient checkpointing (RuntimeError:
    # MmBackward0 returned an invalid gradient — expected device meta but got cuda:0).
    # Load explicitly onto cuda:0 instead — there's only one GPU here.
    model = AutoModelForCausalLM.from_pretrained(
        base_model,
        torch_dtype=torch.float16,
        trust_remote_code=True,
    )
    model = model.to("cuda")
    model.config.use_cache = False
    model.enable_input_require_grads()  # required for LoRA without kbit training
    tokenizer = AutoTokenizer.from_pretrained(base_model, trust_remote_code=True)
    tokenizer.model_max_length = MAX_LENGTH  # TRL 1.x: set here instead of SFTConfig.max_seq_length
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

    # Fail-closed truncation pre-check: if the majority of formatted texts exceed the
    # sequence cap, training learns on silently-truncated targets. Mirrors the identical
    # check in run-dpo-training.py. Override with SLM_ALLOW_TRUNCATION=1 when intentionally
    # training a length-capped pass.
    _text_est_tokens = sorted(len(r["text"]) // 4 for r in records)
    if _text_est_tokens:
        _p50 = _text_est_tokens[len(_text_est_tokens) // 2]
        _over = sum(1 for t in _text_est_tokens if t > MAX_LENGTH)
        _pct_over = _over / len(_text_est_tokens)
        print(f"[train] truncation check: max_length={MAX_LENGTH}, text est-tokens "
              f"p50={_p50}, over-cap={_over}/{len(_text_est_tokens)} ({_pct_over:.0%})")
        if _p50 > MAX_LENGTH or _pct_over > 0.5:
            _allow = os.environ.get("SLM_ALLOW_TRUNCATION", "").lower() in ("1", "true")
            print(
                f"[ERROR] {_pct_over:.0%} of records exceed max_length={MAX_LENGTH} "
                f"(p50 est-tokens={_p50}). Training would learn on truncated targets.\n"
                f"        Fix: raise MAX_LENGTH, curate the corpus to fit, or set\n"
                f"        SLM_ALLOW_TRUNCATION=1 to override. Aborting.",
                file=sys.stderr,
            )
            if not _allow:
                sys.exit(1)
            print("[WARN] SLM_ALLOW_TRUNCATION set — proceeding despite truncation", file=sys.stderr)

    dataset = Dataset.from_list([{"text": r["text"]} for r in records])

    # Cap the eval split to ~64 rows instead of a flat 10% — a full held-out eval on every
    # eval_strategy="epoch" firing should stay fast. --sft-input records carry no _task_type
    # metadata (that only exists on the production load_sft_pairs/load_engineering_pairs
    # path), so this is a deterministic-seed uniform subsample here; true stratification by
    # _task_type applies only when that field is present.
    _eval_n = min(64, max(1, len(dataset) // 10))
    if records and "_task_type" in records[0]:
        # Round-robin pick from _task_type buckets up to _eval_n total.
        _buckets: dict[str, list[int]] = {}
        for i, r in enumerate(records):
            _buckets.setdefault(r.get("_task_type", ""), []).append(i)
        _eval_indices: list[int] = []
        _bucket_iters = {k: iter(v) for k, v in _buckets.items()}
        while len(_eval_indices) < _eval_n and _bucket_iters:
            for k in list(_bucket_iters.keys()):
                try:
                    _eval_indices.append(next(_bucket_iters[k]))
                except StopIteration:
                    del _bucket_iters[k]
                if len(_eval_indices) >= _eval_n:
                    break
        _eval_set = set(_eval_indices)
        split = {
            "train": dataset.select([i for i in range(len(dataset)) if i not in _eval_set]),
            "test": dataset.select(sorted(_eval_set)),
        }
    else:
        split = dataset.train_test_split(test_size=_eval_n, seed=42)

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
        # max_seq_length moved out of SFTConfig in TRL 1.x — set on tokenizer instead.
        dataset_text_field="text",
        logging_steps=5,
        save_steps=25,
        save_total_limit=2,
        # eval_strategy="epoch" (not "steps"/eval_steps=5): a full held-out eval every 5
        # optimizer steps dominated the training window under RuntimeCapCallback's forced
        # stop — Run 14-17 completed only ~21 of ~743 expected steps/epoch as a result.
        # With NUM_EPOCHS=1 this fires exactly once, at the natural end of training; a
        # capped smoke run that gets stopped early does zero evals, which is correct — the
        # smoke run's job is "did training run," deploy-gate.sh verifies quality.
        eval_strategy="epoch",
        report_to="none",
        bf16=torch.cuda.is_available(),
        remove_unused_columns=False,
        packing=True,
    )

    trainer = SFTTrainer(
        model=model,
        args=training_args,
        train_dataset=split["train"],
        eval_dataset=split["test"],
        processing_class=tokenizer,  # TRL >=0.12 (aligned with transformers 4.47+)
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
                assert_checkpoint_rank_compatible(candidate, LORA_R, LORA_ALPHA)
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
        # Pre-built corpus from export-sft.py — records have {prompt, completion} fields.
        # Convert to Alpaca {text} format so the rest of the pipeline is uniform.
        if not os.path.isfile(args.sft_input):
            print(f"[ERROR] --sft-input file not found: {args.sft_input}", file=sys.stderr)
            sys.exit(1)
        with open(args.sft_input) as fh:
            raw = [json.loads(line) for line in fh if line.strip()]
        records = [
            {"text": format_alpaca_prompt(r.get("prompt", ""), r.get("completion", ""))}
            for r in raw
            if r.get("prompt") or r.get("completion")
        ]
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
