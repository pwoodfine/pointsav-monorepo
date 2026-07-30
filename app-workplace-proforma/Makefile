# Workplace*Proforma — Makefile
# Common development and build tasks.
# Requires: make, npm, cargo
#
# Copyright © 2026 PointSav Digital Systems — EUPL-1.2

.PHONY: help setup dev build clean audit check

# Default target
help:
	@echo ""
	@echo "  Workplace✦Proforma — Developer Tasks"
	@echo "  ────────────────────────────────────────────"
	@echo "  make setup     First-time setup (npm install)"
	@echo "  make dev       Start development server (tauri dev)"
	@echo "  make build     Production build (tauri build)"
	@echo "  make check     Run Rust checks without building"
	@echo "  make audit     Run cargo audit (dependency CVE check)"
	@echo "  make clean     Remove build artefacts"
	@echo ""

# ─── First-time setup ────────────────────────────────────────────────────────

setup:
	npm install
	@echo ""
	@echo "  ✓ Setup complete."
	@echo "  Run: make dev"
	@echo ""

# ─── Development server ───────────────────────────────────────────────────────

dev:
	npm run tauri dev

# ─── Production build ────────────────────────────────────────────────────────

build:
	npm run tauri build

# ─── Rust checks ─────────────────────────────────────────────────────────────

check:
	cd src-tauri && cargo check

# ─── Security audit ──────────────────────────────────────────────────────────

audit:
	cd src-tauri && cargo audit
	@echo ""
	@echo "  If cargo-audit is not installed:"
	@echo "    cargo install cargo-audit"
	@echo ""

# ─── Clean ───────────────────────────────────────────────────────────────────

clean:
	rm -rf src-tauri/target
	rm -rf node_modules
	@echo "  Cleaned build artefacts."
	@echo "  Run 'make setup' to reinstall dependencies before next build."
