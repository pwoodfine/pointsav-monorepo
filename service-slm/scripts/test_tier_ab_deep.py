#!/usr/bin/env python3
"""
test_tier_ab_deep.py — Deep Tier A / Tier B extraction quality test.

Runs every test case through both tiers with full intermediate logging:
  - Raw model output (before pre-fill restoration)
  - Pre-fill restored JSON
  - Every filter step with decision reason
  - Final filtered entities vs expected
  - Pass / fail with diff

Saves structured JSONL log to test_results_deep_<timestamp>.jsonl for
post-run analysis and codebase improvement recommendations.

Usage:
    python3 test_tier_ab_deep.py               # Tier A only
    python3 test_tier_ab_deep.py --tier-b      # Tier A + Tier B
    python3 test_tier_ab_deep.py --tier-b-only # Tier B only

Tier B endpoint read from env SLM_YOYO_ENDPOINT + SLM_YOYO_BEARER
(same as Doorman config at /etc/local-doorman/local-doorman.env).
"""

import argparse
import json
import os
import subprocess
import sys
import time
from datetime import datetime, timezone

# ── Endpoints ────────────────────────────────────────────────────────────────
TIER_A_ENDPOINT = os.environ.get(
    "SLM_LOCAL_ENDPOINT", "http://127.0.0.1:8080/v1/chat/completions"
)
TIER_B_ENDPOINT = os.environ.get("SLM_YOYO_ENDPOINT", "").rstrip("/")
TIER_B_BEARER = os.environ.get("SLM_YOYO_BEARER", "")
if TIER_B_ENDPOINT and not TIER_B_ENDPOINT.endswith("/v1/chat/completions"):
    TIER_B_ENDPOINT = TIER_B_ENDPOINT + "/v1/chat/completions"

TIMEOUT = 500
TEMPERATURE = 0
MAX_TOKENS = 200

# ── System prompt (must match service-content/src/main.rs exactly) ───────────
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

# ── Filter logic (mirrors entity_filter.rs exactly) ──────────────────────────
ALLOWED_CLASSIFICATIONS = {"Person", "Company", "Project", "Account", "Location"}

KNOWN_PLACES = {
    "portugal", "spain", "france", "germany", "italy", "netherlands", "belgium",
    "austria", "switzerland", "poland", "sweden", "norway", "denmark", "finland",
    "united kingdom", "united states", "canada", "australia", "mexico", "brazil",
    "argentina", "india", "china", "japan", "south korea", "singapore",
    "hong kong", "new zealand",
}

PATH_SUFFIXES = {".sh", ".py", ".rs", ".md", ".json", ".jsonl", ".conf", ".toml", ".yaml", ".service"}

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


