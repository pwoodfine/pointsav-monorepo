#!/usr/bin/env bash
# daily-slm-report.sh — Daily status snapshot of the SLM training + DataGraph substrate.
#
# Produces a concise Markdown report covering:
#   - service-slm (Doorman): tiers, circuit state, apprenticeship queue + corpus (TRAINING)
#   - service-content: graph entity count, watcher state (DATAGRAPH)
#   - Yo-Yo Tier B VM state + cost guardrails
#   - deployed binary integrity vs binary-ledger
#
# All checks are READ-ONLY (localhost HTTP + filesystem + gcloud describe + systemctl show).
# Each section is failure-tolerant — a down service yields "(unreachable)", never aborts.
#
# Output:
#   $REPORT_DIR/slm-daily-<YYYY-MM-DD>.md   (dated)
#   $REPORT_DIR/slm-daily-latest.md          (always-current copy)
#
# Usage:   ./daily-slm-report.sh            (prints path; writes the report)
#          REPORT_DIR=/tmp ./daily-slm-report.sh
# Intended to run daily via foundry-slm-daily-report.timer (see docs/deploy/).

set -uo pipefail

REPORT_DIR="${REPORT_DIR:-/srv/foundry/data/reports}"
DOORMAN="${SLM_DOORMAN_ENDPOINT:-http://127.0.0.1:9080}"
CONTENT="${SERVICE_CONTENT_ENDPOINT:-http://127.0.0.1:9081}"
APPR="${APPRENTICESHIP_DIR:-/srv/foundry/data/apprenticeship}"
CORPUS="${TRAINING_CORPUS_DIR:-/srv/foundry/data/training-corpus}"
DOORMAN_ENV="${DOORMAN_ENV_FILE:-/etc/local-doorman/local-doorman.env}"
LEDGER_DIR="${BINARY_LEDGER_DIR:-/srv/foundry/data/binary-ledger}"
YOYO_INSTANCE="${SLM_YOYO_GCP_INSTANCE:-yoyo-tier-b-1}"
YOYO_ZONE="${SLM_YOYO_GCP_ZONE:-europe-west4-a}"
YOYO_PROJECT="${SLM_YOYO_GCP_PROJECT:-woodfine-node-gcp-free}"

NOW="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
DATE="$(date -u +%Y-%m-%d)"
mkdir -p "$REPORT_DIR" 2>/dev/null || true
OUT="$REPORT_DIR/slm-daily-${DATE}.md"

# ── helpers ───────────────────────────────────────────────────────────────────
jget() { python3 -c "import json,sys; d=json.load(sys.stdin); print($1)" 2>/dev/null || echo "?"; }
cnt()  { ls "$1" 2>/dev/null | wc -l | tr -d ' '; }
svc()  { local s; s="$(systemctl is-active "$1" 2>/dev/null)"; echo "${s:-unknown}"; }

# ── gather: Doorman /readyz ─────────────────────────────────────────────────────
READYZ="$(curl -s --max-time 6 "$DOORMAN/readyz" 2>/dev/null)"
if [ -n "$READYZ" ]; then
  TIER_A="$(echo "$READYZ" | jget "d.get('tier_a')")"
  TIER_A_REASON="$(echo "$READYZ" | jget "d.get('tier_a_reason')")"
  NODE_CLASS="$(echo "$READYZ" | jget "d.get('node_class')")"
  TB_DEFAULT="$(echo "$READYZ" | jget "'circuit='+str(d['tier_b']['default']['circuit'])+' health_up='+str(d['tier_b']['default']['health_up'])+' zone='+str(d['tier_b']['default'].get('zone'))+' reason='+str(d['tier_b']['default'].get('reason'))")"
else
  TIER_A="(Doorman unreachable)"; TIER_A_REASON="?"; NODE_CLASS="?"; TB_DEFAULT="(unreachable)"
fi

# ── gather: service-content /healthz ─────────────────────────────────────────────
CHEALTH="$(curl -s --max-time 6 "$CONTENT/healthz" 2>/dev/null)"
if [ -n "$CHEALTH" ]; then
  ENTITY_COUNT="$(echo "$CHEALTH" | jget "d.get('entity_count')")"
  CSTATUS="$(echo "$CHEALTH" | jget "d.get('status')")"
else
  ENTITY_COUNT="(service-content unreachable)"; CSTATUS="down"
fi

# ── gather: apprenticeship queue ─────────────────────────────────────────────────
Q_PEND="$(cnt "$APPR/queue")"; Q_DONE="$(cnt "$APPR/queue-done")"
Q_POISON="$(cnt "$APPR/queue-poison")"; Q_FLIGHT="$(cnt "$APPR/queue-in-flight")"

