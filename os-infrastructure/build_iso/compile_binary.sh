#!/usr/bin/env bash
# ==============================================================================
# Script: compile_binary.sh (Workspace-Aware Compile)
# Product: os-infrastructure (Node 1 - Muscle)
# Purpose: Compiles and extracts the binary from the Workspace Root.
# Renamed 2026-04-23 from forge_iso.sh to resolve filename collision
# with ../forge_iso.sh (ISO assembly). This script is the compile
# step; the sibling at the project root is the assembly step.
# ==============================================================================

set -euo pipefail

# In a Workspace, the target folder is at the Monorepo Root
WORKSPACE_ROOT="/home/mathew/Foundry/factory-pointsav/pointsav-monorepo"
PRODUCT_DIR="${WORKSPACE_ROOT}/os-infrastructure"
BUILD_DIR="${PRODUCT_DIR}/build_iso"
TARGET="x86_64-unknown-none"

echo "[INFO] Starting Workspace Forge for os-infrastructure..."

# 1. Compile from Workspace Root
cd "${WORKSPACE_ROOT}"
cargo build --release --target "${TARGET}" -p os-infrastructure --config "target.x86_64-unknown-none.rustflags=['-C', 'link-arg=-Tos-infrastructure/linker.ld']"

# 2. Extract Binary from WORKSPACE_ROOT/target
BINARY="${WORKSPACE_ROOT}/target/${TARGET}/release/os-infrastructure"
cp "${BINARY}" "${BUILD_DIR}/os-infrastructure.elf"

echo "[INFO] Binary successfully extracted to: ${BUILD_DIR}/os-infrastructure.elf"
echo "[SUCCESS] Product ready for ISO wrapping."
