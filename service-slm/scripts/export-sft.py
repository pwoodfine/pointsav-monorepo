#!/usr/bin/env python3
"""export-sft.py — Build the supervised fine-tuning (SFT) corpus from the
apprenticeship shadow tuples' senior-authored gold diffs.

Rationale (2026-06-19 Opus audit, training-architecture finding P0)
-------------------------------------------------------------------
The DPO corpus produced by export-dpo.sh frames an UNLEARNABLE task: prompt =
bare commit subject (no file context), chosen = the entire multi-file repo diff
(mean 93x longer than the rejected side), rejected = OLMo's broken fragment. No
preference recipe fixes this. The fix is to teach the format and the edit skill
with SFT first, on the senior diffs that are already gold.

This script salvages the existing shadow corpus into SFT data by:
  1. Reading each `shadow-*.jsonl` tuple's `.actual_diff` (the senior's real,
     committed, multi-file diff).
  2. Splitting it into one segment PER FILE on `diff --git` boundaries — this
     collapses the 93x length ratio toward ~1-3x and makes each target tractable.
  3. Wrapping each single-file diff in the canonical response envelope
     (YAML frontmatter + `## Reasoning` + fenced `## Diff`) that the model is
     pre-filled to emit at inference, so the SFT target IS on-policy format.

Output: one JSONL line per (commit, file):
  {"prompt": <user task text>, "completion": <assistant envelope>,
   "source_brief_id": <id>, "path": <file path>}

LIMITATION (follow-up, needs capture-path change): the prompt carries the commit
subject + file path but NOT the pre-edit file contents, because the shadow
tuples never captured them (`brief.scope.files` is empty and `brief_id` is not
the git SHA). True file-grounded prompts require the git post-commit hook to
record the SHA + pre-edit blobs at commit time. Tracked in
BRIEF-training-pipeline-10x.md. Even without file context, per-file SFT on the
canonical envelope is a large net improvement over the impossible whole-repo task.

Usage:
  ./export-sft.py [--dry-run] [--out=<path>] [--max-files-per-commit=N]

Exit codes:
  0 — corpus written (or dry-run summary)
  2 — corpus dir not found
"""

import argparse
import hashlib
import json
import os
import re
import sys
from datetime import datetime, timezone
from pathlib import Path

FOUNDRY_ROOT = os.environ.get("FOUNDRY_ROOT", "/srv/foundry")
CORPUS_ROOT = os.path.join(FOUNDRY_ROOT, "data", "training-corpus", "apprenticeship")

# Strip the synthetic prefix the git post-commit hook prepends to the brief body.
_SUBJECT_PREFIX = "git-commit diff:"

# A `diff --git a/<path> b/<path>` header begins each per-file segment.
_DIFF_HEADER_RE = re.compile(r"^diff --git a/(.+?) b/(.+?)\s*$", re.MULTILINE)

# Segments we never want as training targets.
_BINARY_MARKERS = ("GIT binary patch", "Binary files ")
# Minimum useful single-file diff length (chars). Below this it is a rename-only
# or mode-only stub with no content to learn from.
_MIN_SEGMENT_CHARS = 40
# Maximum single-file diff length (chars). Above this is almost always a generated
# or vendored file (Cargo.lock, minified bundle, snapshot) — not edit-skill signal,
# and it would truncate past the SFT max_length=2048 (~8000 chars) anyway. Dropping
# these keeps targets within the trainer's window. (Opus audit: drop generated-file noise.)
_MAX_SEGMENT_CHARS = 8000


def split_per_file(full_diff: str) -> list[tuple[str, str]]:
    """Split a multi-file unified diff into (path, segment) per file."""
    out = []
    # Find each `diff --git` header position; slice between consecutive headers.
    headers = list(_DIFF_HEADER_RE.finditer(full_diff))
    for i, m in enumerate(headers):
        start = m.start()
        end = headers[i + 1].start() if i + 1 < len(headers) else len(full_diff)
        segment = full_diff[start:end].rstrip("\n")
        # Prefer the b/ path (post-edit); fall back to a/.
        path = m.group(2) or m.group(1)
        out.append((path, segment))
    return out


def one_line_reasoning(subject: str) -> str:
    """Derive a single-sentence reasoning line from the commit subject."""
    s = subject.strip().replace("\n", " ")
    if len(s) > 160:
        s = s[:157] + "..."
    return f"Apply the committed change: {s}"


def build_completion(subject: str, segment: str) -> str:
    """Wrap a single-file diff in the canonical response envelope."""
    return (
        "---\n"
        "self_confidence: 0.95\n"
        "escalate: false\n"
        "---\n\n"
        "## Reasoning\n"
        f"{one_line_reasoning(subject)}\n\n"
        "## Diff\n"
        "```diff\n"
        f"{segment}\n"
        "```\n"
    )


def build_prompt(subject: str, path: str) -> str:
    """User task text — describes the change and names the file, no answer leak."""
    return (
        "## Task\n"
        "Produce a unified diff for the file below that accomplishes this change.\n\n"
        f"## Change\n{subject.strip()}\n\n"
        f"## File\n{path}\n"
    )


