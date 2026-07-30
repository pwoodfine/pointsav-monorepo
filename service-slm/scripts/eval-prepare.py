#!/usr/bin/env python3
"""
eval-prepare.py — Generate the held-out eval set for eval-adapter.sh.

Samples 10% of the SFT queue-done corpus (same source as run-sft-training.py)
and writes Alpaca-formatted pairs to the holdout JSONL file.  These pairs are
NOT used for training; they form a stable reference for pass@5 regression checks.

Format of each output line:
  {"prompt": "### Instruction:\n...\n\n### Response:\n", "expected": "<diff>"}

The `prompt` field ends BEFORE the response so eval-adapter.sh can feed it
to llama-server and compare the model's completion against `expected`.

Usage:
  python3 scripts/eval-prepare.py
  python3 scripts/eval-prepare.py --out /path/to/holdout.jsonl
  python3 scripts/eval-prepare.py --sample-frac 0.05   # 5% instead of 10%
  python3 scripts/eval-prepare.py --seed 99             # reproducible shuffle
"""

import argparse
import glob
import json
import os
import random
import sys

FOUNDRY_ROOT = os.environ.get("FOUNDRY_ROOT", "/srv/foundry")
DEFAULT_QUEUE_DONE = os.path.join(FOUNDRY_ROOT, "data", "apprenticeship", "queue-done")
DEFAULT_OUT = os.path.join(FOUNDRY_ROOT, "data", "training-corpus", "eval", "holdout-v1.jsonl")
MIN_DIFF_CHARS = 20


def build_instruction(brief: dict) -> str:
    parts = [brief.get("body", "").strip()]
    scope = brief.get("scope", "")
    if scope and str(scope).strip():
        parts.append(f"\n\n## Scope\n{scope}")
    acceptance = brief.get("acceptance_test", "")
    if acceptance and str(acceptance).strip():
        parts.append(f"\n\n## Acceptance test\n{acceptance}")
    return "".join(parts)


def load_pairs(queue_done: str) -> list[dict]:
    files = sorted(glob.glob(os.path.join(queue_done, "*.brief.jsonl")))
    pairs = []
    for f in files:
        try:
            with open(f) as fh:
                line = fh.readline().strip()
            if not line:
                continue
            entry = json.loads(line)
        except Exception:
            continue
        actual_diff = (entry.get("actual_diff") or "").strip()
        if not actual_diff or len(actual_diff) < MIN_DIFF_CHARS:
            continue
        brief = entry.get("brief", {})
        instruction = build_instruction(brief).strip()
        if not instruction:
            continue
        pairs.append({
            "prompt": f"### Instruction:\n{instruction}\n\n### Response:\n",
            "expected": actual_diff,
        })
    return pairs


def main() -> None:
    parser = argparse.ArgumentParser(description="Generate held-out eval set for eval-adapter.sh")
    parser.add_argument("--queue-done", default=DEFAULT_QUEUE_DONE)
    parser.add_argument("--out", default=DEFAULT_OUT)
    parser.add_argument("--sample-frac", type=float, default=0.10,
                        help="Fraction of corpus to hold out (default 0.10 = 10%%)")
    parser.add_argument("--seed", type=int, default=42)
    args = parser.parse_args()

    pairs = load_pairs(args.queue_done)
    if not pairs:
        print(f"ERROR: no valid pairs in {args.queue_done}", file=sys.stderr)
        sys.exit(1)

    rng = random.Random(args.seed)
    rng.shuffle(pairs)
    n = max(1, int(len(pairs) * args.sample_frac))
    holdout = pairs[:n]

    os.makedirs(os.path.dirname(args.out), exist_ok=True)
    with open(args.out, "w") as fh:
        for p in holdout:
            fh.write(json.dumps(p) + "\n")

    print(f"[eval-prepare] wrote {len(holdout)} pairs to {args.out}")
    print(f"[eval-prepare] source: {len(pairs)} total; sampled {args.sample_frac:.0%}")


if __name__ == "__main__":
    main()
