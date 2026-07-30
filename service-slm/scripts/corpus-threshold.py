#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0 OR MIT
"""
Phase 3 corpus threshold watcher and training trigger.

Counts JSONL tuples per adapter bucket against per-adapter thresholds.
When a threshold is reached (or --force is supplied), writes a
training-pending marker and syncs corpus to GCS for Yo-Yo #1 pickup
(if SLM_YOYO_WEIGHTS_GCS_BUCKET is set; otherwise marker is local only).

Run modes:
  On-demand:     corpus-threshold.py [--dry-run] [--adapter NAME]
  Sunday cron:   corpus-threshold.py --force          (via systemd timer)

Adapter buckets and thresholds:
  engineering-pointsav    engineering/**/*.jsonl   threshold=50  SFT
  apprenticeship-pointsav apprenticeship/**/*.jsonl threshold=50  DPO
"""

import argparse
import json
import os
import subprocess
import sys
from datetime import datetime, timezone
from pathlib import Path

FOUNDRY_ROOT = Path(os.environ.get("FOUNDRY_ROOT", Path.home() / "Foundry"))
CORPUS_ROOT = FOUNDRY_ROOT / "data" / "training-corpus"
PENDING_DIR = FOUNDRY_ROOT / "data" / "training-pending"

ADAPTER_SPECS: dict = {
    "engineering-pointsav": {
        "glob": "engineering/**/*.jsonl",
        "threshold": 50,
        "method": "sft",
        "description": "Cross-cluster engineering edit tuples for the pointsav vendor adapter",
    },
    "apprenticeship-pointsav": {
        "glob": "apprenticeship/**/*.jsonl",
        "threshold": 50,
        "method": "dpo",
        "description": "Apprenticeship shadow + verdict tuples; DPO requires signed verdicts",
    },
}


def count_files(glob_pattern: str) -> list:
    return list(CORPUS_ROOT.glob(glob_pattern))


def count_valid_dpo_files(glob_pattern: str) -> list:
    """Count apprenticeship JSONL files that carry a non-empty attempt.diff.

    Files where attempt.diff is absent, null, or "" are degenerate tuples
    produced when Tier B was unavailable and OLMo escalated without a diff.
    They are not valid DPO signal and must not count toward the threshold.
    """
    valid = []
    for path in CORPUS_ROOT.glob(glob_pattern):
        try:
            with open(path, "r") as f:
                first_line = f.readline().strip()
            if not first_line:
                continue
            row = json.loads(first_line)
            attempt_diff = (row.get("attempt") or {}).get("diff") or ""
            if attempt_diff:
                valid.append(path)
        except (OSError, json.JSONDecodeError):
            continue
    return valid


def write_pending_marker(adapter_name: str, files: list, dry_run: bool = False) -> Path:
    timestamp = datetime.now(timezone.utc).isoformat()
    spec = ADAPTER_SPECS[adapter_name]
    # Parse role and tenant from adapter name (pattern: <role>-<tenant>)
    parts = adapter_name.split("-", 1)
    role = parts[0] if len(parts) >= 1 else adapter_name
    tenant = parts[1] if len(parts) >= 2 else "pointsav"
    corpus_prefix = spec["glob"].split("/**")[0]
    marker = {
        "adapter": adapter_name,
        "tenant": tenant,
        "role": role,
        "corpus_path": str(CORPUS_ROOT / corpus_prefix),
        "method": spec["method"],
        "training_method": spec["method"],
        "version": 1,
        "tuple_count": len(files),
        "triggered_at": timestamp,
        "sample_files": [f.name for f in files[:5]],
    }
    PENDING_DIR.mkdir(parents=True, exist_ok=True)
    marker_path = PENDING_DIR / f"{adapter_name}-{timestamp[:10]}.json"
    if dry_run:
        print(f"    [DRY-RUN] Would write marker: {marker_path}")
        print(f"    [DRY-RUN] {json.dumps(marker, indent=6)}")
        return marker_path
    with open(marker_path, "w") as f:
        json.dump(marker, f, indent=2)
    return marker_path


def trigger_training_cycle(adapter_name: str, files: list, dry_run: bool = False) -> bool:
    """Write GCS training marker. Returns True if marker was dispatched."""
    import subprocess

    spec = ADAPTER_SPECS[adapter_name]
    marker_path = write_pending_marker(adapter_name, files, dry_run=dry_run)
    if not dry_run:
        print(f"    [MARKER] Written: {marker_path}")

    gcs_bucket = os.environ.get("SLM_YOYO_WEIGHTS_GCS_BUCKET", "")
    if not gcs_bucket:
        print(f"    [TRAIN] SLM_YOYO_WEIGHTS_GCS_BUCKET not set — marker local only ({marker_path.name})")
        return True

    if dry_run:
        print(f"    [DRY-RUN] Would sync corpus to gs://{gcs_bucket}/training-corpus/ + upload marker")
        return True

    corpus_prefix = spec["glob"].split("/**")[0]
    try:
        subprocess.run(
            ["gcloud", "storage", "cp", "-r",
             str(CORPUS_ROOT / corpus_prefix) + "/",
             f"gs://{gcs_bucket}/training-corpus/{corpus_prefix}/"],
            check=True, capture_output=True
        )
        # Sync DPO feedback pairs (chosen/rejected/prompt) alongside shadow corpus
        feedback_dir = CORPUS_ROOT / "feedback"
        if feedback_dir.exists():
            subprocess.run(
                ["gcloud", "storage", "rsync",
                 str(feedback_dir),
                 f"gs://{gcs_bucket}/training-corpus/feedback/"],
                check=True, capture_output=True
            )
        subprocess.run(
            ["gcloud", "storage", "cp", str(marker_path),
             f"gs://{gcs_bucket}/training-pending/{marker_path.name}"],
            check=True, capture_output=True
        )
        print(f"    [TRAIN] Corpus + feedback synced → gs://{gcs_bucket}/training-pending/{marker_path.name}")
        _start_trainer_vm()
        return True
    except subprocess.CalledProcessError as e:
        print(f"    [TRAIN] GCS dispatch failed: {e}")
        print(f"    [TRAIN] Marker remains local: {marker_path}")
        return True  # local marker still allows manual pickup


