#!/usr/bin/env python3
"""redrive-quarantine.py — move quarantined apprenticeship briefs back to queue.

Run AFTER Stage 6 promotes the hardened binary (commit 1a914564) and the
service has been restarted.  The classification-guard fix in
raw_entities_to_graph() means many previously-rejected briefs will now
process cleanly.

Usage:
    python3 redrive-quarantine.py [--dry-run] [--base-dir <path>]

Flags:
    --dry-run     Print what would be moved; make no changes.
    --base-dir    Apprenticeship base dir (default: /srv/foundry/data/apprenticeship).
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
    args = parser.parse_args()

    quarantine_dir = os.path.join(args.base_dir, "quarantine")
    queue_dir = os.path.join(args.base_dir, "queue")

    if not os.path.isdir(quarantine_dir):
        print(f"[ERROR] quarantine dir not found: {quarantine_dir}", file=sys.stderr)
        sys.exit(1)
    if not os.path.isdir(queue_dir):
        print(f"[ERROR] queue dir not found: {queue_dir}", file=sys.stderr)
        sys.exit(1)

    entries = sorted(
        f for f in os.listdir(quarantine_dir)
        if os.path.isfile(os.path.join(quarantine_dir, f))
    )

    if not entries:
        print("Quarantine is empty — nothing to re-drive.")
        sys.exit(0)

    mode = "DRY-RUN" if args.dry_run else "LIVE"
    print(f"[{mode}] quarantine: {quarantine_dir}")
    print(f"[{mode}] queue:      {queue_dir}")
    print(f"[{mode}] entries:    {len(entries)}")
    print()

    moved = 0
    skipped = 0

    for name in entries:
        src = os.path.join(quarantine_dir, name)
        dst = os.path.join(queue_dir, name)

        if os.path.exists(dst):
            # Collision: don't overwrite a queued entry with the same name.
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
        print(f"Re-drive complete.  {moved} briefs returned to queue.")
        print("Monitor the next nightly migration cycle for entity_f1 improvement.")


if __name__ == "__main__":
    main()
