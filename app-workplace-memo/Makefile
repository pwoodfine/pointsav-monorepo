# Workplace*Memo — Makefile
# Common development and build tasks.
# Requires: make, npm, cargo, curl, base64
#
# Copyright © 2026 PointSav Digital Systems — EUPL-1.2

.PHONY: help setup deps fonts dev build clean audit check

# Default target
help:
	@echo ""
	@echo "  Workplace✦Memo — Developer Tasks"
	@echo "  ────────────────────────────────────────────"
	@echo "  make setup     Full first-time setup (deps + fonts + npm install)"
	@echo "  make deps      Download Paged.js and font WOFF2 files"
	@echo "  make fonts     Embed fonts into src/js/font-data.js"
	@echo "  make dev       Start development server (tauri dev)"
	@echo "  make build     Production build (tauri build)"
	@echo "  make check     Run Rust checks without building"
	@echo "  make audit     Run cargo audit (dependency CVE check)"
	@echo "  make clean     Remove build artefacts"
	@echo ""

# ─── First-time setup ────────────────────────────────────────────────────────

setup: deps fonts
	npm install
	@echo ""
	@echo "  ✓ Setup complete."
	@echo "  Run: make dev"
	@echo ""

# ─── Download dependencies (fonts + Paged.js) ────────────────────────────────

deps:
	@chmod +x scripts/download-deps.sh
	./scripts/download-deps.sh

# ─── Embed fonts into font-data.js ───────────────────────────────────────────

fonts:
	@chmod +x scripts/embed-fonts.sh
	./scripts/embed-fonts.sh

# ─── Development server ───────────────────────────────────────────────────────

dev: fonts
	npm run tauri dev

# ─── Production build ────────────────────────────────────────────────────────

build: fonts
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
	rm -f src/js/font-data.js
	@echo "  Cleaned build artefacts."
	@echo "  Run 'make fonts' to regenerate font-data.js before next build."