def _start_trainer_vm() -> None:
    """Start yoyo-batch if it is stopped. No-op if already running."""
    # Kill switch: touch /srv/foundry/data/yoyo-disabled to suppress all VM starts.
    kill_switch = Path("/srv/foundry/data/yoyo-disabled")
    if kill_switch.exists():
        print(f"    [VM] kill switch present ({kill_switch}) — VM start suppressed")
        return
    vm_name = os.environ.get("SLM_TRAINER_VM_NAME", "yoyo-batch")
    vm_zone = os.environ.get("SLM_TRAINER_VM_ZONE", "us-central1-a")
    if not vm_name:
        return
    try:
        result = subprocess.run(
            ["gcloud", "compute", "instances", "describe", vm_name,
             "--zone", vm_zone, "--format=get(status)"],
            check=True, capture_output=True, text=True
        )
        status = result.stdout.strip()
        if status == "RUNNING":
            print(f"    [VM] {vm_name} already RUNNING — startup script will drain GCS markers")
            return
        subprocess.run(
            ["gcloud", "compute", "instances", "start", vm_name, "--zone", vm_zone],
            check=True, capture_output=True
        )
        print(f"    [VM] {vm_name} started — will auto-run training on boot (80-min cap)")
    except subprocess.CalledProcessError as e:
        print(f"    [VM] Could not start {vm_name}: {e} — training must be triggered manually")


def main() -> None:
    parser = argparse.ArgumentParser(
        description="Phase 3 corpus threshold watcher and Yo-Yo training trigger"
    )
    parser.add_argument(
        "--force", action="store_true",
        help="Trigger training regardless of threshold (Sunday 02:00 UTC cron mode)"
    )
    parser.add_argument(
        "--dry-run", action="store_true",
        help="Report counts and decisions without writing markers or calling APIs"
    )
    parser.add_argument(
        "--adapter", metavar="NAME",
        choices=list(ADAPTER_SPECS.keys()),
        help="Check only this adapter (default: all adapters)"
    )
    args = parser.parse_args()

    now = datetime.now(timezone.utc).isoformat()
    flags = []
    if args.force:
        flags.append("FORCE")
    if args.dry_run:
        flags.append("DRY-RUN")
    flag_str = f" [{', '.join(flags)}]" if flags else ""
    print(f"[{now}] Phase 3 corpus threshold check{flag_str}")
    print(f"  FOUNDRY_ROOT    = {FOUNDRY_ROOT}")
    print(f"  CORPUS_ROOT     = {CORPUS_ROOT}")
    gcs_bucket = os.environ.get("SLM_YOYO_WEIGHTS_GCS_BUCKET", "(not set — local marker only)")
    print(f"  GCS_BUCKET      = {gcs_bucket}")
    print()

    any_triggered = False

    for adapter_name, spec in ADAPTER_SPECS.items():
        if args.adapter and adapter_name != args.adapter:
            continue

        if spec["method"] == "dpo":
            files = count_valid_dpo_files(spec["glob"])
            all_files = count_files(spec["glob"])
            degenerate = len(all_files) - len(files)
        else:
            files = count_files(spec["glob"])
            degenerate = 0
        count = len(files)
        threshold = spec["threshold"]
        at_threshold = count >= threshold

        print(f"  [{adapter_name}]")
        print(f"    tuples:      {count} / {threshold} threshold (valid DPO signal)")
        if degenerate:
            print(f"    degenerate:  {degenerate} skipped (attempt.diff empty — Tier B was unavailable)")
        print(f"    method:      {spec['method']}")
        print(f"    description: {spec['description']}")

        trigger = at_threshold or args.force
        if not trigger:
            print(f"    status:      accumulating — {threshold - count} more tuples needed")
            print()
            continue

        any_triggered = True
        reason = "threshold reached" if at_threshold else "forced (Sunday cron)"
        print(f"    status:      READY — {reason}")

        trigger_training_cycle(adapter_name, files, dry_run=args.dry_run)
        print()

    if not any_triggered:
        print("  No adapters at threshold. No training triggered.")

    sys.exit(0)


if __name__ == "__main__":
    main()
