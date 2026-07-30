#!/usr/bin/env python3
"""
Build extraction SFT pairs from human-curated YAML ledgers.

RAFT-style: top-k entity candidates from people.csv (pipe-delimited, positional) as context.
Output: /srv/foundry/data/training-corpus/extraction/jennifer-sft-<stem>.jsonl
provenance: "human-curated"

ToS boundary: NO Claude/Tier C outputs in instruction or output fields. Human labels only.
SYS-ADR-07: CSVs are read as prose, not passed as structured data through AI.
"""
import os, sys, csv, json, pathlib, datetime

JENNIFER_LEDGER_DIR = pathlib.Path(os.environ.get(
    "JENNIFER_LEDGER_DIR",
    "/srv/foundry/deployments/cluster-totebox-jennifer/service-research/ledger",
))
JENNIFER_ASSETS_DIR = pathlib.Path(os.environ.get(
    "JENNIFER_ASSETS_DIR",
    "/srv/foundry/deployments/cluster-totebox-jennifer/service-research/assets",
))
PEOPLE_CSV = pathlib.Path(os.environ.get(
    "PEOPLE_CSV",
    "/srv/foundry/deployments/cluster-totebox-jennifer/service-people/people.csv",
))
OUTPUT_DIR = pathlib.Path(os.environ.get(
    "OUTPUT_DIR",
    "/srv/foundry/data/training-corpus/extraction",
))
TOP_K = 20
MODULE_ID = "jennifer"
BRIEF_ID = "jennifer-2-ingest-pipeline"

SKIP_MARKERS = ("extraction_protocol", "fidelity_mandate", "EXTRACTION_PROTOCOL")
MEDIA_STUB_KEYS = {"asset_type", "filename", "processed", "sha256"}


def normalize_reference_yaml(path):
    """
    Normalize heterogeneous YAML to {entities, metrics, themes}.

    Actual jennifer-1 schema (Opus audit 5, 2026-06-14):
      - entities: rare (1-3/461 files); sub-fields 'name' + 'type' (may be list)
      - metrics: ~212/461; sub-field 'metric_name' (NOT 'name')
      - themes: ~212/461 as 'theme_alignment'; also 'woodfine_institutional_themes'
      - canonical_name/entity_name/entity_type NEVER appear at entity level
    """
    try:
        import yaml
    except ImportError:
        import subprocess, sys as _sys
        subprocess.check_call([_sys.executable, "-m", "pip", "install", "pyyaml", "-q"])
        import yaml

    try:
        text = path.read_text(encoding="utf-8")
    except Exception:
        return None
    if len(text) < 60:
        return None
    for marker in SKIP_MARKERS:
        if marker in text:
            return None
    try:
        data = yaml.safe_load(text)
    except Exception:
        return None
    if not isinstance(data, dict):
        return None
    # Skip media stubs (filename/sha256/processed only)
    if set(data.keys()) & MEDIA_STUB_KEYS and len(data) <= 6:
        return None

    def unwrap(d):
        for wrapper in ("document_analysis", "article_metadata", "institutional_analysis",
                        "woodfine_analysis", "analysis"):
            if wrapper in d and isinstance(d[wrapper], dict):
                return d[wrapper]
        return d

    block = unwrap(data)

    # Entities (rare but handle)
    raw_entities = block.get("entities") or data.get("entities") or []
    if not isinstance(raw_entities, list):
        raw_entities = []
    norm_entities = []
    for e in raw_entities:
        if isinstance(e, dict):
            name = e.get("name") or e.get("entity_name") or e.get("canonical_name")
            if name:
                etype = e.get("type") or e.get("entity_type") or "UNKNOWN"
                if isinstance(etype, list):
                    etype = etype[0] if etype else "UNKNOWN"
                norm_entities.append({"name": str(name), "entity_type": str(etype)})
        elif isinstance(e, str) and e.strip():
            norm_entities.append({"name": e.strip(), "entity_type": "UNKNOWN"})

    # Metrics — three field name variants seen in the wild: metric_name, name, metric
    raw_metrics = block.get("metrics") or data.get("metrics") or []
    if not isinstance(raw_metrics, list):
        raw_metrics = []
    norm_metrics = []
    for m in raw_metrics:
        if isinstance(m, dict):
            # Bug A fix: add "metric" as a fallback key (was dropping 39% of metrics)
            mname = m.get("metric_name") or m.get("name") or m.get("metric")
            if mname:
                norm_metrics.append({
                    "metric_name": str(mname),
                    "value": str(m.get("value", "")),
                    "unit": str(m.get("unit", "")),
                })
        elif isinstance(m, str) and m.strip():
            norm_metrics.append({"metric_name": m.strip(), "value": "", "unit": ""})

    # Themes — multiple possible field names; stop at first non-empty hit
    norm_themes = []
    for theme_key in ("theme_alignment", "woodfine_institutional_themes",
                      "article_themes", "woodfine_themes", "themes", "institutional_themes"):
        raw = block.get(theme_key) or data.get(theme_key)
        if isinstance(raw, list):
            for t in raw:
                # Bug B fix: dict-themes must be flattened to a string, not repr'd.
                # A dict like {theme, relevance, sub_themes} → extract the theme value.
                if isinstance(t, dict):
                    ts = str(t.get("theme") or t.get("name") or next(iter(t.values()), "")).strip()
                else:
                    ts = str(t).strip()
                if ts and ts not in ("No themes loaded.", "", "[]"):
                    norm_themes.append(ts)
        if norm_themes:
            break

    if not norm_entities and not norm_metrics and not norm_themes:
        return None
    return {"entities": norm_entities, "metrics": norm_metrics, "themes": norm_themes}


