#!/usr/bin/env bash
# score-gate.sh — scored promotion gate: direction-aware quality check against a
# curated held-out set.
#
# Why this exists: deploy-gate.sh (same directory) proves an adapter is NOT a
# no-op — it answers "did output change." It cannot answer "did output get
# BETTER," because an adapter that made every response worse would pass the
# same delta check. This script answers that second, harder question by
# scoring the adapter's own completions on a fixed set of task-quality checks,
# not by diffing against a baseline.
#
# Reuses deploy-gate.sh's scratch-server scale-toggle infrastructure via
# _gate-common.sh (same script, same protocol, same "never touches production
# :8080" discipline) — only the probe source and scoring logic differ.
#
# Scoring components (per probe, adapter active at scale 1.0):
#   1. diff-parse       — does the completion contain a ```diff fenced block
#                          (same regex as apprenticeship.rs's extract_diff_block)
#   2. git-apply-check   — does the extracted diff parse via `git apply --check
#                          --stat` (same pattern as refilter-dpo.py's
#                          _git_apply_check). Run from this repo's working tree —
#                          a diff against a path outside this tree will correctly
#                          fail extraction plausibility for reasons unrelated to
#                          diff syntax; this is a known limitation inherited from
#                          the existing corpus-quality-filter pattern, not new.
#   3. envelope-format   — does the completion have the canonical response
#                          envelope: YAML frontmatter (self_confidence: <float in
#                          [0,1]>, escalate: <bool>) then "## Reasoning" then
#                          "## Diff" in order (matches apprenticeship.rs's
#                          render_canonical_response / export-sft.py's
#                          build_completion, the format the model was trained on)
#   4. entity-F1         — NOT IMPLEMENTED. service-gliner/eval/ (gold set +
#                          scorer) doesn't exist yet — that's a P1 roadmap item
#                          from the first Fable audit, not yet built. Reported as
#                          {"available": false, "reason": "..."} rather than
#                          silently omitted or faked as 0.
#
# recommend_promotion is true only if diff-parse and git-apply pass rates both
# clear PASS_RATE_THRESHOLD — envelope-format is reported but does not gate the
# boolean (secondary quality signal, not a correctness signal). Adjustable via
# --pass-rate; not load-bearing product policy, just this script's default.
#
# Held-out set: defaults to eval-prepare.py's fresh output
# (data/training-corpus/eval/holdout-v1.jsonl, 388 rows, `prompt`/`expected`
# schema — `prompt` is already fully formatted). Reconciled 2026-07-04: this
# script previously defaulted to the older, stale
# data/corpus/eval/holdout.jsonl (76 rows, `instruction`/`brief_id`/`task_type`
# schema, built the same way export-sft.py's build_instruction() builds
# training prompts) — both schemas are still supported (pass --holdout to use
# the old file), since `expected`/`output` were never read by the scoring
# logic below anyway (diff-parse/git-apply-check/envelope-format all operate
# on the model's own completion alone, no ground-truth comparison needed).
# Stratified sample by task_type when present and the requested probe count
# is smaller than the full set; the new schema carries no task_type, so rows
# fall into one bucket and sampling is a deterministic first-N instead —
# still reproducible, just not stratified.
#
# Prompt format: matches run-sft-training.py's format_alpaca_prompt() exactly
# ("### Instruction:\n{instruction}\n\n### Response:\n") — the model was
# trained on this template; anything else is an unfair probe. The new
# schema's `prompt` field is already in this exact format; the old schema's
# `instruction` field gets wrapped in the template at probe-generation time.
#
# Usage:
#   score-gate.sh --adapter-path <path> [--holdout <jsonl>] [--probes N]
#                 [--pass-rate 0.8] [--max-tokens 768] [--dry-run]
#
# --max-tokens default is 768, NOT deploy-gate.sh's 128 — a real diff completion
# (envelope + reasoning + fenced diff) needs far more headroom than a short
# explanatory probe. curl's timeout scales with it (PROBE_CURL_MAX_TIME, default
# 240s) since generation competes with the production Tier A queue on this VM.
#
# Exit codes:
#   0   recommend_promotion=true
#   1   recommend_promotion=false
#   3   Error — same causes as deploy-gate.sh (missing adapter, conversion
#       failure, unreachable scratch server, holdout file not found/empty)

set -uo pipefail