def filter_step_by_step(entity: dict) -> tuple[dict | None, list[str]]:
    """Apply every filter step, return (entity_or_None, [log_lines])."""
    steps = []
    name = entity.get("entity_name")
    cls = entity.get("classification")

    # null guard
    if not name or not isinstance(name, str):
        steps.append(f"  ✗ NULL/missing entity_name → REJECT")
        return None, steps
    if not cls or not isinstance(cls, str):
        steps.append(f"  ✗ NULL/missing classification → REJECT")
        return None, steps

    t = name.strip()
    lower = t.lower()

    # placeholder
    if lower in PLACEHOLDERS:
        steps.append(f"  ✗ placeholder value '{t}' → REJECT")
        return None, steps

    # backtick
    if t.startswith("`") and t.endswith("`"):
        steps.append(f"  ✗ backtick-wrapped identifier → REJECT")
        return None, steps

    # $ env var
    if t.startswith("$"):
        steps.append(f"  ✗ env var reference ($...) → REJECT")
        return None, steps

    # glob
    if "*" in t:
        steps.append(f"  ✗ glob pattern → REJECT")
        return None, steps

    # file path (contains /)
    if "/" in t:
        steps.append(f"  ✗ path separator '/' → REJECT")
        return None, steps

    # file extension
    for suf in PATH_SUFFIXES:
        if lower.endswith(suf):
            steps.append(f"  ✗ file extension '{suf}' → REJECT")
            return None, steps

    # call expression
    if "(" in t and ")" in t:
        steps.append(f"  ✗ call expression '(...)' → REJECT")
        return None, steps

    # snake_case (underscore, no space)
    if " " not in t and "_" in t:
        steps.append(f"  ✗ snake_case (underscore without space) → REJECT")
        return None, steps

    # numeric prefix
    if t and t[0].isdigit():
        steps.append(f"  ✗ numeric prefix → REJECT")
        return None, steps

    # fragment starters
    for starter in FRAGMENT_STARTERS:
        if lower.startswith(starter):
            steps.append(f"  ✗ fragment starter '{starter.strip()}' → REJECT")
            return None, steps

    # comma / ' and '
    if ", " in t or " and " in lower:
        steps.append(f"  ✗ comma/conjunction phrase → REJECT")
        return None, steps

    # single-word abstract nouns
    if " " not in t and lower in ABSTRACT_NOUNS:
        steps.append(f"  ✗ abstract noun '{lower}' → REJECT")
        return None, steps

    # commit prefix: type(scope)
    if "(" in t and t.endswith(")"):
        open_idx = t.find("(")
        head = t[:open_idx]
        if head and all(c.isalnum() or c in "_-" for c in head) and open_idx + 1 < len(t) - 1:
            steps.append(f"  ✗ commit prefix pattern type(scope) → REJECT")
            return None, steps

    # word count
    if len(t.split()) > 8:
        steps.append(f"  ✗ > 8 words → REJECT")
        return None, steps

    steps.append(f"  ✓ noise filters passed")

    # coerce_classification
    coerced = cls
    if cls == "Company" and lower in KNOWN_PLACES:
        coerced = "Location"
        steps.append(f"  ~ coerce: Company → Location (known place)")
    elif cls == "Project" and "/" in name:
        steps.append(f"  ✗ coerce: Project with path '/' → REJECT")
        return None, steps
    elif (cls == "Project" and " " in name
          and all(c.islower() or c == " " for c in name)):
        steps.append(f"  ✗ coerce: lowercase spaced Project phrase (noise) → REJECT")
        return None, steps
    elif cls == "Account":
        all_caps = all(c.isupper() or c == "_" or c.isdigit() for c in name) and any(c.isupper() for c in name) and len(name) >= 2
        if all_caps:
            steps.append(f"  ✗ coerce: ALL_CAPS Account (env var) → REJECT")
            return None, steps
        all_lower_hyphen = ((" " in name or "-" in name) and
                            all(c.islower() or c in " -" for c in name))
        if all_lower_hyphen:
            steps.append(f"  ✗ coerce: lowercase+hyphen Account (noise phrase) → REJECT")
            return None, steps
        single_word_lowercase = (
            " " not in name and "-" not in name and ":" not in name
            and "@" not in name and "." not in name
            and all(c.islower() for c in name)
        )
        if single_word_lowercase:
            steps.append(f"  ✗ coerce: single-word lowercase Account (generic noun) → REJECT")
            return None, steps

    # allowed classification
    if coerced not in ALLOWED_CLASSIFICATIONS:
        steps.append(f"  ✗ classification '{coerced}' not in allowed set → REJECT")
        return None, steps

    result = {"classification": coerced, "entity_name": name}
    if coerced != cls:
        steps.append(f"  ✓ ACCEPT as ({coerced}, {name})")
    else:
        steps.append(f"  ✓ ACCEPT as ({cls}, {name})")
    return result, steps


def call_model(endpoint: str, text: str, bearer: str = "") -> tuple[list | None, dict, float]:
    """Call inference endpoint. Returns (raw_entities, debug_info, elapsed_s)."""
    messages = [
        {"role": "system", "content": EXTRACTION_SYSTEM_PROMPT},
        {"role": "user", "content": text},
        {"role": "assistant", "content": '[{"'},
    ]
    body = json.dumps({
        "messages": messages,
        "max_tokens": MAX_TOKENS,
        "temperature": TEMPERATURE,
        "stream": False,
    })

    cmd = ["curl", "-sf", "--max-time", str(TIMEOUT),
           "-X", "POST", endpoint,
           "-H", "Content-Type: application/json"]
    if bearer:
        cmd += ["-H", f"Authorization: Bearer {bearer}"]
    cmd += ["-d", body]

    t0 = time.time()
    try:
        proc = subprocess.run(cmd, capture_output=True, text=True, timeout=TIMEOUT + 5)
        elapsed = time.time() - t0
        if proc.returncode != 0 or not proc.stdout.strip():
            return None, {"error": "curl_failed", "stderr": proc.stderr[:200]}, elapsed

        raw_response = json.loads(proc.stdout)
        raw_content = raw_response["choices"][0]["message"]["content"]
        prefill_restored = '[{"' + raw_content
        usage = raw_response.get("usage", {})
        tps = usage.get("completion_tokens", 0) / elapsed if elapsed > 0 else 0.0

        debug = {
            "raw_content": raw_content,
            "prefill_restored": prefill_restored,
            "completion_tokens": usage.get("completion_tokens", 0),
            "prompt_tokens": usage.get("prompt_tokens", 0),
            "tok_per_s": round(tps, 2),
            "elapsed_s": round(elapsed, 1),
        }

        # parse JSON
        try:
            parsed = json.loads(prefill_restored)
        except json.JSONDecodeError:
            try:
                parsed = json.loads(prefill_restored + "]")
                debug["json_repair"] = "appended ]"
            except json.JSONDecodeError:
                parsed = []
                debug["json_error"] = "parse failed"

        return parsed, debug, elapsed

    except subprocess.TimeoutExpired:
        return None, {"error": "timeout"}, time.time() - t0
    except Exception as e:
        return None, {"error": str(e)}, time.time() - t0


