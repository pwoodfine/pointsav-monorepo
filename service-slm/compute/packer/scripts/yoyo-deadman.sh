#!/usr/bin/env bash
# Yo-Yo dead-man's-switch (Phase 0 G3 + G17 hardening).
#
# Two modes:
#   arm  (default — run by yoyo-deadman.service at boot): schedules a transient
#        systemd timer that fires `--fire` at max-lifetime.
#   --fire: tags the stop `deadman` in instance metadata (G17 — so the Doorman
#        idle monitor treats it as sticky and does NOT auto-restart the VM),
#        then powers the VM off.
#
# This is the GUARANTEED teardown path: unlike the Doorman idle monitor and the
# start-yoyo.sh --runtime watchdog (both of which die with their host process),
# it runs on the VM itself and survives Doorman crashes / network partitions.
# The normal stop paths fire earlier; whichever fires first wins. On the next
# boot this service re-arms.
set -uo pipefail

META="http://metadata.google.internal/computeMetadata/v1/instance"
mh() { curl -sf -H 'Metadata-Flavor: Google' "$@"; }

# ── Fire mode — tag the stop, then power off ─────────────────────────────────
if [[ "${1:-}" == "--fire" ]]; then
    # G17: tag last-stop-reason=deadman so the idle monitor will not restart it.
    NAME="$(mh "${META}/name" || true)"
    ZONE="$(mh "${META}/zone" | awk -F/ '{print $NF}')"
    PROJECT="$(mh http://metadata.google.internal/computeMetadata/v1/project/project-id || true)"
    if [[ -n "${NAME}" && -n "${ZONE}" && -n "${PROJECT}" ]]; then
        gcloud compute instances add-metadata "${NAME}" \
            --zone="${ZONE}" --project="${PROJECT}" \
            --metadata=last-stop-reason=deadman >/dev/null 2>&1 \
            || echo "yoyo-deadman: WARN could not tag last-stop-reason=deadman"
    fi
    echo "yoyo-deadman: max lifetime reached — powering off."
    systemctl poweroff
    exit 0
fi

# ── Arm mode — schedule the fire at max-lifetime ─────────────────────────────
# max-lifetime-seconds is set in instance metadata by start-yoyo.sh / OpenTofu.
MAX="$(mh "${META}/attributes/max-lifetime-seconds" || true)"
[[ "${MAX}" =~ ^[0-9]+$ ]] || MAX=14400   # default 4 h

# A transient systemd timer owns the schedule — it survives this script exiting
# and fires `--fire` (which tags metadata then powers off) at max-lifetime.
if systemd-run --on-active="${MAX}s" --unit=yoyo-deadman-fire --collect \
        /usr/local/bin/yoyo-deadman.sh --fire >/dev/null 2>&1; then
    echo "yoyo-deadman: armed — VM self-stops in ${MAX}s via systemd timer yoyo-deadman-fire."
else
    # Fallback: classic scheduled halt if systemd-run is unavailable. This path
    # cannot tag last-stop-reason, but the VM still self-stops.
    MIN=$(( (MAX + 59) / 60 ))
    [[ "${MIN}" -lt 1 ]] && MIN=1
    shutdown -h "+${MIN}" "yoyo-deadman: max lifetime ${MAX}s reached" || true
    echo "yoyo-deadman: armed via shutdown +${MIN}m (systemd-run unavailable)."
fi
