#!/usr/bin/env python3
"""redrive-quarantine.py — move poison-queue apprenticeship briefs back to active queue.

Targets queue-poison/ (retry-exhausted dead-letter) — not the legacy quarantine/ dir
(classification-guard rejections) which the Doorman does not currently populate.

Run AFTER Stage 6 promotes a hardened binary and the service has been restarted so the
fix that caused the failures is live before briefs are re-enqueued.

Usage:
    python3 redrive-quarantine.py [--dry-run] [--base-dir <path>] [--from-quarantine]

Flags:
    --dry-run          Print what would be moved; make no changes.
    --base-dir         Apprenticeship base dir (default: /srv/foundry/data/apprenticeship).
    --from-quarantine  Target legacy quarantine/ instead of queue-poison/ (historical use).
"""

import argparse
import os
import shutil
import sys


DEFAULT_BASE_DIR = "/srv/foundry/data/apprenticeship"


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--dry-run", action="store_true", help="Print actions without executing")
    parser.add_argument("--base-dir", default=DEFAULT_BASE_DIR, help="Apprenticeship base directory")
    parser.add_argument("--from-quarantine", action="store_true",
                        help="Target quarantine/ instead of queue-poison/ (legacy classification-guard use)")
    args = parser.parse_args()

    source_subdir = "quarantine" if args.from_quarantine else "queue-poison"
    source_dir = os.path.join(args.base_dir, source_subdir)
    queue_dir = os.path.join(args.base_dir, "queue")

    # Create source dir if absent — a missing queue-poison/ just means no dead-letters yet.
    if not os.path.isdir(source_dir):
        if args.from_quarantine:
            print(f"[ERROR] quarantine dir not found: {source_dir}", file=sys.stderr)
            sys.exit(1)
        os.makedirs(source_dir, exist_ok=True)
        print(f"[INFO] {source_dir} created (was absent — no dead-letters to re-drive).")
        sys.exit(0)

    if not os.path.isdir(queue_dir):
        print(f"[ERROR] queue dir not found: {queue_dir}", file=sys.stderr)
        sys.exit(1)

    entries = sorted(
        f for f in os.listdir(source_dir)
        if os.path.isfile(os.path.join(source_dir, f))
    )

    if not entries:
        print(f"{source_subdir}/ is empty — nothing to re-drive.")
        sys.exit(0)

    mode = "DRY-RUN" if args.dry_run else "LIVE"
    print(f"[{mode}] source: {source_dir}")
    print(f"[{mode}] queue:  {queue_dir}")
    print(f"[{mode}] entries: {len(entries)}")
    if len(entries) > 0:
        print(f"[WARN]  {len(entries)} dead-letter brief(s) found — verify the root cause is fixed before re-driving.")
    print()

    moved = 0
    skipped = 0

    for name in entries:
        src = os.path.join(source_dir, name)
        dst = os.path.join(queue_dir, name)

        if os.path.exists(dst):
            print(f"  SKIP (already in queue): {name}")
            skipped += 1
            continue

        if args.dry_run:
            print(f"  WOULD MOVE: {name}")
        else:
            shutil.move(src, dst)
            print(f"  MOVED: {name}")
        moved += 1

    print()
    print(f"[{mode}] moved={moved}  skipped={skipped}  total={len(entries)}")

    if args.dry_run:
        print("[DRY-RUN] No files were changed.  Re-run without --dry-run to apply.")
    else:
        print(f"Re-drive complete.  {moved} brief(s) returned to queue.")
        print("Monitor the apprenticeship drain; check /readyz queue_poison for recurrence.")


if __name__ == "__main__":
    main()