# ── Test cases ────────────────────────────────────────────────────────────────
TESTS = [
    ("core",      "Person + Company + Location",
     "Jennifer Woodfine is managing director at Woodfine Management Corp. in Vancouver, Canada.",
     [("Company","Woodfine Management Corp."), ("Location","Vancouver"), ("Person","Jennifer Woodfine")]),

    ("core",      "Project names only",
     "service-vm-fleet and service-vm-host were promoted to canonical this morning.",
     [("Project","service-vm-fleet"), ("Project","service-vm-host")]),

    ("negation",  "Negation trap — exclude service-research",
     "The cluster contains service-fs, not service-research. Let me explore the actual structure.",
     [("Project","service-fs")]),

    ("negation",  "Negated person excluded",
     "This commit was not authored by Peter Woodfine — it came from the automation bot.",
     []),

    ("negation",  "Conditional exclusion — service-bim not active",
     "service-content handles extraction; service-bim is not yet active.",
     [("Project","service-content")]),

    ("noise",     "Git commit prefix → []",
     "ops(slm): update drain predicate — remove !tier_a_first guard",
     []),

    ("noise",     "Env var name → []",
     "Set SERVICE_CONTENT_TIER_A_GRAMMAR=json_schema and restart local-content.service.",
     []),

    ("noise",     "Abstract tech nouns → []",
     "The outbox status shows the corpus payload key is missing from the enrichment queue.",
     []),

    ("noise",     "Shell command → []",
     "Run cargo clippy -p slm-doorman-server -- -D warnings to check for lint errors.",
     []),

    ("noise",     "Rust path fragment → []",
     "The panic is at service-slm/crates/slm-doorman-server/src/http.rs:1302:9.",
     []),

    ("workspace", "GCP zone as Location",
     "Peter Woodfine approved moving the yoyo-batch instance to us-central1-b.",
     [("Location","us-central1-b"), ("Person","Peter Woodfine")]),

    ("workspace", "Mixed: company + service names",
     "Woodfine Management Corp. uses service-content and service-slm for entity extraction.",
     [("Company","Woodfine Management Corp."), ("Project","service-content"), ("Project","service-slm")]),

    ("edge",      "Doorman — named system component",
     "The Doorman routes all inference requests through its circuit breaker.",
     [("Project","Doorman")]),

    ("edge",      "Multi-entity GCP + people",
     "Mathew provisioned the yoyo-batch GPU VM in us-central1-a for PointSav Digital Systems.",
     [("Company","PointSav Digital Systems"), ("Location","us-central1-a"), ("Person","Mathew")]),
]


def run_tier(tier_label: str, endpoint: str, bearer: str = "") -> list[dict]:
    """Run all tests against one tier. Returns list of result records."""
    print(f"\n{'='*70}")
    print(f"TIER {tier_label}  endpoint: {endpoint}")
    print(f"{'='*70}\n")

    results = []
    for cat, label, text, expected_list in TESTS:
        expected = set(expected_list)
        print(f"[{cat}] {label}")
        print(f"  Input: {text[:80]}{'…' if len(text)>80 else ''}")

        raw, debug, elapsed = call_model(endpoint, text, bearer)

        record = {
            "tier": tier_label,
            "cat": cat,
            "label": label,
            "input": text,
            "expected": list(expected_list),
            "elapsed_s": round(elapsed, 1),
            "debug": debug,
        }

        if raw is None:
            print(f"  ❌ TIMEOUT / ERROR ({elapsed:.0f}s)")
            if "error" in debug:
                print(f"     {debug['error']}")
            record.update({"outcome": "timeout", "raw_model": None, "filtered": [], "passed": False})
            results.append(record)
            print()
            continue

        print(f"  Raw model output:  {debug.get('prefill_restored', '')[:120]}")
        print(f"  Tokens: {debug.get('completion_tokens',0)} completion  "
              f"{debug.get('tok_per_s',0):.1f} tok/s  {elapsed:.1f}s")
        if debug.get("json_repair"):
            print(f"  ⚠ JSON repair: {debug['json_repair']}")
        if debug.get("json_error"):
            print(f"  ⚠ JSON error: {debug['json_error']}")

        print(f"  Filter chain:")
        filtered_entities = []
        raw_accepted = []
        for e in raw:
            accepted, steps = filter_step_by_step(e)
            name = e.get("entity_name", "?")
            cls = e.get("classification", "?")
            print(f"    [{cls}] {name!r}")
            for s in steps:
                print(f"    {s}")
            if accepted:
                filtered_entities.append((accepted["classification"], accepted["entity_name"]))
                raw_accepted.append(accepted)

        got = set(filtered_entities)
        missing = expected - got
        extra = got - expected
        passed = (got == expected)

        record.update({
            "outcome": "pass" if passed else "fail",
            "raw_model": raw,
            "filtered": filtered_entities,
            "missing": list(missing),
            "extra": list(extra),
            "passed": passed,
        })

        if passed:
            print(f"  ✅ PASS  got={sorted(got)}")
        else:
            print(f"  ❌ FAIL")
            if missing:
                print(f"     missing: {sorted(missing)}")
            if extra:
                print(f"     extra:   {sorted(extra)}")

        results.append(record)
        print()

    return results


