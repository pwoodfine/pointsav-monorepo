#!/usr/bin/env python3
"""
repair-ledger.py — Remove stale SHA entries from the sweep ledger.

A ledger entry is stale when the SHA was recorded on 202-ACK from /v1/ingest
but Tier B was down, so the document was never enriched and no DPO pair was
written. Without removal, those SHAs are permanently skipped on future cycles.

Detection: a ledger SHA has a stale entry if no enrichment-*.jsonl file in
FEEDBACK_DIR contains a record with doc_id == "sweep-<sha>".

Run once after Tier B returns and Phase 4b has executed at least one cycle.
Safe to re-run: idempotent (removes stale entries; never adds new ones).
Does not modify the enrichment JSONLs.

Usage:
    python3 repair-ledger.py [--dry-run] [--verbose]

Environment overrides (same defaults as yoyo-daily-cycle.sh):
    SWEEP_LEDGER   path to the sweep ledger file
    FEEDBACK_DIR   directory containing enrichment-*.jsonl files
"""

import argparse
import glob
import json
import os
import sys

SWEEP_LEDGER = os.environ.get(
    "SWEEP_LEDGER",
    "/srv/foundry/data/yoyo-datagraph-sweep.ledger",
)
FEEDBACK_DIR = os.environ.get(
    "FEEDBACK_DIR",
    "/home/mathew/deployments/woodfine-fleet-deployment"
    "/cluster-totebox-jennifer/service-fs/data/training-corpus/feedback",
)


def load_ledger(path: str) -> list[str]:
    if not os.path.isfile(path):
        return []
    with open(path) as f:
        return [line.strip() for line in f if line.strip()]


def build_enriched_set(feedback_dir: str) -> set[str]:
    """Return the set of doc_ids that appear in any enrichment-*.jsonl."""
    enriched: set[str] = set()
    pattern = os.path.join(feedback_dir, "enrichment-*.jsonl")
    for path in glob.glob(pattern):
        try:
            with open(path) as f:
                for line in f:
                    line = line.strip()
                    if not line:
                        continue
                    try:
                        record = json.loads(line)
                        doc_id = record.get("doc_id") or record.get("prompt_doc_id")
                        if doc_id:
                            enriched.add(doc_id)
                    except json.JSONDecodeError:
                        pass
        except OSError:
            pass
    return enriched


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--dry-run", action="store_true", help="Report stale entries without modifying the ledger")
    parser.add_argument("--verbose", action="store_true", help="Print each stale SHA")
    args = parser.parse_args()

    ledger_entries = load_ledger(SWEEP_LEDGER)
    if not ledger_entries:
        print(f"Ledger is empty or absent: {SWEEP_LEDGER}")
        print("Nothing to repair.")
        return 0

    print(f"Ledger: {SWEEP_LEDGER} ({len(ledger_entries)} entries)")
    print(f"Enrichment dir: {FEEDBACK_DIR}")

    enriched_doc_ids = build_enriched_set(FEEDBACK_DIR)
    print(f"Enrichment JSONL doc_ids loaded: {len(enriched_doc_ids)}")

    stale: list[str] = []
    keep: list[str] = []
    for sha in ledger_entries:
        doc_id = f"sweep-{sha}"
        if doc_id in enriched_doc_ids:
            keep.append(sha)
        else:
            stale.append(sha)
            if args.verbose:
                print(f"  STALE: {sha[:12]}")

    print(f"\nResults: {len(keep)} enriched (keep), {len(stale)} stale (remove)")

    if not stale:
        print("No stale entries — ledger is clean.")
        return 0

    if args.dry_run:
        print(f"Dry-run: would remove {len(stale)} stale entries from ledger.")
        return 0

    # Write back only the keep entries
    with open(SWEEP_LEDGER, "w") as f:
        for sha in keep:
            f.write(sha + "\n")

    print(f"Ledger repaired: {len(stale)} stale entries removed, {len(keep)} retained.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
