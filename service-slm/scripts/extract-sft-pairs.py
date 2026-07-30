#!/usr/bin/env python3
"""Extract supervised fine-tuning pairs from the apprenticeship queue-done corpus.

Reads all *.jsonl files under QUEUE_DONE, filters to entries with substantive
actual_diff, and writes instruction→output pairs in HuggingFace SFT JSONL format.

Usage:
    python3 extract-sft-pairs.py [--out PATH] [--max-diff-chars N]

Output format (one JSON object per line):
    {"instruction": "...", "output": "..."}

Where:
    instruction  = brief context (cluster + commit SHA + task description)
    output       = actual unified diff from the commit

These pairs teach the model to produce properly formatted unified diffs in
the Foundry coding style. All 544 pairs are ground-truth P2 shadow captures —
diffs that were actually committed, not model-generated predictions.
"""

import argparse
import json
import os
import glob
import sys
from typing import Optional

QUEUE_DONE = "/srv/foundry/data/apprenticeship/queue-done"
DEFAULT_OUT = os.path.join(os.path.dirname(__file__), "sft-pairs", "sft-train.jsonl")
DEFAULT_MAX_DIFF = 32_000  # chars; skip very large diffs that exceed typical token budgets
MIN_DIFF = 20              # chars; skip trivially empty or whitespace-only diffs


def extract_instruction(brief: dict) -> str:
    body = brief.get("body", "").strip()
    scope = brief.get("scope", {})
    cluster = scope.get("cluster", "")
    scope_files = scope.get("files", [])

    parts = []
    if cluster:
        parts.append(f"Cluster: {cluster}")
    if scope_files:
        file_list = "\n".join(f"  {f}" for f in scope_files[:15])
        parts.append(f"Files changed:\n{file_list}")
    if body:
        parts.append(body)
    return "\n\n".join(parts).strip()


def is_valid(entry: dict, max_diff: int) -> bool:
    diff = entry.get("actual_diff", "")
    if not diff or len(diff.strip()) < MIN_DIFF:
        return False
    if len(diff) > max_diff:
        return False
    brief = entry.get("brief", {})
    # Require at least a body to provide context
    if not brief.get("body", "").strip():
        return False
    return True


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--out", default=DEFAULT_OUT, help="Output JSONL file path")
    parser.add_argument(
        "--max-diff-chars",
        type=int,
        default=DEFAULT_MAX_DIFF,
        help=f"Skip diffs larger than this many chars (default {DEFAULT_MAX_DIFF})",
    )
    parser.add_argument(
        "--queue-done",
        default=QUEUE_DONE,
        help="Path to queue-done directory",
    )
    args = parser.parse_args()

    os.makedirs(os.path.dirname(args.out), exist_ok=True)

    files = sorted(glob.glob(os.path.join(args.queue_done, "*.jsonl")))
    if not files:
        print(f"ERROR: no .jsonl files found in {args.queue_done}", file=sys.stderr)
        sys.exit(1)

    pairs = []
    skipped_empty = 0
    skipped_toolarge = 0
    skipped_error = 0

    for path in files:
        try:
            with open(path) as f:
                entry = json.load(f)
        except Exception as e:
            skipped_error += 1
            print(f"WARN: {os.path.basename(path)}: {e}", file=sys.stderr)
            continue

        diff = entry.get("actual_diff", "")
        if not diff or len(diff.strip()) < MIN_DIFF:
            skipped_empty += 1
            continue
        if len(diff) > args.max_diff_chars:
            skipped_toolarge += 1
            continue
        brief = entry.get("brief", {})
        if not brief.get("body", "").strip():
            skipped_empty += 1
            continue

        instruction = extract_instruction(brief)
        output = diff.strip()
        pairs.append({"instruction": instruction, "output": output})

    with open(args.out, "w", encoding="utf-8") as f:
        for pair in pairs:
            f.write(json.dumps(pair, ensure_ascii=False) + "\n")

    total = len(files)
    kept = len(pairs)
    print(f"Processed {total} entries:")
    print(f"  Kept:              {kept}")
    print(f"  Skipped (empty):   {skipped_empty}")
    print(f"  Skipped (too big): {skipped_toolarge}")
    print(f"  Skipped (error):   {skipped_error}")
    print(f"Output: {args.out}")

    if pairs:
        diff_lens = [len(p["output"]) for p in pairs]
        print(f"Diff size: min={min(diff_lens)}, median={sorted(diff_lens)[len(diff_lens)//2]}, max={max(diff_lens)}")


if __name__ == "__main__":
    main()
