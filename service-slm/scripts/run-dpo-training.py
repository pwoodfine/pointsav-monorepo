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
  - Default base model: read from data/base-registry.yaml (canonical = OLMo 3 7B Instruct,
    matching the served GGUF so the adapter is servable). Pre-load it on the persistent
    weights disk via vllm-weights-prep.sh to avoid re-download.
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


def canonical_base_model(default: str = "allenai/OLMo-3-7B-Instruct") -> str:
    """Read the pinned base model from data/base-registry.yaml (single source of truth).

    The base MUST match the served GGUF so a trained adapter is servable. Falls back to
    the canonical default if the registry is unreadable.
    """
    candidates = [
        os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(__file__))),
                     "data", "base-registry.yaml"),
        os.path.join(FOUNDRY_ROOT, "data", "base-registry.yaml"),
    ]
    for registry in candidates:
        try:
            with open(registry) as fh:
                for line in fh:
                    s = line.strip()
                    if s.startswith("canonical_base:"):
                        val = s.split(":", 1)[1].strip().strip("\"'")
                        if val:
                            return val
        except OSError:
            continue
    return default


# LoRA hyperparameters for OLMo 7B
# r=32/alpha=64: a sound default for 7B preference LoRA (r=16-32, alpha=2r). Rank is a minor
# lever vs all-linear targeting / LR / data quality (verified research 2026-06-20).
LORA_R = 32
LORA_ALPHA = 64
LORA_DROPOUT = 0.05
# HF Olmo2/Olmo3ForCausalLM use LLaMA-style leaf names — the legacy att_proj/ff_proj names
# match ZERO modules on an HF OLMo base (silent/aborting no-op). Kept in sync with run-sft.
LORA_TARGET_MODULES = ["q_proj", "k_proj", "v_proj", "o_proj", "gate_proj", "up_proj", "down_proj"]
MAX_PROMPT_LENGTH = 512
BATCH_SIZE = 2
GRAD_ACCUM = 8   # raised 4→8; effective batch 16; damps gradient noise at low per-device batch
LEARNING_RATE = 2e-6   # lowered from 1e-5; 12-25× too hot vs OLMo 2 reference recipe (Tülu 3 = 8e-7..2e-6)
NUM_EPOCHS = 1   # lowered from 3; 3 epochs on single-task corpus → over-reinforcement collapse risk (Opus audit §17)
BETA = 0.1  # DPO default. Prior 0.5 justification (empty-"[]" rejected) is obsolete — those pairs are now filtered.

# SFT phase hyperparameters (--mode sft). The 2026-06-19 Opus audit established
# that the apprenticeship corpus must be taught with SFT FIRST: OLMo 7B emits
# template placeholders on the majority of shadow briefs, so there is no valid
# `rejected` worth contrasting against and preference optimisation is premature.
# SFT on the senior-authored `chosen` diffs teaches the base format + edit skill;
# on-policy DPO/SimPO is layered on afterward once the model emits valid diffs.
# SFT needs a HOTTER LR than DPO (DPO LR 2e-6 is 5-10× too cold for SFT) and 2-3
# epochs on a ~1-2k example corpus.
SFT_LEARNING_RATE = 2e-4   # was 2e-5 (full-FT default); LoRA-SFT band is 1e-4..3e-4
SFT_NUM_EPOCHS = 2

# System message wrapped around every SFT example so the training prompt matches
# the exact system+user shape the model conditions on at inference. Mirrors
# APPRENTICE_SYSTEM_PROMPT in slm-doorman/src/apprenticeship.rs — keep in sync.
SFT_SYSTEM_PROMPT = (
    "You are a code-editing assistant. Output ONLY the structured response below "
    "— no prose before it.\n\n"
    "REQUIRED FORMAT (copy exactly, fill in values):\n\n"
    "---\nself_confidence: 0.7\nescalate: false\n---\n\n"
    "## Reasoning\nOne sentence: what changed and why.\n\n"
    "## Diff\n```diff\n--- a/path/to/file\n+++ b/path/to/file\n"
    "@@ -1,3 +1,3 @@\n context line\n-old line\n+new line\n```\n\n"
    "Rules:\n"
    "- The VERY FIRST characters of your response must be ---\n"
    "- Write reasoning in ONE sentence only — brevity leaves tokens for the diff.\n"
    "- Set escalate: false and write the unified diff when you can make the change.\n"
    "- The diff MUST be a valid unified diff (--- a/ +++ b/ @@ lines)."
)


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

