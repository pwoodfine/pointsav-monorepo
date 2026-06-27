#!/usr/bin/env python3
"""refilter-dpo.py — Retroactively apply corpus_gate rules to historical DPO pairs.

The 1,952 pairs in data/training-corpus/feedback/ predate the corpus_gate
length-ratio rule (added later in corpus_gate.rs). This script reads every
pair, applies the same gate logic as Rust, and writes surviving pairs to
feedback-filtered-<YYYYMMDD>.jsonl. Prints a rejection-reason histogram.

Corpus gate rules applied (mirrors corpus_gate.rs):
  1. Both chosen and rejected must be >= 80 chars (MIN_REJECTED_CHARS).
  2. length_ratio = max(len(chosen), len(rejected)) / min(...) must be <= 8.0.
  3. Neither side starts with a TEMPLATE_ECHO_PREFIX.
  4. (Optional, --git-check) chosen must be parseable by `git apply --check`.

Usage:
  python refilter-dpo.py [--out=<path>] [--dry-run] [--git-check]

Exit codes:
  0 — output written (or dry-run summary)
  1 — no pairs survived; DPO phase should be deferred
  2 — corpus dir not found
"""

import argparse
import json
import os
import subprocess
import sys
import tempfile
from datetime import datetime, timezone
from pathlib import Path

FOUNDRY_ROOT = os.environ.get("FOUNDRY_ROOT", "/srv/foundry")
_CORPUS_BASE = os.path.join(FOUNDRY_ROOT, "data", "training-corpus")
_FEEDBACK_DIR = os.path.join(_CORPUS_BASE, "feedback")

_MAX_LENGTH_RATIO = 8.0
_MIN_CHARS = 80

_TEMPLATE_ECHO_PREFIXES = (
    "<no diff provided",
    "<no changes",
    "<insert diff",
    "auto-reject: olmo-attempt-below-senior-standard",
    "auto-reject:",
    "<unified diff:",
)

CLEAN_PAIR_FLOOR = 100  # below this, DPO phase should be deferred


def _has_echo_prefix(text: str) -> bool:
    low = text.lstrip()
    return any(low.startswith(p) for p in _TEMPLATE_ECHO_PREFIXES)


def _git_apply_check(diff_text: str) -> bool:
    """Returns True if diff_text is parseable by git apply --check."""
    try:
        with tempfile.NamedTemporaryFile(mode="w", suffix=".patch", delete=False) as f:
            f.write(diff_text)
            patch_path = f.name
        result = subprocess.run(
            ["git", "apply", "--check", "--stat", patch_path],
            capture_output=True,
            timeout=5,
        )
        return result.returncode == 0
    except Exception:
        return False
    finally:
        try:
            os.unlink(patch_path)
        except Exception:
            pass


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    parser.add_argument("--out", default=None, help="Output path (default: feedback-filtered-YYYYMMDD.jsonl in corpus dir)")
    parser.add_argument("--dry-run", action="store_true", help="Print stats only; do not write output")
    parser.add_argument("--git-check", action="store_true", help="Also run git apply --check on chosen side")
    args = parser.parse_args()

    if not os.path.isdir(_FEEDBACK_DIR):
        print(f"[ERROR] Feedback dir not found: {_FEEDBACK_DIR}", file=sys.stderr)
        return 2

    out_path = args.out or os.path.join(
        _CORPUS_BASE,
        f"feedback-filtered-{datetime.now(timezone.utc).strftime('%Y%m%d')}.jsonl",
    )

    reasons: dict[str, int] = {}
    surviving: list[dict] = []
    total = 0

    for jsonl_file in sorted(Path(_FEEDBACK_DIR).rglob("*.jsonl")):
        with open(jsonl_file) as fh:
            for line in fh:
                line = line.strip()
                if not line:
                    continue
                try:
                    rec = json.loads(line)
                except json.JSONDecodeError:
                    reasons["json_decode_error"] = reasons.get("json_decode_error", 0) + 1
                    total += 1
                    continue

                total += 1
                chosen = rec.get("chosen", "") or ""
                rejected = rec.get("rejected", "") or ""

                if len(chosen) < _MIN_CHARS:
                    reasons["chosen_too_short"] = reasons.get("chosen_too_short", 0) + 1
                    continue
                if len(rejected) < _MIN_CHARS:
                    reasons["rejected_too_short"] = reasons.get("rejected_too_short", 0) + 1
                    continue

                lo = min(len(chosen), len(rejected))
                hi = max(len(chosen), len(rejected))
                ratio = hi / lo if lo > 0 else float("inf")
                if ratio > _MAX_LENGTH_RATIO:
                    reasons["length_ratio_exceeded"] = reasons.get("length_ratio_exceeded", 0) + 1
                    continue

                if _has_echo_prefix(chosen):
                    reasons["chosen_template_echo"] = reasons.get("chosen_template_echo", 0) + 1
                    continue
                if _has_echo_prefix(rejected):
                    reasons["rejected_template_echo"] = reasons.get("rejected_template_echo", 0) + 1
                    continue

                if args.git_check and not _git_apply_check(chosen):
                    reasons["git_apply_failed"] = reasons.get("git_apply_failed", 0) + 1
                    continue

                surviving.append(rec)

    print(f"[refilter-dpo] Input:     {total} pairs")
    print(f"[refilter-dpo] Surviving: {len(surviving)} pairs ({len(surviving)/total*100:.1f}%)")
    print(f"[refilter-dpo] Rejected:  {total - len(surviving)} pairs")
    print("[refilter-dpo] Rejection reasons:")
    for reason, count in sorted(reasons.items(), key=lambda x: -x[1]):
        print(f"  {reason:40s}: {count}")

    if len(surviving) < CLEAN_PAIR_FLOOR:
        print(
            f"\n[WARN] Only {len(surviving)} pairs survived — below CLEAN_PAIR_FLOOR={CLEAN_PAIR_FLOOR}.\n"
            "       DPO phase should be deferred until more clean pairs are captured.",
            file=sys.stderr,
        )

    if args.dry_run:
        print("[refilter-dpo] --dry-run: no output written.")
        return 0 if len(surviving) >= CLEAN_PAIR_FLOOR else 1

    with open(out_path, "w") as fh:
        for rec in surviving:
            fh.write(json.dumps(rec, ensure_ascii=False) + "\n")
    print(f"[refilter-dpo] Written:   {out_path}")
    return 0 if len(surviving) >= CLEAN_PAIR_FLOOR else 1


if __name__ == "__main__":
    sys.exit(main())
