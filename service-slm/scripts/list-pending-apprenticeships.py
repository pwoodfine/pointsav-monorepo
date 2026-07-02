#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0 OR MIT
"""
list-pending-apprenticeships.py — list apprenticeship shadow attempts that have
not yet received an operator verdict, so a human can pick items to review.

Background: cast_apprenticeship_verdict (MCP tool) exists and works, but has
zero call sites anywhere in the codebase — it is a pure operator-action tool
with no workflow that surfaces which attempts are waiting for review. Verdicts
are consequently near-zero across the whole corpus. This script is the
smallest correct fix: make the pending queue visible. It does NOT cast
verdicts itself (no LLM-judge auto-verdict path — SYS-ADR-07 and
model-collapse evidence both rule that out; verdicts must stay human-cast).

Read-only: never writes to queue-done/, ledger.md, or anywhere else.

Usage:
  list-pending-apprenticeships.py [--limit N] [--task-type TYPE]

Output: a scannable table of the N most recent shadow attempts in
queue-done/ whose brief_id does not appear in ledger.md's verdict log,
plus a total-pending count and a breakdown by task_type.

To act on an item: read its .brief.jsonl directly for the attempt_id, then
call the cast_apprenticeship_verdict MCP tool with brief_id + attempt_id +
verdict (accept|refine|reject|defer-tier-c).
"""

import argparse
import json
import re
import sys
from pathlib import Path

FOUNDRY_ROOT = Path("/srv/foundry")
QUEUE_DONE = FOUNDRY_ROOT / "data" / "apprenticeship" / "queue-done"
LEDGER = FOUNDRY_ROOT / "data" / "apprenticeship" / "ledger.md"

BRIEF_ID_RE = re.compile(r"brief_id=([A-F0-9]+)")


def load_verdicted_brief_ids() -> set[str]:
    if not LEDGER.exists():
        return set()
    text = LEDGER.read_text(errors="ignore")
    return set(BRIEF_ID_RE.findall(text))


def load_brief(path: Path) -> dict:
    try:
        with open(path) as f:
            row = json.loads(f.readline())
        return row.get("brief", {})
    except (OSError, json.JSONDecodeError):
        return {}


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--limit", type=int, default=20, help="max rows to print (default 20)")
    ap.add_argument("--task-type", default=None, help="filter to one task_type")
    args = ap.parse_args()

    if not QUEUE_DONE.is_dir():
        print(f"ERROR: queue-done directory not found: {QUEUE_DONE}", file=sys.stderr)
        sys.exit(1)

    verdicted = load_verdicted_brief_ids()
    files = sorted(QUEUE_DONE.glob("*.brief.jsonl"), key=lambda p: p.stat().st_mtime, reverse=True)

    print(f"queue-done: {len(files)} total shadow attempts")
    print(f"ledger:     {len(verdicted)} brief_ids already have a verdict")
    print()

    task_type_counts: dict[str, int] = {}
    pending_rows = []
    for f in files:
        brief_id = f.name.removesuffix(".brief.jsonl")
        if brief_id in verdicted:
            continue
        brief = load_brief(f)
        task_type = brief.get("task_type", "?")
        task_type_counts[task_type] = task_type_counts.get(task_type, 0) + 1
        pending_rows.append((f, brief_id, brief))

    shown = 0
    for f, brief_id, brief in pending_rows:
        task_type = brief.get("task_type", "?")
        if args.task_type and task_type != args.task_type:
            continue
        if shown >= args.limit:
            continue
        created = brief.get("created", "?")
        senior = brief.get("senior_identity", "?")
        body = (brief.get("body", "") or "").replace("\n", " ")[:70]
        print(f"{brief_id:<34} {task_type:<16} {created:<24} {senior:<11} {body}")
        shown += 1

    print()
    filt = f", task_type={args.task_type}" if args.task_type else ""
    print(f"Shown: {shown} (limit {args.limit}{filt})")
    print(f"Total pending (no verdict yet): {len(pending_rows)}")
    print()
    print("By task_type:")
    for t, c in sorted(task_type_counts.items(), key=lambda kv: -kv[1]):
        print(f"  {t:<20} {c}")
    print()
    print("To cast a verdict: read the .brief.jsonl for its attempt_id, then call the")
    print(f"cast_apprenticeship_verdict MCP tool. File: {QUEUE_DONE}/<brief_id>.brief.jsonl")


if __name__ == "__main__":
    main()