def main() -> None:
    ap = argparse.ArgumentParser(description="Build SFT corpus from apprenticeship gold diffs")
    ap.add_argument("--dry-run", action="store_true", help="Report counts without writing")
    ap.add_argument("--out", default=None, help="Output path (default: data/corpus/sft/<date>.jsonl)")
    ap.add_argument("--max-files-per-commit", type=int, default=20,
                    help="Skip commits touching more than N files (default 20; guards against "
                         "bulk-rename / vendored-dir commits that are not edit-skill signal).")
    ap.add_argument("--task-type", default="git-commit",
                    help="Only export tuples of this task_type (default git-commit). Other shadow "
                         "types (prose-edit, design-edit, ...) carry capture-boilerplate bodies "
                         "rather than real task descriptions, so their prompts are low-signal. "
                         "Pass 'all' to export every task type.")
    args = ap.parse_args()

    if not os.path.isdir(CORPUS_ROOT):
        print(f"ERROR: corpus dir not found: {CORPUS_ROOT}", file=sys.stderr)
        sys.exit(2)

    date_stamp = datetime.now(timezone.utc).strftime("%Y-%m-%d")
    out_path = args.out or os.path.join(FOUNDRY_ROOT, "data", "corpus", "sft", f"sft-{date_stamp}.jsonl")

    tuples = 0
    emitted = 0
    skipped_task_type = 0
    skipped_no_diff = 0
    skipped_big_commit = 0
    skipped_binary = 0
    skipped_short = 0
    skipped_huge = 0
    seen = set()  # (path, sha of segment) — dedup identical file diffs across commits
    rows = []

    # When restricting to git-commit, scan only that subdir — far faster than
    # an rglob over the whole apprenticeship tree (1,896 files).
    if args.task_type != "all":
        scan_root = Path(CORPUS_ROOT) / args.task_type
        if not scan_root.is_dir():
            print(f"ERROR: task-type dir not found: {scan_root}", file=sys.stderr)
            sys.exit(2)
    else:
        scan_root = Path(CORPUS_ROOT)

    for tuple_file in scan_root.rglob("shadow-*.jsonl"):
        try:
            with open(tuple_file) as fh:
                first = fh.readline().strip()
            if not first:
                continue
            d = json.loads(first)
        except Exception as e:
            print(f"[WARN] skip {tuple_file}: {e}", file=sys.stderr)
            continue
        tuples += 1

        if args.task_type != "all" and (d.get("task_type") or "") != args.task_type:
            skipped_task_type += 1
            continue

        actual = (d.get("actual_diff") or "").strip()
        if not actual:
            skipped_no_diff += 1
            continue

        brief = d.get("brief", {}) or {}
        subject = (brief.get("body") or "").strip()
        if subject.startswith(_SUBJECT_PREFIX):
            subject = subject[len(_SUBJECT_PREFIX):].strip()
        if not subject:
            subject = "(no commit subject captured)"

        segments = split_per_file(actual)
        if not segments:
            skipped_no_diff += 1
            continue
        if len(segments) > args.max_files_per_commit:
            skipped_big_commit += 1
            continue

        brief_id = brief.get("brief_id") or d.get("brief", {}).get("id") or "unknown"
        for path, segment in segments:
            if any(mk in segment for mk in _BINARY_MARKERS):
                skipped_binary += 1
                continue
            if len(segment) < _MIN_SEGMENT_CHARS:
                skipped_short += 1
                continue
            if len(segment) > _MAX_SEGMENT_CHARS:
                skipped_huge += 1
                continue
            key = (path, hashlib.sha256(segment.encode("utf-8", "replace")).hexdigest())
            if key in seen:
                continue
            seen.add(key)
            rows.append({
                "prompt": build_prompt(subject, path),
                "completion": build_completion(subject, segment),
                "source_brief_id": brief_id,
                "path": path,
            })
            emitted += 1

    print("")
    print("export-sft.py summary:")
    print(f"  task_type filter:          {args.task_type}")
    print(f"  shadow tuples read:        {tuples}")
    print(f"  SFT records emitted:       {emitted}")
    print(f"  skipped (task_type):       {skipped_task_type}")
    print(f"  skipped (no actual_diff):  {skipped_no_diff}")
    print(f"  skipped (>{args.max_files_per_commit} files):       {skipped_big_commit}")
    print(f"  skipped (binary):          {skipped_binary}")
    print(f"  skipped (segment <{_MIN_SEGMENT_CHARS}c):    {skipped_short}")
    print(f"  skipped (segment >{_MAX_SEGMENT_CHARS}c):  {skipped_huge}")

    if args.dry_run:
        print("  (dry run; no output written)")
        if rows:
            print("\n  --- sample record ---")
            sample = rows[0]
            print(f"  path: {sample['path']}")
            print("  prompt:\n" + "\n".join("    " + l for l in sample["prompt"].splitlines()))
            print("  completion (head):\n" + "\n".join(
                "    " + l for l in sample["completion"].splitlines()[:10]))
        return

    os.makedirs(os.path.dirname(out_path), exist_ok=True)
    with open(out_path, "w") as fh:
        for r in rows:
            fh.write(json.dumps(r, ensure_ascii=False) + "\n")
    print(f"  out:                       {out_path} ({emitted} lines)")

    # LIMA-style soft floor for SFT is lower than for DPO — a few hundred clean
    # format demonstrations already teach the envelope. Surface the count.
    if emitted >= 500:
        print(f"\n✓ {emitted} SFT records — sufficient to teach format + basic edit skill.")
    else:
        print(f"\n→ {emitted} SFT records — usable; more shadow commits will grow this.")


if __name__ == "__main__":
    main()
