#!/usr/bin/env bash
# stage6-gate.sh — dead-link + frontmatter gate for app-mediakit-knowledge promotes.
#
# Run this before every Stage 6 promote of the sub-clone. It invokes
# `cargo xtask check-content` (xtask/src/main.rs) against the three live
# content mounts and exits 1 if any dead wikilinks or missing required
# frontmatter fields are found. Exits 0 = clean; 1 = gate failure.
#
# Usage (from anywhere inside the sub-clone):
#   app-mediakit-knowledge/scripts/stage6-gate.sh
#
# The script locates the monorepo root automatically via git.
set -euo pipefail

MONOREPO_ROOT="$(git -C "$(dirname "$0")" rev-parse --show-toplevel)"
EDITORIAL="/srv/foundry/clones/project-editorial"

for dir in \
    "$EDITORIAL/media-knowledge-documentation" \
    "$EDITORIAL/media-knowledge-projects" \
    "$EDITORIAL/media-knowledge-corporate"; do
    if [[ ! -d "$dir" ]]; then
        echo "ERROR: content directory not found: $dir" >&2
        echo "  Ensure project-editorial is cloned at $EDITORIAL" >&2
        exit 2
    fi
done

echo "==> stage6-gate: running xtask check-content across 3 mounts"
cd "$MONOREPO_ROOT"
cargo xtask check-content \
    "$EDITORIAL/media-knowledge-documentation" \
    "$EDITORIAL/media-knowledge-projects" \
    "$EDITORIAL/media-knowledge-corporate"
