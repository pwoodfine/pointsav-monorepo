#!/usr/bin/env python3
"""
run-grpo-training.py — GRPO self-improving training scaffold.

Uses TRL GRPOTrainer with a source-grounding reward signal:
  reward = grounded_entities / max(total_extracted, 1)

Where "grounded" means each extracted entity name appears verbatim
(case-insensitive) in the source document text.

This closes the self-improving loop without curated DPO pairs:
  OLMo generates completions → reward validates source fidelity →
  policy gradient update → better extraction on future documents.

Based on: MedGround-R1 (arxiv 2507.02994) adapted for SMB entity extraction.
GRPO paper: arxiv 2506.08008 (Group Relative Policy Optimization).

Requirements (trainer VM — same environment as run-dpo-training.py):
  pip install trl>=0.15 peft>=0.10 transformers>=4.46 datasets

Status: SCAFFOLD — not yet wired to any systemd unit.
        Operator activates for run 16+ after run 15 SimPO adapter is validated.

Kill switch: Do NOT run on workspace VM while /srv/foundry/data/yoyo-test-mode-kill exists.

Usage (trainer VM):
  python3 run-grpo-training.py --corpus /data/corpus/feedback/ --output-dir /data/weights/adapters/grpo-run-1
"""

import argparse
import json
import os
import sys
from pathlib import Path
from typing import Optional

FOUNDRY_ROOT = os.environ.get("FOUNDRY_ROOT", "/srv/foundry")


def canonical_base_model(default: str = "allenai/OLMo-3-7B-Instruct") -> str:
    """Read pinned base model from data/base-registry.yaml."""
    import yaml  # noqa: F401 — only on trainer VM
    registry = Path(FOUNDRY_ROOT) / "data" / "base-registry.yaml"
    if registry.exists():
        try:
            with open(registry) as f:
                data = yaml.safe_load(f)
            return data.get("canonical", default)
        except Exception:
            pass
    return default


# ── reward function ───────────────────────────────────────────────────────────

def extract_entity_names(completion: str) -> list[str]:
    """Parse entity names from a JSON completion produced by the extraction prompt.

    The completion is expected to be a JSON object matching the entity extraction
    schema: {"entities": [{"entity_name": "...", ...}, ...]} or a JSON array of
    entity objects directly (pre-fill mode). Returns [] on any parse failure.
    """
    text = completion.strip()
    # Try object form first
    try:
        obj = json.loads(text)
        if isinstance(obj, dict):
            entities = obj.get("entities", [])
        elif isinstance(obj, list):
            entities = obj
        else:
            return []
        return [
            e.get("entity_name", "")
            for e in entities
            if isinstance(e, dict) and e.get("entity_name")
        ]
    except json.JSONDecodeError:
        pass
    # Pre-fill re-attachment: model returned the continuation of "[{"
    try:
        obj = json.loads("[{\"" + text)
        return [
            e.get("entity_name", "")
            for e in obj
            if isinstance(e, dict) and e.get("entity_name")
        ]
    except json.JSONDecodeError:
        return []


def source_grounding_reward(
    completions: list[str],
    source_texts: list[str],
) -> list[float]:
    """Compute source-grounding reward for each (completion, source_text) pair.

    reward = grounded / max(total_extracted, 1)

    Where grounded = count of entity names that appear verbatim (case-insensitive)
    in the source document text.

    Guard: max(total, 1) prevents NaN when the model extracts zero entities.
    A zero-extraction completion receives reward 0.0, not NaN.
    """
    rewards = []
    for completion, source in zip(completions, source_texts):
        entities = extract_entity_names(completion)
        total = max(len(entities), 1)  # never divide by zero
        source_lower = source.lower()
        grounded = sum(
            1 for name in entities if name and name.lower() in source_lower
        )
        rewards.append(float(grounded) / float(total))
    return rewards


# ── data loading ──────────────────────────────────────────────────────────────

def load_grpo_dataset(corpus_dir: str):
    """Load CORPUS files from the feedback directory as GRPO prompts.

    Each example: {"prompt": "<source_text>", "source_text": "<source_text>"}
    The source_text is passed to the reward function for grounding verification.
    """
    import datasets

    records = []
    corpus_path = Path(corpus_dir)
    for jsonl_path in sorted(corpus_path.glob("CORPUS_*.json")):
        try:
            with open(jsonl_path) as f:
                obj = json.load(f)
            text = obj.get("corpus", "")
            if not text or len(text) < 50:
                continue
            # Extraction system prompt (same as Tier A worker in service-content)
            prompt = (
                "Extract all named entities from the following text. "
                "Return a JSON array of objects with keys: entity_name (string), "
                "classification (one of: Person, Company, Project, Account, Location). "
                "Return only grounded entities — names that appear verbatim in the text.\n\n"
                f"Text:\n{text[:2000]}"
            )
            records.append({"prompt": prompt, "source_text": text[:2000]})
        except Exception:
            continue

    if not records:
        print(f"[grpo] No CORPUS files found in {corpus_dir}", file=sys.stderr)
        sys.exit(1)

    print(f"[grpo] Loaded {len(records)} examples from {corpus_dir}")
    return datasets.Dataset.from_list(records)


