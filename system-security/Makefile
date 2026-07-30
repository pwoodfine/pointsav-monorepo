SDK_PATH ?= $(error Set SDK_PATH to your Microkit SDK directory, e.g. SDK_PATH=~/microkit-sdk-2.1.0 make)
BOARD    := x86_64_generic
CONFIG   := debug

CC       := gcc
LD       := x86_64-linux-gnu-ld
MICROKIT := $(SDK_PATH)/bin/microkit
LIB_PATH := $(SDK_PATH)/board/$(BOARD)/$(CONFIG)/lib
INC_PATH := $(SDK_PATH)/board/$(BOARD)/$(CONFIG)/include
BUILD_DIR := build
TARGET_DIR := ../target/x86_64-unknown-none/debug

all: $(BUILD_DIR)/final_image.elf

# Compile the C wrapper
$(BUILD_DIR)/notify.o: src/notify.c
	@mkdir -p $(BUILD_DIR)
	$(CC) -c -ffreestanding -I$(INC_PATH) $< -o $@

$(BUILD_DIR)/system_security.elf: src/lib.rs $(BUILD_DIR)/notify.o
	@mkdir -p $(BUILD_DIR)
	cargo build --target x86_64-unknown-none
	$(LD) -T $(LIB_PATH)/microkit.ld \
		--whole-archive $(TARGET_DIR)/libsystem_security.a --no-whole-archive \
		$(BUILD_DIR)/notify.o \
		$(LIB_PATH)/libmicrokit.a \
		-o $@

$(BUILD_DIR)/watchdog.elf: src/watchdog.rs $(BUILD_DIR)/notify.o
	@mkdir -p $(BUILD_DIR)
	rustc --target x86_64-unknown-none -C panic=abort -C opt-level=z --emit=obj $< -o $(BUILD_DIR)/watchdog.o
	# Using standard libmicrokit initialization
	$(LD) -T $(LIB_PATH)/microkit.ld \
		$(BUILD_DIR)/watchdog.o \
		$(BUILD_DIR)/notify.o \
		$(LIB_PATH)/libmicrokit.a \
		-o $@

$(BUILD_DIR)/final_image.elf: $(BUILD_DIR)/system_security.elf $(BUILD_DIR)/watchdog.elf security.system
	$(MICROKIT) security.system --search-path $(BUILD_DIR) --board $(BOARD) --config $(CONFIG) -o $@

clean:
	rm -rf $(BUILD_DIR)
	cargo clean