# Verbatim tokens from APPRENTICE_SYSTEM_PROMPT's diff example. ≥2 co-occurring
# means OLMo copied the example rather than producing a real diff — a stub that
# still carries "--- a/"/"@@" markers and so slips past _REAL_DIFF_MARKERS.
# Must stay in sync with corpus_gate.rs SYSTEM_PROMPT_EXAMPLE_MARKERS.
_SYSTEM_PROMPT_EXAMPLE_MARKERS = ("path/to/file", "-old line", "+new line", " context line")


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
        if not is_echo:
            # System-prompt example echo: ≥2 verbatim marker tokens co-occur.
            example_markers = sum(1 for m in _SYSTEM_PROMPT_EXAMPLE_MARKERS if m in rejected)
            if example_markers >= 2:
                is_echo = True
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


def load_sft_files(corpus_path: str) -> list[dict]:
    """Load SFT records from corpus_path.

    Reads `sft-*.jsonl` files produced by export-sft.py. Each source line is
    `{"prompt": <user text>, "completion": <assistant envelope>}`. We wrap each
    into the conversational system+user+assistant shape the model sees at
    inference (SFT_SYSTEM_PROMPT system turn + user prompt + assistant
    completion), so the training distribution matches deployment.

    Unlike the DPO loader there is no `rejected` side and no length-ratio gate:
    every senior-authored completion is gold. We only drop structurally-empty
    rows and completions that are too short to carry the canonical envelope.
    """
    files = sorted(set(glob.glob(os.path.join(corpus_path, "sft-*.jsonl"))))
    print(f"[corpus] found {len(files)} SFT file(s) in {corpus_path}")
    records = []
    skipped_format = 0
    skipped_short = 0
    for f in files:
        with open(f) as fh:
            for line in fh:
                line = line.strip()
                if not line:
                    continue
                try:
                    d = json.loads(line)
                except Exception as e:
                    print(f"[WARN] skip line in {f}: {e}", file=sys.stderr)
                    skipped_format += 1
                    continue
                prompt = d.get("prompt", "")
                completion = d.get("completion", "")
                if not prompt or not completion:
                    skipped_format += 1
                    continue
                # The canonical envelope (frontmatter + reasoning + fenced diff)
                # is ~120 chars of scaffold even for a one-line diff; anything
                # below that is a malformed capture.
                if len(completion) < 120:
                    skipped_short += 1
                    continue
                records.append({
                    "messages": [
                        {"role": "system", "content": SFT_SYSTEM_PROMPT},
                        {"role": "user", "content": prompt},
                        {"role": "assistant", "content": completion},
                    ]
                })
    print(
        f"[corpus] loaded {len(records)} SFT records "
        f"(format-skip={skipped_format} too-short={skipped_short})"
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
                from trl import SimPOConfig, SimPOTrainer  # noqa: F401
            except ImportError:
                print(
                    "[ERROR] --loss-type simpo requested but SimPOConfig/SimPOTrainer are not\n"
                    "        available in the installed trl. SimPO (reference-free) is the\n"
                    "        load-bearing objective for affordable on-policy training; silently\n"
                    "        degrading to DPO would defeat that and re-introduce the ref-model\n"
                    "        memory cost. Aborting.\n"
                    "        Fix: install a trl with SimPO, OR wire CPOTrainer(loss_type='simpo',\n"
                    "        cpo_alpha=0.0) after verifying the installed trl API, OR pass\n"
                    "        --loss-type dpo explicitly to opt into standard DPO.",
                    file=sys.stderr,
                )
                sys.exit(1)
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

    # Startup assertion: verify target_modules exist before peft applies them.
    # HF Olmo2/Olmo3ForCausalLM use LLaMA-style names (q_proj/k_proj/...); the legacy
    # att_proj/ff_proj names match zero modules and would train a no-op adapter.
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
    # max_length must fit prompt + full completion. The previous flat 512 silently
    # truncated every diff longer than ~512 tokens mid-hunk (median chosen ≈ 1000+
    # tokens), so the model trained on syntactically-invalid half-diffs (Opus audit
    # 2026-06-19, training-architecture finding P2). SimPO is a single forward pass
    # (no reference model) so 1024 fits comfortably on an L4 24 GB; standard DPO is a
    # double forward and stays at 512 to preserve the documented OOM guard.
    if is_32b:
        _max_length = 1024
    elif loss_type == "dpo":
        _max_length = 512   # double-forward OOMs >512 on L4 24 GB
    else:  # simpo — single forward
        _max_length = 1024
    if is_32b:
        print(f"[train] 32B memory mode: batch=1, grad_ckpt=True, max_len={_max_length}, grad_accum={_grad_accum}")
    else:
        print(f"[train] 7B memory mode: batch={_batch_size}, grad_ckpt=True, max_len={_max_length}, loss={loss_type}")

    # Fail-closed truncation pre-check: if the majority of `chosen` completions exceed the
    # sequence cap, training learns on diffs cut mid-hunk (the documented truncation +
    # length-confound defect). Refuse rather than silently train on truncated targets.
    # Override with SLM_ALLOW_TRUNCATION=1 when intentionally training a length-capped pass.
    _chosen_est_tokens = sorted(len(r.get("chosen", "")) // 4 for r in records)
    if _chosen_est_tokens:
        _p50 = _chosen_est_tokens[len(_chosen_est_tokens) // 2]
        _over = sum(1 for t in _chosen_est_tokens if t > _max_length)
        _pct_over = _over / len(_chosen_est_tokens)
        print(f"[train] truncation check: max_length={_max_length}, chosen est-tokens "
              f"p50={_p50}, over-cap={_over}/{len(_chosen_est_tokens)} ({_pct_over:.0%})")
        if _p50 > _max_length or _pct_over > 0.5:
            _allow = os.environ.get("SLM_ALLOW_TRUNCATION", "").lower() in ("1", "true")
            print(
                f"[ERROR] {_pct_over:.0%} of chosen completions exceed max_length={_max_length} "
                f"(p50 est-tokens={_p50}). Training would learn on truncated diffs.\n"
                f"        Fix: raise max_length (SimPO single-forward fits more), curate the corpus\n"
                f"        to fit, or set SLM_ALLOW_TRUNCATION=1 to override. Aborting.",
                file=sys.stderr,
            )
            if not _allow:
                sys.exit(1)
            print("[WARN] SLM_ALLOW_TRUNCATION set — proceeding despite truncation", file=sys.stderr)

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


def run_sft_training(records: list[dict], base_model: str, output_dir: str, dry_run: bool,
                     max_runtime_seconds: int = 0, resume: bool = False) -> None:
    """Supervised fine-tune base_model on (system+user → assistant) records.

    This is the FIRST training phase per the 2026-06-19 Opus audit: teach the
    model the canonical output format and the basic edit skill from the
    senior-authored gold diffs before any preference optimisation. Uses TRL's
    SFTTrainer on the conversational `messages` format; the OLMo chat template
    masks the prompt and computes loss on the assistant turn only.
    """
    print(f"[sft] base model: {base_model}")
    print(f"[sft] output dir: {output_dir}")
    print(f"[sft] records:    {len(records)}")
    print(f"[sft] LoRA r={LORA_R} alpha={LORA_ALPHA} lr={SFT_LEARNING_RATE} epochs={SFT_NUM_EPOCHS}")
    if max_runtime_seconds:
        print(f"[sft] runtime cap: {max_runtime_seconds}s")

    if dry_run:
        print("[sft] DRY-RUN — skipping actual training")
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
    print(f"[sft] CUDA available: {torch.cuda.is_available()}")
    if torch.cuda.is_available():
        print(f"[sft] GPU: {torch.cuda.get_device_name(0)}")

    bnb_config = BitsAndBytesConfig(
        load_in_4bit=True,
        bnb_4bit_quant_type="nf4",
        bnb_4bit_compute_dtype=torch.bfloat16,
        bnb_4bit_use_double_quant=True,
    )

    print("[sft] loading tokenizer ...")
    tokenizer = AutoTokenizer.from_pretrained(base_model, trust_remote_code=True)
    if tokenizer.pad_token is None:
        tokenizer.pad_token = tokenizer.eos_token
    tokenizer.padding_side = "right"

    print("[sft] loading model (4-bit) ...")
    model = AutoModelForCausalLM.from_pretrained(
        base_model,
        quantization_config=bnb_config,
        device_map="auto",
        trust_remote_code=True,
    )
    model.config.use_cache = False

    # Same target-module assertion as the preference path: OLMo names, not LLaMA.
    _model_module_names = {name.split(".")[-1] for name, _ in model.named_modules()}
    _matched = [m for m in LORA_TARGET_MODULES if m in _model_module_names]
    if not _matched:
        print(
            f"[ERROR] LORA_TARGET_MODULES {LORA_TARGET_MODULES} matched 0 modules in model.\n"
            f"        Training would produce a no-op adapter. Aborting.",
            file=sys.stderr,
        )
        sys.exit(1)
    print(f"[sft] LoRA target assertion: {len(_matched)}/{len(LORA_TARGET_MODULES)} modules matched: {_matched}")

    peft_config = LoraConfig(
        task_type=TaskType.CAUSAL_LM,
        r=LORA_R,
        lora_alpha=LORA_ALPHA,
        lora_dropout=LORA_DROPOUT,
        target_modules=LORA_TARGET_MODULES,
        bias="none",
    )

    class RuntimeCapCallback(TrainerCallback):
        def __init__(self, max_seconds: int) -> None:
            self._start = time.monotonic()
            self._max = max_seconds

        def on_step_end(self, args, state, control, **kwargs):
            if self._max and (time.monotonic() - self._start) >= self._max:
                print(f"[sft] runtime cap reached — saving checkpoint and stopping")
                control.should_save = True
                control.should_training_stop = True

    dataset = Dataset.from_list(records)
    split = dataset.train_test_split(test_size=0.1, seed=42)

    os.environ.setdefault("PYTORCH_CUDA_ALLOC_CONF", "expandable_segments:True")

    callbacks = []
    if max_runtime_seconds:
        callbacks.append(RuntimeCapCallback(max_runtime_seconds))

    training_args = SFTConfig(
        output_dir=output_dir,
        num_train_epochs=SFT_NUM_EPOCHS,
        per_device_train_batch_size=BATCH_SIZE,
        gradient_accumulation_steps=GRAD_ACCUM,
        gradient_checkpointing=True,
        gradient_checkpointing_kwargs={"use_reentrant": False},
        learning_rate=SFT_LEARNING_RATE,
        max_length=2048,   # full single-file diff in canonical envelope fits; single forward
        logging_steps=5,
        save_steps=5,
        save_total_limit=2,
        eval_strategy="steps",
        eval_steps=5,
        report_to="none",
        bf16=torch.cuda.is_available(),
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

    resume_ckpt = None
    if resume:
        checkpoints = sorted(glob.glob(os.path.join(output_dir, "checkpoint-*")))
        if checkpoints:
            resume_ckpt = checkpoints[-1]
            print(f"[sft] resuming from checkpoint: {resume_ckpt}")
    print(f"[sft] starting SFT on {len(split['train'])} records ...")
    trainer.train(resume_from_checkpoint=resume_ckpt)
    print(f"[sft] saving adapter to {output_dir}")
    trainer.save_model(output_dir)
    tokenizer.save_pretrained(output_dir)
    print("[sft] done")


def main() -> None:
    parser = argparse.ArgumentParser(description="LoRA SFT/DPO training for apprenticeship adapter")
    parser.add_argument("--corpus", default=os.path.join(FOUNDRY_ROOT, "data", "training-corpus", "feedback"),
                        help="Path to feedback/ directory containing apprenticeship-*.jsonl files")
    parser.add_argument("--base-model", default=canonical_base_model(),
                        help="OLMo base model ID or local path (OLMo-only policy). Default read "
                             "from data/base-registry.yaml (canonical = OLMo 3 7B Instruct, the served "
                             "base). Pre-load on the yoyo-batch weights disk to avoid re-downloading.")
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
    parser.add_argument("--mode", default="pref", choices=["pref", "sft"],
                        help="Training phase. 'sft' (supervised fine-tune on gold diffs) MUST "
                             "run first per the 2026-06-19 audit — it teaches the canonical "
                             "format + edit skill before any preference optimisation. 'pref' "
                             "(default, DPO/SimPO) is only valid once SFT produces a model that "
                             "emits valid diffs >90% of the time, ideally with on-policy rejected "
                             "samples. SFT reads sft-*.jsonl; pref reads apprenticeship-*.jsonl.")
    parser.add_argument("--loss-type", default="simpo", choices=["simpo", "dpo"],
                        help="Preference learning objective (--mode pref only). 'simpo' (default) "
                             "avoids the reference-model length-normalisation bias that caused "
                             "token-count discrimination in the Jun-14 run. 'dpo' for ablation.")
    parser.add_argument("--simpo-gamma", type=float, default=0.5,
                        help="SimPO margin (gamma). Default 0.5. Increase to widen the "
                             "reward margin between chosen and rejected; decrease if training "
                             "is unstable on small corpora. Ignored when --loss-type=dpo.")
    args = parser.parse_args()

    corpus_path = args.corpus
    if args.from_gcs:
        local_staging = "/tmp/foundry-training-corpus"
        corpus_path = sync_from_gcs(args.adapter_name, local_staging)

    if args.mode == "sft":
        records = load_sft_files(corpus_path)
    else:
        records = load_feedback_files(corpus_path)
    if not records:
        kind = "SFT records" if args.mode == "sft" else "DPO pairs"
        print(f"[ERROR] No valid {kind} found — check corpus path and field names", file=sys.stderr)
        sys.exit(1)

    # Use a fixed -wip suffix so --resume finds the same checkpoint directory each day.
    # Only rename to a dated path when promoting the adapter to the registry.
    output_dir = args.output_dir or f"./adapters/{args.adapter_name}-wip"

    if args.mode == "sft":
        run_sft_training(records, args.base_model, output_dir, dry_run=args.dry_run,
                         max_runtime_seconds=args.max_runtime_seconds,
                         resume=args.resume)
    else:
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
