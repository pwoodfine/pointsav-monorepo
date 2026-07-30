#!/usr/bin/env bash
# Yo-Yo stability gate (Phase 0 G4 hardening).
#
# Defers the expensive weights-prep + model load until the VM has survived a
# stability window. A SPOT VM preempted inside the window then costs ~cents of
# idle boot instead of dollars of doomed model load (the ~$50 incident pattern).
#
# This service is the ONLY boot entrypoint for inference: `vllm-weights-prep`
# and `llama-server` are no longer WantedBy=multi-user.target — they start only
# when this gate triggers `llama-server.service` (which Requires weights-prep).
set -uo pipefail

META="http://metadata.google.internal/computeMetadata/v1/instance"
mh() { curl -sf -H 'Metadata-Flavor: Google' "$@"; }

WINDOW="$(mh "${META}/attributes/stability-window-seconds" || true)"
[[ "${WINDOW}" =~ ^[0-9]+$ ]] || WINDOW=120   # default 120 s

echo "yoyo-stability-gate: holding ${WINDOW}s before model load (cheap-failure window)..."
sleep "${WINDOW}"

# GCE flips this metadata key to TRUE when the instance has been preempted.
PREEMPTED="$(mh "${META}/preempted" || echo UNKNOWN)"
if [[ "${PREEMPTED}" == "TRUE" ]]; then
    echo "yoyo-stability-gate: instance flagged PREEMPTED — NOT loading the model. Exiting clean."
    exit 0
fi

echo "yoyo-stability-gate: VM stable after ${WINDOW}s (preempted=${PREEMPTED}) — starting inference."
# Requires=/After= on llama-server.service pulls in vllm-weights-prep first.
systemctl start llama-server.service