def print_summary(results: list[dict], tier_label: str):
    by_cat: dict[str, dict] = {}
    total_pass = 0
    for r in results:
        cat = r["cat"]
        by_cat.setdefault(cat, {"pass": 0, "fail": 0, "timeout": 0})
        if r["passed"]:
            by_cat[cat]["pass"] += 1
            total_pass += 1
        elif r["outcome"] == "timeout":
            by_cat[cat]["timeout"] += 1
        else:
            by_cat[cat]["fail"] += 1

    n = len(results)
    print(f"\n{'='*50}")
    print(f"TIER {tier_label} SUMMARY: {total_pass}/{n}")
    for cat, counts in by_cat.items():
        icon = "✅" if counts["fail"] == 0 and counts["timeout"] == 0 else "❌"
        total_cat = counts["pass"] + counts["fail"] + counts["timeout"]
        detail = ""
        if counts["timeout"]:
            detail = f" ({counts['timeout']} timeout)"
        print(f"  {icon} {cat}: {counts['pass']}/{total_cat}{detail}")
    print(f"{'='*50}")
    return total_pass


def print_comparison(results_a: list[dict], results_b: list[dict]):
    print(f"\n{'='*70}")
    print("TIER A vs TIER B COMPARISON")
    print(f"{'='*70}")
    print(f"{'Test':<45} {'Tier A':>8} {'Tier B':>8}")
    print("-" * 65)
    for a, b in zip(results_a, results_b):
        a_icon = "✅" if a["passed"] else ("⏱" if a["outcome"]=="timeout" else "❌")
        b_icon = "✅" if b["passed"] else ("⏱" if b["outcome"]=="timeout" else "❌")
        label = a["label"][:44]
        print(f"  {label:<44} {a_icon:>7}  {b_icon:>7}")
    print()


def save_jsonl(results: list[dict], path: str):
    with open(path, "w") as f:
        for r in results:
            f.write(json.dumps(r) + "\n")
    print(f"\nResults saved to: {path}")


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--tier-b", action="store_true", help="Also test Tier B")
    parser.add_argument("--tier-b-only", action="store_true", help="Only test Tier B")
    parser.add_argument("--out", default="", help="Output JSONL path (default: auto)")
    args = parser.parse_args()

    ts = datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%SZ")
    out_path = args.out or os.path.join(
        os.path.dirname(__file__),
        f"test_results_deep_{ts}.jsonl"
    )

    all_results = []

    if not args.tier_b_only:
        results_a = run_tier("A", TIER_A_ENDPOINT)
        print_summary(results_a, "A")
        all_results.extend(results_a)

    results_b = None
    if args.tier_b or args.tier_b_only:
        if not TIER_B_ENDPOINT:
            print("\n⚠ Tier B: SLM_YOYO_ENDPOINT not set — skipping")
        else:
            results_b = run_tier("B", TIER_B_ENDPOINT, TIER_B_BEARER)
            print_summary(results_b, "B")
            all_results.extend(results_b)

    if results_b and not args.tier_b_only:
        print_comparison(results_a, results_b)

    save_jsonl(all_results, out_path)
    print("\nAnalysis tip: look for patterns in test_results_deep_*.jsonl —")
    print("  - Raw model output vs filtered: what is the filter rejecting vs the model doing?")
    print("  - Timeout patterns: which tests always timeout? (multi-entity = CPU bottleneck)")
    print("  - Tier A vs Tier B diff: which failure modes each tier has")

    n_pass = sum(1 for r in all_results if r["passed"])
    sys.exit(0 if n_pass == len(all_results) else 1)


if __name__ == "__main__":
    main()
