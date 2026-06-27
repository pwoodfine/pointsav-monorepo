#!/usr/bin/env python3
"""export-sft.py — Build the supervised fine-tuning (SFT) corpus from the
apprenticeship shadow tuples' senior-authored gold diffs, and/or from the
engineering edit tuples' committed diffs.

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

Engineering tuples (tuple_type="edit") are handled the same way:
  - `commit_msg` is the subject / prompt context.
  - `diff` contains the full git diff; split per-file with the same logic.
  - Tuples where `diff_truncated=true` AND `diff` is >8000 chars are skipped
    (same _MAX_SEGMENT_CHARS cap — the diff was cut mid-file).

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
  ./export-sft.py [--source=apprenticeship|engineering|all]
                  [--dry-run] [--out=<path>] [--max-files-per-commit=N]

  --source=apprenticeship  (default) Original behavior — shadow tuples only.
  --source=engineering     Engineering edit tuples only.
  --source=all             Merge both; deduplicate by commit subject
                           (apprenticeship version wins on collision).

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
_CORPUS_BASE = os.path.join(FOUNDRY_ROOT, "data", "training-corpus")
_APPR_ROOT = os.path.join(_CORPUS_BASE, "apprenticeship")
_ENG_ROOT = os.path.join(_CORPUS_BASE, "engineering")
_MERGED_ROOT = os.path.join(_CORPUS_BASE, "merged")

# Strip the synthetic prefix the git post-commit hook prepends to the brief body.
_SUBJECT_PREFIX = "git-commit diff:"

# A `diff --git a/<path> b/<path>` header begins each per-file segment.
_DIFF_HEADER_RE = re.compile(r"^diff --git a/(.+?) b/(.+?)\s*$", re.MULTILINE)

# Diff-XYZ (arxiv:2510.12487) + CarperAI finding: LLMs struggle with precise
# line numbers in unified-diff hunk headers (`@@ -N,M +N,M @@`). Stripping
# them to `@@ ... @@` reduces hallucinated line numbers by 23-27pp on SWE-bench
# without changing the format or requiring a full Search/Replace migration.
# Applied to the SFT training corpus only — live captures are unchanged.
_HUNK_HEADER_RE = re.compile(r"@@ -\d+(?:,\d+)? \+\d+(?:,\d+)? @@")

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
    """Split a multi-file unified diff into (path, segment) per file.

    Hunk header line numbers are stripped (`@@ -N,M +N,M @@` → `@@ ... @@`)
    per Diff-XYZ / CarperAI findings — see _HUNK_HEADER_RE above.
    """
    out = []
    # Find each `diff --git` header position; slice between consecutive headers.
    headers = list(_DIFF_HEADER_RE.finditer(full_diff))
    for i, m in enumerate(headers):
        start = m.start()
        end = headers[i + 1].start() if i + 1 < len(headers) else len(full_diff)
        segment = full_diff[start:end].rstrip("\n")
        # Strip line numbers from hunk headers so the model learns context-matching,
        # not line-count prediction (Diff-XYZ arxiv:2510.12487).
        segment = _HUNK_HEADER_RE.sub("@@ ... @@", segment)
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


def _normalize_subject(s: str) -> str:
    """Return a whitespace-normalized, lowercased subject for dedup comparison."""
    return " ".join(s.strip().lower().split())


# ---------------------------------------------------------------------------
# Apprenticeship loader
# ---------------------------------------------------------------------------

def load_apprenticeship(
    corpus_root: str,
    task_type: str,
    max_files_per_commit: int,
    counters: dict,
    seen_segments: set,
    seen_subjects: dict,
) -> list[dict]:
    """Read apprenticeship shadow tuples and return SFT rows.

    Also populates seen_subjects with {normalized_subject: "apprenticeship"} so
    that the engineering loader can skip duplicate commits.
    """
    if task_type != "all":
        scan_root = Path(corpus_root) / task_type
        if not scan_root.is_dir():
            print(f"ERROR: task-type dir not found: {scan_root}", file=sys.stderr)
            sys.exit(2)
    else:
        scan_root = Path(corpus_root)

    rows = []

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
        counters["tuples_appr"] += 1

        if task_type != "all" and (d.get("task_type") or "") != task_type:
            counters["skipped_task_type"] += 1
            continue

        actual = (d.get("actual_diff") or "").strip()
        if not actual:
            counters["skipped_no_diff"] += 1
            continue

        brief = d.get("brief", {}) or {}
        subject = (brief.get("body") or "").strip()
        if subject.startswith(_SUBJECT_PREFIX):
            subject = subject[len(_SUBJECT_PREFIX):].strip()
        if not subject:
            subject = "(no commit subject captured)"

        # Register this subject as owned by apprenticeship for cross-corpus dedup.
        norm_subj = _normalize_subject(subject)
        seen_subjects[norm_subj] = "apprenticeship"

        segments = split_per_file(actual)
        if not segments:
            counters["skipped_no_diff"] += 1
            continue
        if len(segments) > max_files_per_commit:
            counters["skipped_big_commit"] += 1
            continue

        brief_id = brief.get("brief_id") or d.get("brief", {}).get("id") or "unknown"
        for path, segment in segments:
            if any(mk in segment for mk in _BINARY_MARKERS):
                counters["skipped_binary"] += 1
                continue
            if len(segment) < _MIN_SEGMENT_CHARS:
                counters["skipped_short"] += 1
                continue
            if len(segment) > _MAX_SEGMENT_CHARS:
                counters["skipped_huge"] += 1
                continue
            key = (path, hashlib.sha256(segment.encode("utf-8", "replace")).hexdigest())
            if key in seen_segments:
                continue
            seen_segments.add(key)
            rows.append({
                "prompt": build_prompt(subject, path),
                "completion": build_completion(subject, segment),
                "source_brief_id": brief_id,
                "path": path,
                "_source": "apprenticeship",
            })
            counters["emitted_appr"] += 1

    return rows


# ---------------------------------------------------------------------------
# Engineering loader
# ---------------------------------------------------------------------------

def load_engineering(
    corpus_root: str,
    max_files_per_commit: int,
    counters: dict,
    seen_segments: set,
    seen_subjects: dict,
) -> list[dict]:
    """Read engineering edit tuples and return SFT rows.

    Skips commits whose normalized subject already appears in seen_subjects
    (populated by the apprenticeship loader) — the apprenticeship version wins.
    """
    eng_path = Path(corpus_root)
    if not eng_path.is_dir():
        print(f"ERROR: engineering corpus dir not found: {eng_path}", file=sys.stderr)
        sys.exit(2)

    rows = []

    for tuple_file in eng_path.rglob("*.jsonl"):
        try:
            with open(tuple_file) as fh:
                raw = fh.read().strip()
            if not raw:
                continue
            d = json.loads(raw)
        except Exception as e:
            print(f"[WARN] skip {tuple_file}: {e}", file=sys.stderr)
            continue

        if d.get("tuple_type") != "edit":
            counters["skipped_not_edit"] += 1
            continue

        counters["tuples_eng"] += 1

        commit_msg = (d.get("commit_msg") or "").strip()
        if not commit_msg:
            commit_msg = "(no commit message)"

        # Cross-corpus dedup: skip if apprenticeship already has this subject.
        norm_subj = _normalize_subject(commit_msg)
        if seen_subjects.get(norm_subj) == "apprenticeship":
            counters["skipped_dedup_appr_wins"] += 1
            continue

        diff = (d.get("diff") or "").strip()
        if not diff:
            counters["skipped_no_diff"] += 1
            continue

        # Skip truncated diffs that are also very long — the diff was cut mid-file
        # and the tail is unusable. Matches the _MAX_SEGMENT_CHARS cap intent.
        diff_truncated = bool(d.get("diff_truncated"))
        if diff_truncated and len(diff) > _MAX_SEGMENT_CHARS:
            counters["skipped_truncated_diff"] += 1
            continue

        source_commit = (d.get("source_commit") or "")[:40]

        segments = split_per_file(diff)
        if not segments:
            counters["skipped_no_diff"] += 1
            continue
        if len(segments) > max_files_per_commit:
            counters["skipped_big_commit"] += 1
            continue

        for path, segment in segments:
            if any(mk in segment for mk in _BINARY_MARKERS):
                counters["skipped_binary"] += 1
                continue
            if len(segment) < _MIN_SEGMENT_CHARS:
                counters["skipped_short"] += 1
                continue
            if len(segment) > _MAX_SEGMENT_CHARS:
                counters["skipped_huge"] += 1
                continue
            key = (path, hashlib.sha256(segment.encode("utf-8", "replace")).hexdigest())
            if key in seen_segments:
                continue
            seen_segments.add(key)
            rows.append({
                "prompt": build_prompt(commit_msg, path),
                "completion": build_completion(commit_msg, segment),
                "source_brief_id": source_commit or "unknown",
                "path": path,
                "_source": "engineering",
            })
            counters["emitted_eng"] += 1

    return rows


# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------

def main() -> None:
    ap = argparse.ArgumentParser(
        description="Build SFT corpus from apprenticeship gold diffs and/or engineering edit tuples"
    )
    ap.add_argument("--dry-run", action="store_true", help="Report counts without writing")
    ap.add_argument(
        "--out",
        default=None,
        help="Output path (default: data/corpus/sft/<date>.jsonl for --source=apprenticeship; "
             "data/training-corpus/merged/<date>.jsonl for --source=all or --source=engineering)",
    )
    ap.add_argument(
        "--max-files-per-commit",
        type=int,
        default=20,
        help="Skip commits touching more than N files (default 20; guards against "
             "bulk-rename / vendored-dir commits that are not edit-skill signal).",
    )
    ap.add_argument(
        "--task-type",
        default="git-commit",
        help="Only export apprenticeship tuples of this task_type (default git-commit). Other shadow "
             "types (prose-edit, design-edit, ...) carry capture-boilerplate bodies "
             "rather than real task descriptions, so their prompts are low-signal. "
             "Pass 'all' to export every task type. Ignored when --source=engineering.",
    )
    ap.add_argument(
        "--source",
        default="apprenticeship",
        choices=["apprenticeship", "engineering", "all"],
        help="Which corpus to export: 'apprenticeship' (default, existing behavior), "
             "'engineering' (edit tuples only), or 'all' (merge both; apprenticeship wins on "
             "duplicate commit subjects).",
    )
    args = ap.parse_args()

    date_stamp = datetime.now(timezone.utc).strftime("%Y-%m-%d")

    # Determine default output path based on --source.
    if args.out:
        out_path = args.out
    elif args.source == "apprenticeship":
        out_path = os.path.join(FOUNDRY_ROOT, "data", "corpus", "sft", f"sft-{date_stamp}.jsonl")
    else:
        out_path = os.path.join(_MERGED_ROOT, f"sft-{args.source}-{date_stamp}.jsonl")

    # Validate source dirs exist before doing any work.
    if args.source in ("apprenticeship", "all"):
        if not os.path.isdir(_APPR_ROOT):
            print(f"ERROR: apprenticeship corpus dir not found: {_APPR_ROOT}", file=sys.stderr)
            sys.exit(2)
    if args.source in ("engineering", "all"):
        if not os.path.isdir(_ENG_ROOT):
            print(f"ERROR: engineering corpus dir not found: {_ENG_ROOT}", file=sys.stderr)
            sys.exit(2)

    counters: dict = {
        "tuples_appr": 0,
        "tuples_eng": 0,
        "emitted_appr": 0,
        "emitted_eng": 0,
        "skipped_task_type": 0,
        "skipped_no_diff": 0,
        "skipped_big_commit": 0,
        "skipped_binary": 0,
        "skipped_short": 0,
        "skipped_huge": 0,
        "skipped_truncated_diff": 0,
        "skipped_dedup_appr_wins": 0,
        "skipped_not_edit": 0,
    }

    # Shared dedup state across both loaders.
    seen_segments: set = set()
    # Normalized subject → corpus source; used for cross-corpus dedup.
    # Apprenticeship loader writes here first; engineering loader reads it.
    seen_subjects: dict = {}

    all_rows: list[dict] = []

    # --- Apprenticeship pass (always first, so it can claim subjects) ---
    if args.source in ("apprenticeship", "all"):
        appr_rows = load_apprenticeship(
            corpus_root=_APPR_ROOT,
            task_type=args.task_type,
            max_files_per_commit=args.max_files_per_commit,
            counters=counters,
            seen_segments=seen_segments,
            seen_subjects=seen_subjects,
        )
        all_rows.extend(appr_rows)

    # --- Engineering pass (second, so apprenticeship wins on collision) ---
    if args.source in ("engineering", "all"):
        eng_rows = load_engineering(
            corpus_root=_ENG_ROOT,
            max_files_per_commit=args.max_files_per_commit,
            counters=counters,
            seen_segments=seen_segments,
            seen_subjects=seen_subjects,
        )
        all_rows.extend(eng_rows)

    total_emitted = counters["emitted_appr"] + counters["emitted_eng"]

    # Strip the internal _source field before writing (it is for reporting only).
    output_rows = [{k: v for k, v in r.items() if k != "_source"} for r in all_rows]

    print("")
    print("export-sft.py summary:")
    print(f"  source:                    {args.source}")
    if args.source in ("apprenticeship", "all"):
        print(f"  task_type filter:          {args.task_type}")
        print(f"  apprenticeship tuples:     {counters['tuples_appr']}")
        print(f"  SFT records (appr):        {counters['emitted_appr']}")
        print(f"  skipped (task_type):       {counters['skipped_task_type']}")
    if args.source in ("engineering", "all"):
        print(f"  engineering tuples:        {counters['tuples_eng']}")
        print(f"  SFT records (eng):         {counters['emitted_eng']}")
        print(f"  skipped (not edit tuple):  {counters['skipped_not_edit']}")
        print(f"  skipped (trunc+huge diff): {counters['skipped_truncated_diff']}")
        if args.source == "all":
            print(f"  skipped (appr wins dedup): {counters['skipped_dedup_appr_wins']}")
    print(f"  total SFT records:         {total_emitted}")
    print(f"  skipped (no diff):         {counters['skipped_no_diff']}")
    print(f"  skipped (>{args.max_files_per_commit} files):       {counters['skipped_big_commit']}")
    print(f"  skipped (binary):          {counters['skipped_binary']}")
    print(f"  skipped (segment <{_MIN_SEGMENT_CHARS}c):    {counters['skipped_short']}")
    print(f"  skipped (segment >{_MAX_SEGMENT_CHARS}c):  {counters['skipped_huge']}")

    if args.dry_run:
        print("  (dry run; no output written)")
        if output_rows:
            print("\n  --- sample record ---")
            sample = output_rows[0]
            print(f"  path: {sample['path']}")
            print("  prompt:\n" + "\n".join("    " + ln for ln in sample["prompt"].splitlines()))
            print("  completion (head):\n" + "\n".join(
                "    " + ln for ln in sample["completion"].splitlines()[:10]))
        return

    os.makedirs(os.path.dirname(out_path) or ".", exist_ok=True)
    with open(out_path, "w", encoding="utf-8") as fh:
        for r in output_rows:
            fh.write(json.dumps(r, ensure_ascii=False) + "\n")
    print(f"  out:                       {out_path} ({total_emitted} lines)")

    # LIMA-style soft floor for SFT is lower than for DPO — a few hundred clean
    # format demonstrations already teach the envelope. Surface the count.
    if total_emitted >= 500:
        print(f"\n  {total_emitted} SFT records — sufficient to teach format + basic edit skill.")
    else:
        print(f"\n  {total_emitted} SFT records — usable; more commits will grow this.")


if __name__ == "__main__":
    main()
