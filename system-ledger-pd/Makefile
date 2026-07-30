# system-ledger-pd — seL4 Microkit PD build
#
# Requires Microkit SDK v2.1.0 for x86_64_generic board.
# Download from https://github.com/seL4/microkit/releases
#
# Usage:
#   SDK_PATH=~/microkit-sdk-2.1.0 make
#   SDK_PATH=~/microkit-sdk-2.1.0 make clean
#
# CI (no SDK required — catches std-creep in upstream crates):
#   cargo build --no-default-features --features sel4 --target x86_64-unknown-none

SDK_PATH ?=

BOARD     := x86_64_generic
CONFIG    := debug
MICROKIT  = $(if $(SDK_PATH),$(SDK_PATH)/bin/microkit,$(error Set SDK_PATH to your Microkit SDK directory, e.g. SDK_PATH=~/microkit-sdk-2.1.0 make))
LIB_PATH  = $(if $(SDK_PATH),$(SDK_PATH)/board/$(BOARD)/$(CONFIG)/lib,$(error Set SDK_PATH))
INC_PATH  = $(if $(SDK_PATH),$(SDK_PATH)/board/$(BOARD)/$(CONFIG)/include,$(error Set SDK_PATH))
BUILD_DIR := build
# Honour CARGO_TARGET_DIR if set (workspace-wide redirect); fall back to local target/.
_CARGO_TARGET := $(if $(CARGO_TARGET_DIR),$(CARGO_TARGET_DIR),target)
TARGET_DIR    := $(_CARGO_TARGET)/x86_64-unknown-none/debug

CC := gcc
LD := x86_64-linux-gnu-ld

.PHONY: all clean check

all: $(BUILD_DIR)/final_image.elf

# ── Rust staticlib ────────────────────────────────────────────────────────────

$(TARGET_DIR)/libsystem_ledger_pd.a: src/lib.rs Cargo.toml
	cargo build --no-default-features --features sel4 --target x86_64-unknown-none

# ── C shim ────────────────────────────────────────────────────────────────────

$(BUILD_DIR)/shim.o: src/shim.c
	@mkdir -p $(BUILD_DIR)
	$(CC) -c -ffreestanding -nostdlib -I$(INC_PATH) $< -o $@

# ── Link system_ledger PD ─────────────────────────────────────────────────────

$(BUILD_DIR)/system_ledger.elf: $(TARGET_DIR)/libsystem_ledger_pd.a $(BUILD_DIR)/shim.o
	@mkdir -p $(BUILD_DIR)
	$(LD) -T $(LIB_PATH)/microkit.ld \
		--whole-archive $(TARGET_DIR)/libsystem_ledger_pd.a --no-whole-archive \
		$(BUILD_DIR)/shim.o \
		$(LIB_PATH)/libmicrokit.a \
		-o $@

# ── Minimal client PD (test harness — issues one ConsultRequest via PPC) ─────

$(BUILD_DIR)/client_pd.elf: src/client_pd.c $(BUILD_DIR)/shim.o
	@mkdir -p $(BUILD_DIR)
	$(CC) -ffreestanding -nostdlib -no-pie -I$(INC_PATH) \
		src/client_pd.c $(BUILD_DIR)/shim.o \
		$(LIB_PATH)/libmicrokit.a \
		-T $(LIB_PATH)/microkit.ld \
		-o $@

# ── Microkit image assembly ───────────────────────────────────────────────────

$(BUILD_DIR)/final_image.elf: $(BUILD_DIR)/system_ledger.elf $(BUILD_DIR)/client_pd.elf ledger.system
	$(MICROKIT) ledger.system \
		--search-path $(BUILD_DIR) \
		--board $(BOARD) --config $(CONFIG) \
		-o $@

# ── CI target (no SDK) ───────────────────────────────────────────────────────

check:
	cargo build --no-default-features --features sel4 --target x86_64-unknown-none

clean:
	rm -rf $(BUILD_DIR)
	cargo clean
