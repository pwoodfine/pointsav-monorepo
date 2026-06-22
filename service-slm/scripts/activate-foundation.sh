#!/usr/bin/env bash
# activate-foundation.sh — operator/Command activation of the foundation fixes.
#
# The foundation CODE is committed on cluster/project-totebox (Commits 1-4). This
# script applies the parts the autonomous build could NOT do unattended: sudo service
# restarts, the lbug infra repair, and the GPU training sequence. Idempotent; run the
# sudo/systemd steps from a Command Session per scope-discipline.
#
# See BRIEF-flow-build-plan and BRIEF-flow-quality-audit.
set -uo pipefail

ARCHIVE="${ARCHIVE:-/srv/foundry/clones/project-totebox}"
SLM="${ARCHIVE}/service-slm"
log() { printf '[activate-foundation] %s\n' "$*"; }

# ── 0. INFRA: repair lbug so service-content compiles (BLOCKS Commit-4 verify) ──────
# /srv/foundry/lbug-includes/lbug.hpp is a dangling symlink (prebuilt cache evicted);
# every service-content build fails at the native lbug step. Repair, then verify Commit 4.
log "0. lbug infra: $(readlink -e /srv/foundry/lbug-includes/lbug.hpp >/dev/null 2>&1 && echo OK || echo 'DANGLING — repair needed')"
log "   fix: cargo clean -p lbug && cargo build -p service-content   (rebuilds lbug from source,"
log "        regenerating .cache/lbug-prebuilt; needs cmake + network on a quiet box)"
log "   then verify Commit 4:  cargo test -p service-content   (normalize_* + er:: tests)"

# ── 1. base-registry consistency ───────────────────────────────────────────────────
REG="${SLM}/data/base-registry.yaml"
CANON=$(grep -E '^canonical_base:' "$REG" | awk '{print $2}')
SERVED=$(curl -s --max-time 5 http://127.0.0.1:8080/v1/models 2>/dev/null | grep -o 'Olmo[^"]*' | head -1)
log "1. base-registry canonical=${CANON}  served-gguf=${SERVED:-unknown}  (must be the same OLMo-3 lineage)"

# ── 2. Doorman default module + base label (sudo — Command Session) ─────────────────
cat <<'EOF'
2. Doorman activation (sudo systemctl — Command Session):
   sudo mkdir -p /etc/systemd/system/local-doorman.service.d
   printf '[Service]\nEnvironment=SLM_DEFAULT_MODULE_ID=woodfine\nEnvironment=SLM_LOCAL_MODEL=OLMo-3-7B-Instruct\n' \
     | sudo tee /etc/systemd/system/local-doorman.service.d/zz-foundation.conf
   sudo systemctl daemon-reload && sudo systemctl restart local-doorman
   # verify: curl -s :9080/v1/graph/context?q=Woodfine  (no module_id) now hits the live namespace
EOF

# ── 3. Interactive vs batch runtime split (ends Tier-A starvation) ──────────────────
cat <<'EOF'
3. Split interactive vs batch (ends Tier-A starvation):
   Run a SECOND llama-server on its own slots for the apprenticeship drain (Nice/IOSchedulingClass=idle),
   leaving the interactive :8080 pool for low-latency Doorman traffic. Until then, throttle the drain:
   set SLM_DRAIN_CONCURRENCY=1 (already default) and SLM_DRAIN_PAUSED=true during interactive work.
EOF

# ── 4. Serve a trained adapter (after training; needs GPU) ──────────────────────────
cat <<'EOF'
4. Serve the adapter (after step 5 produces one):
   convert_lora_to_gguf.py <adapter> ; add --lora-scaled <gguf> to local-slm.service ExecStart ;
   sudo systemctl restart local-slm ; deploy-gate: >=20-probe base-vs-adapter delta (null delta = FAIL).
EOF

# ── 5. GPU training sequence (when yoyo-batch L4 capacity returns) ──────────────────
cat <<EOF
5. Train (yoyo-batch L4; SFT-first → on-policy DPO → eval gate → promote):
   cd ${SLM}
   python3 scripts/run-sft-training.py            # base = OLMo-3-Instruct (registry), engineering corpus wired
   python3 scripts/run-dpo-training.py --mode pref --loss-type simpo   # SFT-first guard + truncation pre-check enforced
   bash   scripts/eval-adapter.sh <adapter>       # hard gate before promotion
EOF

log "done — foundation code is committed; the above are the operator/Command activation steps."
