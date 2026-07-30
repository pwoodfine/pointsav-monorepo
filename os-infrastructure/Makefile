# ==============================================================================
# PointSav os-infrastructure Build System
# Target: Node 1 (Mesh Edge Anchor)
# ==============================================================================

BOARD := node1_mesh
BUILD_DIR := build
MONOREPO := $(shell pwd)/..

# 1. Component Paths
MUSCLE := $(MONOREPO)/system-security
WATCHDOG := $(MONOREPO)/system-security
NETWORK := $(MONOREPO)/system-network-interface

.PHONY: all clean compile-all

all: $(BUILD_DIR)/os-infrastructure.iso

compile-all:
	@echo "🛠️  Compiling Mesh Protection Domains..."
	cd $(MUSCLE) && cargo build --release
	cd $(NETWORK) && cargo build --release

$(BUILD_DIR)/os-infrastructure.iso: compile-all
	@echo "💿 Synthesizing Delivery Vehicle: os-infrastructure.iso"
	# Step 1: Microkit tool patches the ELFs with system metadata
	microkit $(BOARD).system --search-path $(BUILD_DIR) -o $(BUILD_DIR)/final_mesh.elf
	
	# Step 2: Forge the Multiboot2 ISO
	./forge_iso.sh
	@echo "🟢 SUCCESS: Artifact staged at $(BUILD_DIR)/os-infrastructure.iso"

clean:
	rm -rf $(BUILD_DIR)
