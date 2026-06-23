#!/usr/bin/env python3
"""corpus-manifest.py — Zero-loss audit record for the SFT training corpus.

Walk both source trees (engineering/ and apprenticeship/) and record, for
every file: path, size, SHA256, mtime. Write the manifest as a timestamped
JSON file at:

    /home/mathew/Foundry/data/training-corpus/MANIFEST-<ISO8601>.json

Run this BEFORE any merge or export operation so the pre-merge state is
permanently recorded.

Usage:
    ./corpus-manifest.py [--corpus-root=<path>] [--out=<path>] [--dry-run]

Exit codes:
    0  — manifest written (or dry-run summary printed)
    2  — corpus root not found
"""

import argparse
import hashlib
import json
import os
import sys
from collections import defaultdict
from datetime import datetime, timezone
from pathlib import Path

_DEFAULT_CORPUS_ROOT = "/home/mathew/Foundry/data/training-corpus"
_SOURCE_TREES = ["engineering", "apprenticeship"]
# Subdirectories that are outputs, not inputs — exclude from manifest.
_EXCLUDE_TREES = {"merged", "doctrine", "extraction", "feedback", "sessions"}


def sha256_file(path: Path) -> str:
    """Compute SHA256 of a file, reading in 64 KB chunks."""
    h = hashlib.sha256()
    with open(path, "rb") as fh:
        for chunk in iter(lambda: fh.read(65536), b""):
            h.update(chunk)
    return h.hexdigest()


def mtime_iso(path: Path) -> str:
    """Return file mtime as ISO 8601 UTC string."""
    ts = path.stat().st_mtime
    return datetime.fromtimestamp(ts, tz=timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")


def walk_tree(root: Path) -> list[dict]:
    """Walk a source tree and return a list of file records."""
    records = []
    for p in sorted(root.rglob("*")):
        if not p.is_file():
            continue
        try:
            stat = p.stat()
            records.append({
                "path": str(p),
                "rel_path": str(p.relative_to(root.parent.parent)),
                "size": stat.st_size,
                "sha256": sha256_file(p),
                "mtime": mtime_iso(p),
            })
        except OSError as e:
            print(f"[WARN] cannot stat {p}: {e}", file=sys.stderr)
    return records


def main() -> None:
    ap = argparse.ArgumentParser(
        description="Build zero-loss audit manifest for SFT training corpus"
    )
    ap.add_argument(
        "--corpus-root",
        default=_DEFAULT_CORPUS_ROOT,
        help=f"Training corpus root (default: {_DEFAULT_CORPUS_ROOT})",
    )
    ap.add_argument(
        "--out",
        default=None,
        help="Output path (default: <corpus-root>/MANIFEST-<ISO8601>.json)",
    )
    ap.add_argument(
        "--dry-run",
        action="store_true",
        help="Print summary without writing the manifest file",
    )
    ap.add_argument(
        "--include-all",
        action="store_true",
        help="Include all subdirs (including merged/, extraction/, etc.); "
             "default is source trees only (engineering/ + apprenticeship/)",
    )
    args = ap.parse_args()

    corpus_root = Path(args.corpus_root)
    if not corpus_root.is_dir():
        print(f"ERROR: corpus root not found: {corpus_root}", file=sys.stderr)
        sys.exit(2)

    # Determine which trees to walk.
    if args.include_all:
        trees = sorted(
            p for p in corpus_root.iterdir()
            if p.is_dir() and p.name not in {"__pycache__"}
        )
    else:
        trees = []
        for name in _SOURCE_TREES:
            t = corpus_root / name
            if t.is_dir():
                trees.append(t)
            else:
                print(f"[WARN] expected source tree not found: {t}", file=sys.stderr)

    now_utc = datetime.now(timezone.utc)
    iso_stamp = now_utc.strftime("%Y-%m-%dT%H%M%SZ")

    out_path = args.out or str(corpus_root / f"MANIFEST-{iso_stamp}.json")

    print(f"corpus-manifest.py — {now_utc.strftime('%Y-%m-%d %H:%M:%S UTC')}")
    print(f"  corpus root:  {corpus_root}")
    print(f"  source trees: {[t.name for t in trees]}")
    print(f"  output:       {out_path}")
    print()

    all_records: list[dict] = []
    per_subdir: dict[str, dict] = {}  # subdir-name → {files, bytes}
    subdir_counts: dict[str, dict] = defaultdict(lambda: {"files": 0, "bytes": 0})

    for tree in trees:
        print(f"  scanning {tree.name}/ ...", end=" ", flush=True)
        records = walk_tree(tree)
        print(f"{len(records)} files")
        all_records.extend(records)

        # Per-subdir breakdown (one level below the tree root).
        for rec in records:
            rel = Path(rec["path"]).relative_to(corpus_root)
            # parts[0] = tree name (e.g. "engineering"), parts[1] = subdir
            parts = rel.parts
            subdir_key = f"{parts[0]}/{parts[1]}" if len(parts) > 1 else parts[0]
            subdir_counts[subdir_key]["files"] += 1
            subdir_counts[subdir_key]["bytes"] += rec["size"]

    total_files = len(all_records)
    total_bytes = sum(r["size"] for r in all_records)

    manifest = {
        "schema": "corpus-manifest-v1",
        "created": now_utc.strftime("%Y-%m-%dT%H:%M:%SZ"),
        "corpus_root": str(corpus_root),
        "source_trees": [t.name for t in trees],
        "summary": {
            "total_files": total_files,
            "total_bytes": total_bytes,
            "per_subdir": {
                k: {"files": v["files"], "bytes": v["bytes"]}
                for k, v in sorted(subdir_counts.items())
            },
        },
        "files": all_records,
    }

    print()
    print("Summary:")
    print(f"  total files:  {total_files:,}")
    print(f"  total bytes:  {total_bytes:,} ({total_bytes / 1_048_576:.1f} MiB)")
    print()
    print("  Per-subdir breakdown:")
    max_key_len = max((len(k) for k in subdir_counts), default=10)
    for k, v in sorted(subdir_counts.items()):
        print(f"    {k:<{max_key_len}}  {v['files']:>6} files  {v['bytes']:>12,} bytes")

    if args.dry_run:
        print()
        print("  (dry-run — manifest NOT written)")
        return

    os.makedirs(os.path.dirname(out_path) or ".", exist_ok=True)
    with open(out_path, "w", encoding="utf-8") as fh:
        json.dump(manifest, fh, indent=2, ensure_ascii=False)
        fh.write("\n")

    print()
    print(f"  manifest written: {out_path}")
    print(f"  ({total_files:,} records, {os.path.getsize(out_path) / 1_048_576:.1f} MiB)")


if __name__ == "__main__":
    main()