# Diff-generation completions need far more headroom than deploy-gate.sh's short-answer
# probes (its 128-token default truncates every real diff mid-way, before the closing
# ```-fence — confirmed live: every envelope-format PASS this produced was a completion
# right at the ~128-token/~410-char mark, cut off before diff_parse could ever succeed).
# Set BEFORE sourcing _gate-common.sh so its `${VAR:-default}` pattern picks these up
# instead of deploy-gate.sh's short-probe defaults. curl's --max-time scales with it —
# on this shared VM (production Tier A queue competing for CPU), 768 tokens can take
# several minutes under load, not the ~40s a 128-token probe took.
PROBE_MAX_TOKENS="${PROBE_MAX_TOKENS:-768}"
# 480s, not 240s: confirmed live (2026-07-02) that 240s was still not generous enough on
# this shared VM — of 6 live probes at 768 tokens, only the first (which happened to land
# before this VM's production Tier A queue caught up) completed inside 240s; the remaining
# 5 all returned empty at exactly the 240s boundary, consistent with a timeout, not a
# scoring bug (the one completion that DID come back was scored correctly — see
# BRIEF-flow-quality-audit.md's score-gate write-up for the live-run evidence trail).
PROBE_CURL_MAX_TIME="${PROBE_CURL_MAX_TIME:-480}"

_GATE_TAG="score-gate"
source "$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)/_gate-common.sh"

# ── Defaults specific to this script ──────────────────────────────────────────

ADAPTER_PATH=""
BASE_MODEL="${SLM_GATE_BASE_MODEL:-}"
# Workspace-shared path (not archive-local) — matches where eval-prepare.py
# actually writes it, confirmed via `find` before wiring this default.
HOLDOUT="${SLM_GATE_HOLDOUT:-${FOUNDRY_ROOT}/data/training-corpus/eval/holdout-v1.jsonl}"
PROBES=20
PASS_RATE_THRESHOLD="0.8"
DRY_RUN=0
RESULT_FILE="${RESULT_FILE:-${FOUNDRY_ROOT}/data/adapters/score-gate-result.json}"
# Archive-scoped — NOT the same directory as RESULT_FILE above, which is
# workspace-shared, not archive-local.
ARCHIVE_ROOT="${ARCHIVE_ROOT:-${FOUNDRY_ROOT}/clones/project-totebox}"
REGISTRY="${REGISTRY:-${ARCHIVE_ROOT}/data/adapters/registry.yaml}"

# ── Argument parse ────────────────────────────────────────────────────────────

while [[ $# -gt 0 ]]; do
    case "$1" in
        --adapter-path=*)  ADAPTER_PATH="${1#--adapter-path=}" ;;
        --adapter-path)    ADAPTER_PATH="$2"; shift ;;
        --base-model=*)    BASE_MODEL="${1#--base-model=}" ;;
        --base-model)      BASE_MODEL="$2"; shift ;;
        --holdout=*)       HOLDOUT="${1#--holdout=}" ;;
        --holdout)         HOLDOUT="$2"; shift ;;
        --probes=*)        PROBES="${1#--probes=}" ;;
        --probes)          PROBES="$2"; shift ;;
        --pass-rate=*)     PASS_RATE_THRESHOLD="${1#--pass-rate=}" ;;
        --pass-rate)       PASS_RATE_THRESHOLD="$2"; shift ;;
        --max-tokens=*)    PROBE_MAX_TOKENS="${1#--max-tokens=}" ;;
        --max-tokens)      PROBE_MAX_TOKENS="$2"; shift ;;
        --dry-run)         DRY_RUN=1 ;;
        --help|-h)
            sed -n '2,55p' "$0"
            exit 0
            ;;
        *)
            echo "ERROR: unknown argument: $1" >&2; exit 3 ;;
    esac
    shift
done

if [[ -z "${ADAPTER_PATH}" ]]; then
    echo "ERROR: --adapter-path is required" >&2
    echo "Usage: $0 --adapter-path <path> [--holdout <jsonl>] [--probes N]" >&2
    exit 3
fi
if [[ ! -f "${HOLDOUT}" ]]; then
    echo "ERROR: holdout file not found: ${HOLDOUT}" >&2
    exit 3
fi

log "score-gate.sh starting"
log "  adapter_path:  ${ADAPTER_PATH}"
log "  holdout:       ${HOLDOUT}"
log "  probes:        ${PROBES}"
log "  pass_rate:     ${PASS_RATE_THRESHOLD}"
log "  endpoint:      ${ENDPOINT} (scratch — never production :8080)"
log "  result_file:   ${RESULT_FILE}"

gate_common_init

# ── Select a stratified sample of holdout rows ────────────────────────────────
# Stratify by task_type (deterministic — seeded round-robin, not random) so a
# small --probes count still covers every task_type present, same principle
# as run-sft-training.py's eval-split stratification.

