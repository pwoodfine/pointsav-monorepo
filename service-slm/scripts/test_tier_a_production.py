#!/usr/bin/env python3
"""
Test Tier A extraction using the exact production code path:
  - EXTRACTION_SYSTEM_PROMPT from service-content/src/main.rs (lines 29-73)
  - Pre-fill trick: assistant message "[{\""
  - entity_filter.rs deterministic backstop rules
Calls llama-server directly at 127.0.0.1:8080 (same as call_tier_a_extract).
"""
import json
import re
import sys
import time
import urllib.request
import urllib.error

LLAMA_ENDPOINT = "http://127.0.0.1:8080/v1/chat/completions"
TIMEOUT = 500

# Exact copy of EXTRACTION_SYSTEM_PROMPT from service-content/src/main.rs
EXTRACTION_SYSTEM_PROMPT = """Extract named entities from the text below. Classify each entity into exactly one category.
Categories:
  Person    — a named human individual who appears in the text.
  Company   — a registered organisation or business named in the text.
  Project   — a named software crate, infrastructure service, or engineering initiative named in the text.
  Account   — a named financial account, service account reference, or contract identifier in the text.
  Location  — a specific named geographic place or address (city, region, street address). NOT a generic spatial noun.
Omit:
  - Software licences and SPDX identifiers (Apache-2.0, MIT, GPL-3.0, BSL-1.1). These are not companies.
  - Programming languages, file formats, and protocol names (Rust, JSON, HTTP) unless they name a specific product.
  - Shell environment variables and config symbols: $VAR_NAME, SLM_DATA_DIR, FOUNDRY_ARCHIVE_NAME — OMIT.
  - Code identifiers: backtick-quoted terms (`ghi_kwh_m2_yr`), snake_case names without spaces (service_content), file paths (./build.sh, src/main.rs, create-snapshot.sh), call expressions (log(x), ops(slm), func()), and build tool commands (cargo, npm, make, git). OMIT ALL, including any project name appearing as a CLI argument (-p slm-doorman-server, --crate service-content).
  - Commit-message prefixes of the form type(scope): ops(slm), feat(cache), fix(auth), chore(db) — these are NOT projects or accounts. OMIT.
  - Statistical notation (α, β, γ, R², p-value) and mathematical symbols.
  - Laws, regulations, and dates.
  - Generic technical concepts not attached to a proper name: "software-as-a-service (SaaS)", "Hyperscaler", "real-time operating system (RTOS)", "distributed ledger technology". These are descriptors, not entities. OMIT.
  - Placeholder values: "not specified", "N/A", "unknown", "TBD", "none", "null". OMIT.
  - Generic spatial or role phrases that are not proper place names. A Location must be a specific named place.
    EXCLUDE: "retail anchor location", "downtown core", "the site", "trade area".
    INCLUDE: "Murfreesboro, Tennessee", "Billings, Montana", "Chicago".
  - Sentence fragments, clauses, or lists: any name containing a comma, " and ", or starting with "the", "a", "an", "this". OMIT.
  - Any entity whose name appears only in these instructions, not in the text.
Country names: when a country appears as an entity, classify it as Location, NEVER as Company. "Portugal" → Location.
Hard constraint: entity_name must be a short proper noun or proper-noun phrase. Maximum eight words.
A token that looks like a proper noun is not automatically an entity. If it is a licence, a format, a generic descriptor, or a code identifier, omit it rather than forcing it into Company or Location.
If an entity does not clearly fit one category, omit it rather than guessing.
Return only a JSON array. Each element must have exactly two fields: "classification" and "entity_name".

Examples:
Text: Jennifer Woodfine is managing director at Woodfine Management Corp. in Vancouver, Canada.
Output: [{"classification":"Person","entity_name":"Jennifer Woodfine"},{"classification":"Company","entity_name":"Woodfine Management Corp."},{"classification":"Location","entity_name":"Vancouver"}]

Text: The cluster contains service-fs, not service-research. Let me explore the actual structure.
Output: [{"classification":"Project","entity_name":"service-fs"}]

Text: ops(slm): update drain predicate — remove !tier_a_first guard
Output: []

Text: Woodfine Management Corp. uses service-content and service-slm for extraction; service-bim is not yet active.
Output: [{"classification":"Company","entity_name":"Woodfine Management Corp."},{"classification":"Project","entity_name":"service-content"},{"classification":"Project","entity_name":"service-slm"}]

Text: The panic is at service-slm/crates/slm-doorman-server/src/http.rs:1302:9.
Output: []

Text: Run cargo clippy -p slm-doorman-server -- -D warnings to check for lint errors.
Output: []

Text: Peter Woodfine approved moving the yoyo-batch instance to us-central1-b.
Output: [{"classification":"Person","entity_name":"Peter Woodfine"},{"classification":"Location","entity_name":"us-central1-b"}]

Text: The automation bot triggered the outbox status check and corpus pipeline.
Output: []

If no entities are found, return an empty array []."""