def load_people(csv_path):
    """
    Load people.csv → list of canonical names for RAFT candidate injection.
    Format: pipe-delimited, positional (row[0]=name, row[1]=type, row[2]=source).
    Header row is skipped.
    """
    names = []
    try:
        with open(csv_path, encoding="utf-8") as f:
            reader = csv.reader(f, delimiter="|")
            next(reader, None)
            for row in reader:
                if row and len(row) >= 1 and row[0].strip():
                    names.append(row[0].strip())
    except FileNotFoundError:
        print(f"[WARN] people.csv not found at {csv_path} — RAFT injection empty", file=sys.stderr)
    except Exception as e:
        print(f"[WARN] Could not load people.csv: {e}", file=sys.stderr)
    return names


def top_k_candidates(doc_content, all_names, k=TOP_K):
    """Score people-names by presence in the document text, not the filename.
    Filename-token scoring produced near-random noise for documents whose stems
    didn't overlap with names (Audit 3, 2026-06-14). Document-text matching finds
    names that actually appear or are closely referenced in the document body.
    The random-pad branch is removed — an empty list beats misleading noise.
    """
    doc_lower = doc_content.lower()
    scored = []
    for name in all_names:
        # Score by how many tokens of the name appear in the document text.
        name_tokens = name.lower().split()
        if not name_tokens:
            continue
        matches = sum(1 for tok in name_tokens if len(tok) > 2 and tok in doc_lower)
        if matches > 0:
            scored.append((matches, name))
    scored.sort(key=lambda x: -x[0])
    return [n for _, n in scored[:k]]


def main():
    OUTPUT_DIR.mkdir(parents=True, exist_ok=True)
    all_names = load_people(PEOPLE_CSV)
    print(f"Loaded {len(all_names)} people for RAFT candidate injection")

    if not JENNIFER_LEDGER_DIR.exists():
        print(f"ERROR: ledger dir not found at {JENNIFER_LEDGER_DIR}", file=sys.stderr)
        sys.exit(1)
    if not JENNIFER_ASSETS_DIR.exists():
        print(f"WARNING: assets dir not found at {JENNIFER_ASSETS_DIR}", file=sys.stderr)

    written = skipped_empty = skipped_leak = skipped_no_asset = 0
    created_ts = datetime.datetime.utcnow().isoformat() + "Z"

    for yaml_path in sorted(JENNIFER_LEDGER_DIR.glob("*.yaml")):
        stem = yaml_path.stem
        try:
            text_check = yaml_path.read_text(encoding="utf-8")
        except Exception:
            skipped_empty += 1
            continue

        if any(m in text_check for m in SKIP_MARKERS):
            skipped_leak += 1
            continue

        norm = normalize_reference_yaml(yaml_path)
        if norm is None:
            skipped_empty += 1
            continue

        asset_path = JENNIFER_ASSETS_DIR / f"{stem}.md"
        if not asset_path.exists():
            candidates = list(JENNIFER_ASSETS_DIR.glob(f"{stem[:40]}*.md"))
            if not candidates:
                skipped_no_asset += 1
                continue
            asset_path = candidates[0]

        try:
            doc_content = asset_path.read_text(encoding="utf-8", errors="replace")[:8000]
        except Exception:
            skipped_no_asset += 1
            continue

        candidates = top_k_candidates(doc_content, all_names, TOP_K)
        candidate_str = ", ".join(candidates) if candidates else "(none matched)"

        instruction = (
            "Extract named entities, metrics, and themes from the following document.\n"
            "Return a JSON object with exactly three keys:\n"
            "  entities: list of {\"name\": string, \"entity_type\": string}\n"
            "    entity_type must be one of: Person, Company, Project, Account, Location\n"
            "  metrics:  list of {\"metric_name\": string, \"value\": string, \"unit\": string}\n"
            "  themes:   list of strings (short descriptive phrases)\n"
            "Omit any key whose list would be empty.\n"
            f"Candidate named entities found in this document: {candidate_str}\n\n"
            f"Document:\n\n{doc_content}"
        )
        output = json.dumps(norm, ensure_ascii=False)

        record = {
            "task_type": "entity-extraction-sft",
            "brief_id": BRIEF_ID,
            "instruction": instruction,
            "output": output,
            "source_doc": str(asset_path),
            "module_id": MODULE_ID,
            "provenance": "human-curated",
            "created": created_ts,
        }

        out_path = OUTPUT_DIR / f"jennifer-sft-{stem}.jsonl"
        out_path.write_text(json.dumps(record, ensure_ascii=False) + "\n", encoding="utf-8")
        written += 1

    print(f"Done: {written} written, {skipped_empty} skipped-empty/no-labels, "
          f"{skipped_leak} skipped-leak, {skipped_no_asset} skipped-no-asset")


if __name__ == "__main__":
    main()