_SAMPLE_FILE="$(mktemp /tmp/score-gate-sample-XXXXXX.json)"
python3 - "${HOLDOUT}" "${PROBES}" > "${_SAMPLE_FILE}" <<'PYEOF'
import json, sys
from collections import defaultdict

holdout_path, n = sys.argv[1], int(sys.argv[2])
by_type = defaultdict(list)
with open(holdout_path) as f:
    for line in f:
        line = line.strip()
        if not line:
            continue
        row = json.loads(line)
        by_type[row.get("task_type", "?")].append(row)

types = sorted(by_type.keys())
for t in types:
    by_type[t].sort(key=lambda r: r.get("brief_id", ""))  # deterministic order

selected = []
i = 0
while len(selected) < n and any(by_type[t] for t in types):
    t = types[i % len(types)]
    if by_type[t]:
        selected.append(by_type[t].pop(0))
    i += 1
    if i > n * len(types) + len(types):
        break  # safety valve, should not trigger

print(json.dumps(selected[:n]))
PYEOF

_SAMPLE_COUNT="$(python3 -c "import json; print(len(json.load(open('${_SAMPLE_FILE}'))))")"
if [[ "${_SAMPLE_COUNT}" -eq 0 ]]; then
    log "ERROR: holdout file ${HOLDOUT} produced zero usable rows"
    rm -f "${_SAMPLE_FILE}"
    exit 3
fi
log "  selected ${_SAMPLE_COUNT} stratified probes from ${HOLDOUT}"

# ── Generate + score each probe ───────────────────────────────────────────────

if [[ "${DRY_RUN}" -eq 0 ]]; then
    set_adapter_scale "1.0"
fi

log ""
log "=== Generating + scoring ${_SAMPLE_COUNT} probes (adapter scale 1.0) ==="

_ROW_RESULTS_FILE="$(mktemp /tmp/score-gate-rows-XXXXXX.jsonl)"

_i=0
while [[ "${_i}" -lt "${_SAMPLE_COUNT}" ]]; do
    # New schema (prompt/expected) carries an already-formatted `prompt`; old
    # schema (instruction/input/output/task_type/brief_id) needs the template
    # applied here. Read all four possible fields in one call; bash below
    # picks whichever the row actually has.
    _row_fields="$(python3 -c "
import json
rows = json.load(open('${_SAMPLE_FILE}'))
row = rows[${_i}]
print(json.dumps({
    'prompt': row.get('prompt', ''),
    'instruction': row.get('instruction', ''),
    'brief_id': row.get('brief_id', '?'),
    'task_type': row.get('task_type', '?'),
}))
")"
    _prompt_field="$(python3 -c "import json,sys; print(json.loads(sys.argv[1])['prompt'])" "${_row_fields}")"
    _brief_id="$(python3 -c "import json,sys; print(json.loads(sys.argv[1])['brief_id'])" "${_row_fields}")"
    _task_type="$(python3 -c "import json,sys; print(json.loads(sys.argv[1])['task_type'])" "${_row_fields}")"

    if [[ -n "${_prompt_field}" ]]; then
        _prompt="${_prompt_field}"
    else
        _instruction_text="$(python3 -c "import json,sys; print(json.loads(sys.argv[1])['instruction'])" "${_row_fields}")"
        _prompt="### Instruction:
${_instruction_text}

### Response:
"
    fi

    log "  probe $((_i+1))/${_SAMPLE_COUNT} [${_task_type}] ${_brief_id}"

    if [[ "${DRY_RUN}" -eq 1 ]]; then
        _completion="---
self_confidence: 0.9
escalate: false
---

## Reasoning
Dry-run placeholder.