ALLOWED_CLASSIFICATIONS = {"Person", "Company", "Project", "Account", "Location"}
PATH_SUFFIXES = (".sh", ".py", ".rs", ".md", ".json", ".jsonl", ".conf", ".toml", ".yaml", ".service")
ABSTRACT_NOUNS = {
    "framework", "model", "hypothesis", "hypotheses", "pipeline", "approach",
    "process", "mechanism", "algorithm", "methodology", "criterion", "criteria",
    "paradigm", "construct", "abstraction", "concept", "system", "architecture",
}
FRAGMENT_STARTERS = (
    "the ", "a ", "an ", "this ", "all ", "any ", "each ", "most ", "some ",
    "these ", "those ", "section ", "for ", "of ",
)
PLACEHOLDERS = {"not specified", "n/a", "unknown", "tbd", "none", "null"}


def is_commit_prefix(name: str) -> bool:
    t = name.strip()
    if "(" not in t or not t.endswith(")"):
        return False
    open_i = t.index("(")
    head = t[:open_i]
    scope = t[open_i+1:-1]
    return bool(head) and bool(scope) and all(c.isalnum() or c in "_-" for c in head)


def is_noise_entity_name(name: str) -> bool:
    t = name.strip()
    if not t:
        return True
    lower = t.lower()
    if lower in PLACEHOLDERS:
        return True
    if t.startswith("`") and t.endswith("`"):
        return True
    if t.startswith("$"):
        return True
    if "*" in t or "/" in t:
        return True
    if any(lower.endswith(s) for s in PATH_SUFFIXES):
        return True
    if "(" in t and ")" in t:
        return True
    if " " not in t and "_" in t:
        return True
    if t[0].isdigit():
        return True
    if any(lower.startswith(p) for p in FRAGMENT_STARTERS):
        return True
    if ", " in t or " and " in lower:
        return True
    if " " not in t and lower in ABSTRACT_NOUNS:
        return True
    return False


KNOWN_PLACES = {
    "portugal", "spain", "france", "germany", "italy", "netherlands", "belgium",
    "austria", "switzerland", "poland", "sweden", "norway", "denmark", "finland",
    "united kingdom", "united states", "canada", "australia", "mexico", "brazil",
    "argentina", "india", "china", "japan", "south korea", "singapore",
    "hong kong", "new zealand",
}


def coerce_classification(name: str, cls: str):
    lower = name.lower()
    if cls == "Company" and lower in KNOWN_PLACES:
        return "Location"
    if cls == "Project" and "/" in name:
        return None
    if cls == "Project" and " " in name and all(c.islower() or c == " " for c in name):
        return None
    if (cls == "Account" and len(name) >= 2
            and all(c.isupper() or c == "_" or c.isdigit() for c in name)
            and any(c.isupper() for c in name)):
        return None
    # Lowercase-only Account (spaces or hyphens only) → reject (abstract noise phrase).
    # Covers both "outbox status" (space) and "service-content" (hyphen) cases.
    # Real account IDs contain digits, colons, or uppercase letters.
    if (cls == "Account"
            and all(c.islower() or c in " -" for c in name)
            and (" " in name or "-" in name)):
        return None
    # Single-word all-lowercase Account with no structural chars → reject (generic noun).
    # Real account identifiers always have digits, colons, @, dots, or hyphens.
    if (cls == "Account"
            and " " not in name and "-" not in name and ":" not in name
            and "@" not in name and "." not in name
            and all(c.islower() for c in name)):
        return None
    return cls


def apply_filter(raw: list) -> list:
    out = []
    for e in raw:
        name = e.get("entity_name") or ""
        cls = e.get("classification") or ""
        if not name or not isinstance(name, str):
            continue
        if is_commit_prefix(name) or is_noise_entity_name(name):
            continue
        if len(name.split()) > 8:
            continue
        coerced = coerce_classification(name, cls)
        if coerced is None or coerced not in ALLOWED_CLASSIFICATIONS:
            continue
        out.append({"classification": coerced, "entity_name": name})
    return out


def extract(text: str) -> tuple[list, float, float]:
    import subprocess
    body = {
        "messages": [
            {"role": "system", "content": EXTRACTION_SYSTEM_PROMPT},
            {"role": "user",   "content": text},
            {"role": "assistant", "content": "[{\""},
        ],
        "temperature": 0.0,
        "max_tokens": 300,
    }
    data = json.dumps(body)
    t0 = time.time()
    try:
        proc = subprocess.run(
            ["curl", "-s", "-X", "POST", LLAMA_ENDPOINT,
             "-H", "Content-Type: application/json",
             "-d", data,
             "--max-time", str(TIMEOUT)],
            capture_output=True, text=True, timeout=TIMEOUT + 5,
        )
        elapsed = time.time() - t0
        if proc.returncode != 0 or not proc.stdout.strip():
            return None, 0.0, elapsed
        result = json.loads(proc.stdout)
    except (subprocess.TimeoutExpired, json.JSONDecodeError, Exception):
        return None, 0.0, time.time() - t0

    raw_content = result["choices"][0]["message"]["content"]
    full = '[{"' + raw_content
    try:
        parsed = json.loads(full)
    except json.JSONDecodeError:
        try:
            parsed = json.loads(full + "]")
        except json.JSONDecodeError:
            parsed = []

    usage = result.get("usage", {})
    tokens = usage.get("completion_tokens", 0)
    tps = tokens / elapsed if elapsed > 0 else 0.0
    return parsed, tps, elapsed