# ── gather: training corpus tuple counts ─────────────────────────────────────────
CORPUS_COUNTS="$(python3 - "$CORPUS" <<'PY' 2>/dev/null || echo "  (corpus unreadable)"
import os, json, sys, collections
root = sys.argv[1]
c = collections.Counter()
real = 0
for dp, _, files in os.walk(root):
    for f in files:
        if not f.endswith('.jsonl'): continue
        try:
            d = json.load(open(os.path.join(dp, f)))
            c[d.get('tuple_type','unknown')] += 1
            ad = d.get('actual_diff','') or (d.get('attempt') or {}).get('diff','')
            if ad and len(ad) > 10: real += 1
        except Exception:
            pass
for t, n in c.most_common():
    print(f"  - {t}: {n}")
print(f"  - (tuples with a non-empty diff: {real})")
PY
)"

# ── gather: config flags ─────────────────────────────────────────────────────────
DRAIN_PAUSED="$(grep -E '^SLM_DRAIN_PAUSED=' "$DOORMAN_ENV" 2>/dev/null | cut -d= -f2 || echo '?')"
TIER_A_FIRST="$(grep -E '^SLM_TIER_A_FIRST=' "$DOORMAN_ENV" 2>/dev/null | cut -d= -f2 || echo '?')"
CONTENT_FALLBACK="$(systemctl show local-content.service -p Environment 2>/dev/null | tr ' ' '\n' | grep TIER_A_FALLBACK_ENABLED | cut -d= -f2 || echo '?')"

# ── gather: Yo-Yo VM ──────────────────────────────────────────────────────────────
YOYO_STATUS="$(timeout 25 gcloud compute instances describe "$YOYO_INSTANCE" --zone="$YOYO_ZONE" --project="$YOYO_PROJECT" --format='value(status)' 2>/dev/null || echo '(gcloud unavailable)')"

# ── gather: binary integrity vs ledger ────────────────────────────────────────────
bin_check() {
  local name="$1" path="$2"
  local cur led
  cur="$(sha256sum "$path" 2>/dev/null | cut -d' ' -f1)"
  led="$(tail -1 "$LEDGER_DIR/$name.jsonl" 2>/dev/null | python3 -c "import json,sys;print(json.load(sys.stdin).get('sha256',''))" 2>/dev/null)"
  if [ -z "$cur" ]; then echo "  - $name: (not installed)"
  elif [ "$cur" = "$led" ]; then echo "  - $name: ✓ matches ledger (${cur:0:12})"
  else echo "  - $name: ⚠ DRIFT installed=${cur:0:12} ledger=${led:0:12}"; fi
}

# ── write report ──────────────────────────────────────────────────────────────────
{
cat <<MD
# SLM Substrate — Daily Report

**Generated:** $NOW · **Host:** $(hostname -s 2>/dev/null)

## service-slm — Training / Inference gateway (Doorman)

| Field | Value |
|---|---|
| local-doorman.service | $(svc local-doorman.service) |
| local-slm.service (Tier A) | $(svc local-slm.service) |
| node_class | $NODE_CLASS |
| Tier A | $TIER_A (reason: $TIER_A_REASON) |
| Tier B (default) | $TB_DEFAULT |
| Yo-Yo VM ($YOYO_INSTANCE) | $YOYO_STATUS |
| drain paused | $DRAIN_PAUSED · tier-A-first: $TIER_A_FIRST |

### Apprenticeship queue (training capture)
- pending: **$Q_PEND** · in-flight: $Q_FLIGHT · done: **$Q_DONE** · poison: **$Q_POISON**

### Training corpus tuples
$CORPUS_COUNTS

## service-content — DataGraph

| Field | Value |
|---|---|
| local-content.service | $(svc local-content.service) |
| graph status | $CSTATUS |
| entity_count | **$ENTITY_COUNT** |
| Tier A extraction fallback | $CONTENT_FALLBACK |

## Binary integrity (deployed vs ledger)
$(bin_check slm-doorman-server /usr/local/bin/slm-doorman-server)
$(bin_check service-content /usr/local/bin/service-content)

---
*Auto-generated by service-slm/scripts/daily-slm-report.sh. Read-only snapshot.*
MD
} > "$OUT" 2>&1

cp -f "$OUT" "$REPORT_DIR/slm-daily-latest.md" 2>/dev/null || true
chmod 0644 "$OUT" "$REPORT_DIR/slm-daily-latest.md" 2>/dev/null || true
echo "report written: $OUT"
echo "latest: $REPORT_DIR/slm-daily-latest.md"