## Diff
\`\`\`diff
diff --git a/dry-run.txt b/dry-run.txt
index 0000000..1111111 100644
--- a/dry-run.txt
+++ b/dry-run.txt
@@ -1 +1 @@
-old
+new
\`\`\`
"
    else
        _completion="$(probe_completions "${_prompt}")"
    fi

    # Score this one completion (diff-parse, git-apply-check, envelope-format).
    # Passed via a temp file, not string interpolation — completions can contain
    # arbitrary content (quotes, backticks, unicode) that would break a heredoc
    # substitution or shell-quoting round-trip.
    _completion_file="$(mktemp /tmp/score-gate-completion-XXXXXX.txt)"
    printf '%s' "${_completion}" > "${_completion_file}"

    python3 - "${_brief_id}" "${_task_type}" "${_completion_file}" >> "${_ROW_RESULTS_FILE}" <<'PYEOF'
import json, re, subprocess, tempfile, os, sys

brief_id, task_type, completion_file = sys.argv[1], sys.argv[2], sys.argv[3]
with open(completion_file) as f:
    completion = f.read()

# 1. diff-parse — same regex as apprenticeship.rs's extract_diff_block()
diff_re = re.compile(r"```diff\s*\n(.*?)\n```", re.DOTALL)
m = diff_re.search(completion)
diff_text = m.group(1) if m else None
diff_parse_ok = diff_text is not None and len(diff_text.strip()) > 0

# 2. git-apply-check — same pattern as refilter-dpo.py's _git_apply_check()
git_apply_ok = False
if diff_parse_ok:
    patch_path = None
    try:
        with tempfile.NamedTemporaryFile(mode="w", suffix=".patch", delete=False) as f:
            f.write(diff_text)
            patch_path = f.name
        result = subprocess.run(
            ["git", "apply", "--check", "--stat", patch_path],
            capture_output=True, timeout=5,
        )
        git_apply_ok = result.returncode == 0
    except Exception:
        git_apply_ok = False
    finally:
        if patch_path:
            try:
                os.unlink(patch_path)
            except Exception:
                pass

# 3. envelope-format — YAML frontmatter (self_confidence float, escalate bool)
#    then "## Reasoning" then "## Diff", in order (apprenticeship.rs's
#    render_canonical_response / export-sft.py's build_completion).
envelope_ok = False
envelope_reason = "no frontmatter found"
fm_re = re.compile(
    r"^---\s*\nself_confidence:\s*([0-9.]+)\s*\nescalate:\s*(true|false)\s*\n---",
    re.MULTILINE,
)
fm = fm_re.search(completion)
if fm:
    try:
        conf = float(fm.group(1))
        conf_ok = 0.0 <= conf <= 1.0
    except ValueError:
        conf_ok = False
    reasoning_idx = completion.find("## Reasoning")
    diff_idx = completion.find("## Diff")
    order_ok = reasoning_idx != -1 and diff_idx != -1 and reasoning_idx < diff_idx
    if conf_ok and order_ok:
        envelope_ok = True
        envelope_reason = "ok"
    elif not conf_ok:
        envelope_reason = f"self_confidence out of [0,1]: {fm.group(1)}"
    else:
        envelope_reason = "## Reasoning / ## Diff missing or out of order"
else:
    envelope_reason = "frontmatter regex did not match (self_confidence/escalate/--- block)"

row = {
    "brief_id": brief_id,
    "task_type": task_type,
    "diff_parse_ok": diff_parse_ok,
    "git_apply_ok": git_apply_ok,
    "envelope_ok": envelope_ok,
    "envelope_reason": envelope_reason,
    "completion_len": len(completion),
}
print(json.dumps(row))
PYEOF
    rm -f "${_completion_file}"

    _i=$((_i + 1))
done

rm -f "${_SAMPLE_FILE}"

# ── Aggregate ──────────────────────────────────────────────────────────────────

log ""
log "=== Aggregating results ==="

python3 - "${_ROW_RESULTS_FILE}" "${PASS_RATE_THRESHOLD}" "${_SAMPLE_COUNT}" \
         "${ADAPTER_PATH}" "${RESULT_FILE}" "${ENDPOINT}" "${REGISTRY}" "${_CANONICAL_BASE}" \
         "${DRY_RUN}" <<'PYEOF'
import json, sys, os
from datetime import datetime, timezone

(rows_file, pass_rate_threshold, probes_run, adapter_path, result_file, endpoint,
 registry_path, canonical_base, dry_run) = sys.argv[1:10]
pass_rate_threshold = float(pass_rate_threshold)
probes_run = int(probes_run)
dry_run = dry_run == "1"

rows = []
with open(rows_file) as f:
    for line in f:
        line = line.strip()
        if line:
            rows.append(json.loads(line))

n = len(rows)
diff_parse_pass = sum(1 for r in rows if r["diff_parse_ok"])
git_apply_pass = sum(1 for r in rows if r["git_apply_ok"])
envelope_pass = sum(1 for r in rows if r["envelope_ok"])

diff_parse_rate = diff_parse_pass / n if n else 0.0
git_apply_rate = git_apply_pass / n if n else 0.0
envelope_rate = envelope_pass / n if n else 0.0

recommend_promotion = (
    n > 0
    and diff_parse_rate >= pass_rate_threshold
    and git_apply_rate >= pass_rate_threshold
)

result = {
    "recommend_promotion": recommend_promotion,
    "probes_requested": probes_run,
    "probes_scored": n,
    "pass_rate_threshold": pass_rate_threshold,
    "components": {
        "diff_parse": {"pass": diff_parse_pass, "total": n, "rate": round(diff_parse_rate, 3)},
        "git_apply_check": {"pass": git_apply_pass, "total": n, "rate": round(git_apply_rate, 3)},
        "envelope_format": {"pass": envelope_pass, "total": n, "rate": round(envelope_rate, 3),
                             "note": "reported, does not gate recommend_promotion (secondary signal)"},
        "entity_f1": {"available": False,
                      "reason": "service-gliner/eval/ (gold.jsonl + score.py) not yet built — "
                                "P1 roadmap item from the first Fable audit, not this round's scope"},
    },
    "rows": rows,
    "timestamp": datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
    "protocol": "scratch-scale-toggle + holdout-scoring",
    "adapter_path": adapter_path,
    "endpoint": endpoint,
}

with open(result_file, "w") as f:
    json.dump(result, f, indent=2)

# Record this gate attempt in the archive's adapter registry — a flat-list
# schema (name/adapter_dir/base_model/version/trained_on/corpus_pairs/
# eval_pass_at5/promoted/notes), not the incompatible map-keyed adapter-hub
# crate schema; that reconciliation is a separate, already-tracked P1 item,
# not done here. Written regardless of pass/fail — a FAIL is still a real,
# worth-recording data point. Skipped entirely in --dry-run: no real
# inference happened, so there is nothing genuine to record (dry-run's
# placeholder completion would otherwise pollute the registry with fake
# scores, as it did before this guard).
if dry_run:
    print("(dry-run — registry not updated)", file=sys.stderr)
else:
    import yaml

    os.makedirs(os.path.dirname(registry_path), exist_ok=True)
    if os.path.exists(registry_path):
        with open(registry_path) as rf:
            reg = yaml.safe_load(rf) or {}
    else:
        reg = {}

    adapters = reg.get("adapters") or []
    adapter_name = os.path.basename(adapter_path.rstrip("/"))

    # Schema reconciliation (2026-07-06 completion plan, Phase 3): this script
    # previously wrote a narrower schema (name/adapter_dir/base_model/
    # eval_pass_at5/promoted/notes) than the now-retired eval-adapter.sh's
    # (which also had version/trained_on/corpus_pairs) — no adapter had ever
    # been registered with the richer schema. version is a registry-wide
    # max+1 (not per-name), matching that retired script's own scheme
    # exactly. trained_on/corpus_pairs use the same crude proxies it did
    # (today's date; a file count in the adapter directory) — deliberately
    # not a precision improvement, just schema parity.
    version = max((a.get("version", 0) for a in adapters), default=0) + 1
    trained_on = datetime.now(timezone.utc).strftime("%Y-%m-%d")
    try:
        corpus_pairs = len(os.listdir(adapter_path))
    except OSError:
        corpus_pairs = 0

    registry_entry = {
        "name": adapter_name,
        "version": version,
        "adapter_dir": adapter_path,
        "base_model": canonical_base,
        "trained_on": trained_on,
        "corpus_pairs": corpus_pairs,
        "eval_pass_at5": round(diff_parse_rate, 3),
        "promoted": recommend_promotion,
        "notes": (
            f"registered by score-gate.sh — diff_parse={round(diff_parse_rate, 3)} "
            f"git_apply={round(git_apply_rate, 3)} envelope={round(envelope_rate, 3)}"
        ),
    }
    adapters.append(registry_entry)
    reg["adapters"] = adapters

    with open(registry_path, "w") as rf:
        yaml.safe_dump(reg, rf, default_flow_style=False, sort_keys=False)

    print(f"Registered {adapter_name} in {registry_path} (promoted={recommend_promotion})", file=sys.stderr)

print(json.dumps({k: v for k, v in result.items() if k != "rows"}, indent=2))
print(f"\n({n} per-row results also written to {result_file})", file=sys.stderr)

sys.exit(0 if recommend_promotion else 1)
PYEOF
_EXIT_CODE=$?

rm -f "${_ROW_RESULTS_FILE}"

log ""
log "Result written to: ${RESULT_FILE}"
if [[ "${_EXIT_CODE}" -eq 0 ]]; then
    log "RESULT: recommend_promotion=true"
else
    log "RESULT: recommend_promotion=false"
fi
if [[ "${DRY_RUN}" -eq 1 ]]; then
    log "(dry-run — no inference was performed)"
fi

exit "${_EXIT_CODE}"