def run_tests():
    TESTS = [
        # ── Core entity types ──────────────────────────────────────────────────
        ("core", "Person + Company + Location",
         "Jennifer Woodfine is managing director at Woodfine Management Corp. in Vancouver, Canada.",
         [("Company","Woodfine Management Corp."), ("Location","Vancouver"), ("Person","Jennifer Woodfine")]),

        ("core", "Project names only",
         "service-vm-fleet and service-vm-host were promoted to canonical this morning.",
         [("Project","service-vm-fleet"), ("Project","service-vm-host")]),

        # ── Negation ──────────────────────────────────────────────────────────
        ("negation", "Negation trap — exclude service-research",
         "The cluster contains service-fs, not service-research. Let me explore the actual structure.",
         [("Project","service-fs")]),

        ("negation", "Negated person excluded",
         "This commit was not authored by Peter Woodfine — it came from the automation bot.",
         []),

        ("negation", "Conditional exclusion — service-bim not active",
         "service-content handles extraction; service-bim is not yet active.",
         [("Project","service-content")]),

        # ── Noise rejection ───────────────────────────────────────────────────
        ("noise", "Git commit prefix → []",
         "ops(slm): update drain predicate — remove !tier_a_first guard",
         []),

        ("noise", "Env var name → []",
         "Set SERVICE_CONTENT_TIER_A_GRAMMAR=json_schema and restart local-content.service.",
         []),

        ("noise", "Abstract tech nouns → []",
         "The outbox status shows the corpus payload key is missing from the enrichment queue.",
         []),

        ("noise", "Shell command → []",
         "Run cargo clippy -p slm-doorman-server -- -D warnings to check for lint errors.",
         []),

        ("noise", "Rust path fragment → []",
         "The panic is at service-slm/crates/slm-doorman-server/src/http.rs:1302:9.",
         []),

        # ── Workspace-specific ────────────────────────────────────────────────
        ("workspace", "GCP zone as Location",
         "Peter Woodfine approved moving the yoyo-batch instance to us-central1-b.",
         [("Location","us-central1-b"), ("Person","Peter Woodfine")]),

        ("workspace", "Mixed: company + service names",
         "Woodfine Management Corp. uses service-content and service-slm for entity extraction.",
         [("Company","Woodfine Management Corp."), ("Project","service-content"), ("Project","service-slm")]),

        # ── Edge cases ────────────────────────────────────────────────────────
        ("edge", "Doorman — named system component",
         "The Doorman routes all inference requests through its circuit breaker.",
         [("Project","Doorman")]),

        ("edge", "Multi-entity GCP + people",
         "Mathew provisioned the yoyo-batch GPU VM in us-central1-a for PointSav Digital Systems.",
         [("Company","PointSav Digital Systems"), ("Location","us-central1-a"), ("Person","Mathew")]),
    ]

    totals = {"pass": 0, "fail": 0}
    by_cat = {}

    for (cat, label, text, expected_list) in TESTS:
        expected = set(expected_list)
        raw, tps, elapsed = extract(text)

        if raw is None:
            print(f"  ❌ TIMEOUT  [{cat}] {label}")
            totals["fail"] += 1
            by_cat.setdefault(cat, {"pass":0,"fail":0})["fail"] += 1
            continue

        filtered = apply_filter(raw)
        got = {(e["classification"], e["entity_name"]) for e in filtered}

        passed = got == expected
        symbol = "✅" if passed else "❌"
        tps_s = f"{tps:.1f} tok/s" if tps > 0 else "—"
        print(f"  {symbol} [{cat}] {label} ({elapsed:.1f}s, {tps_s})")
        if not passed:
            missing = expected - got
            extra   = got - expected
            if missing:
                print(f"       missing: {sorted(missing)}")
            if extra:
                print(f"       extra:   {sorted(extra)}")
            print(f"       raw model: {raw}")

        totals["pass" if passed else "fail"] += 1
        by_cat.setdefault(cat, {"pass":0,"fail":0})["pass" if passed else "fail"] += 1

    print()
    print("=" * 60)
    print(f"TOTAL: {totals['pass']}/{totals['pass']+totals['fail']} passed")
    print()
    for cat, counts in by_cat.items():
        total_c = counts["pass"] + counts["fail"]
        status = "✅" if counts["fail"] == 0 else "❌"
        print(f"  {status} {cat}: {counts['pass']}/{total_c}")

    verdict = "PASS" if totals["fail"] == 0 else (
        "PARTIAL" if totals["pass"] > 0 else "FAIL"
    )
    print()
    print(f"Verdict: {verdict}")
    return totals["fail"] == 0


if __name__ == "__main__":
    print("OLMo 3 Tier A — Production code path test")
    print(f"Endpoint: {LLAMA_ENDPOINT}")
    print(f"Timeout:  {TIMEOUT}s per call")
    print("=" * 60)
    ok = run_tests()
    sys.exit(0 if ok else 1)