# ── training ──────────────────────────────────────────────────────────────────

def run_grpo_training(
    corpus_dir: str,
    output_dir: str,
    base_model: Optional[str] = None,
    num_generations: int = 4,
    max_length: int = 512,
    num_train_epochs: int = 1,
    learning_rate: float = 1e-6,
) -> None:
    """Fine-tune OLMo with GRPO using source-grounding reward."""
    import torch
    from transformers import AutoModelForCausalLM, AutoTokenizer
    from peft import LoraConfig, get_peft_model
    from trl import GRPOConfig, GRPOTrainer

    model_name = base_model or canonical_base_model()
    print(f"[grpo] Base model: {model_name}")
    print(f"[grpo] Output: {output_dir}")

    tokenizer = AutoTokenizer.from_pretrained(
        model_name, trust_remote_code=True
    )
    if tokenizer.pad_token is None:
        tokenizer.pad_token = tokenizer.eos_token

    model = AutoModelForCausalLM.from_pretrained(
        model_name,
        torch_dtype=torch.float16,
        device_map="auto",
        trust_remote_code=True,
    )

    lora_config = LoraConfig(
        r=16,
        lora_alpha=32,
        target_modules=["q_proj", "k_proj", "v_proj", "o_proj",
                         "gate_proj", "up_proj", "down_proj"],
        lora_dropout=0.05,
        bias="none",
        task_type="CAUSAL_LM",
    )
    model = get_peft_model(model, lora_config)
    model.print_trainable_parameters()

    dataset = load_grpo_dataset(corpus_dir)

    grpo_config = GRPOConfig(
        output_dir=output_dir,
        num_generations=num_generations,
        max_completion_length=max_length,
        num_train_epochs=num_train_epochs,
        learning_rate=learning_rate,
        per_device_train_batch_size=1,
        gradient_accumulation_steps=4,
        logging_steps=10,
        save_steps=100,
        fp16=True,
    )

    def reward_fn(completions: list[str], source_text: list[str], **kwargs) -> list[float]:
        return source_grounding_reward(completions, source_text)

    trainer = GRPOTrainer(
        model=model,
        args=grpo_config,
        tokenizer=tokenizer,
        train_dataset=dataset,
        reward_funcs=reward_fn,
    )

    print("[grpo] Starting GRPO training run")
    trainer.train()
    trainer.save_model(output_dir)
    tokenizer.save_pretrained(output_dir)
    print(f"[grpo] Adapter saved to {output_dir}")


# ── CLI ───────────────────────────────────────────────────────────────────────

def main() -> None:
    parser = argparse.ArgumentParser(description="GRPO self-improving training scaffold")
    parser.add_argument("--corpus", required=True,
                        help="Directory of CORPUS_*.json files (source text for reward)")
    parser.add_argument("--output-dir", default="adapters/grpo-wip",
                        help="Output directory for the trained adapter")
    parser.add_argument("--base-model", default=None,
                        help="Base model path or HF name (default: from base-registry.yaml)")
    parser.add_argument("--num-generations", type=int, default=4,
                        help="Completions per prompt for group relative advantage (default: 4)")
    parser.add_argument("--max-length", type=int, default=512,
                        help="Max completion token length (default: 512)")
    parser.add_argument("--epochs", type=int, default=1,
                        help="Training epochs (default: 1)")
    parser.add_argument("--lr", type=float, default=1e-6,
                        help="Learning rate (default: 1e-6)")
    parser.add_argument("--dry-run", action="store_true",
                        help="Load dataset and print stats without training")
    args = parser.parse_args()

    # Kill switch check (same guard as nightly-run.sh)
    kill_switch = Path(FOUNDRY_ROOT) / "data" / "yoyo-test-mode-kill"
    if kill_switch.exists():
        print(
            f"[grpo] Kill switch active: {kill_switch}\n"
            "       Remove it to enable GPU training runs.",
            file=sys.stderr,
        )
        sys.exit(1)

    if args.dry_run:
        dataset = load_grpo_dataset(args.corpus)
        print(f"[grpo] Dry run: {len(dataset)} examples; would train {args.epochs} epoch(s)")
        # Verify reward function on one example
        if len(dataset) > 0:
            ex = dataset[0]
            rewards = source_grounding_reward(
                ["[{\"entity_name\": \"" + ex["source_text"][:20] + "\"}]"],
                [ex["source_text"]],
            )
            print(f"[grpo] Sample reward (first example): {rewards[0]:.3f}")
        return

    run_grpo_training(
        corpus_dir=args.corpus,
        output_dir=args.output_dir,
        base_model=args.base_model,
        num_generations=args.num_generations,
        max_length=args.max_length,
        num_train_epochs=args.epochs,
        learning_rate=args.lr,
    )


if __name__ == "__main__":
    main()
