# Simplified vendored ArceOS build entry for StarryX.

ARCH ?= x86_64
PLATFORM ?= $(PLAT_NAME)
SMP ?= 1
MODE ?= release
LOG ?= warn
V ?=
TARGET_DIR ?= $(abspath ../target)
EXTRA_CONFIG ?=
OUT_CONFIG ?= $(abspath ../.axconfig.toml)

A ?=
APP ?= $(if $(A),$(abspath $(A)),$(abspath ..))
FEATURES ?=

BLK ?= n
NET ?= n
GRAPHIC ?= n
BUS ?= pci
MEM ?= 1G
ACCEL ?=
DISK_IMG ?= disk.img
QEMU_LOG ?= n
NET_DUMP ?= n
NET_DEV ?= user
VFIO_PCI ?=
VHOST ?= n

IP ?= 10.0.2.15
GW ?= 10.0.2.2

ifeq ($(wildcard $(APP)),)
  $(error Application path "$(APP)" is not valid)
endif

ifeq ($(wildcard $(APP)/Cargo.toml),)
  $(error This trimmed ArceOS tree only supports Rust apps with Cargo.toml)
endif

include scripts/make/features.mk
include scripts/make/platform.mk

ifeq ($(ARCH), x86_64)
  TARGET := x86_64-unknown-none
else ifeq ($(ARCH), aarch64)
  ifeq ($(findstring fp_simd,$(FEATURES)),)
    TARGET := aarch64-unknown-none-softfloat
  else
    TARGET := aarch64-unknown-none
  endif
else ifeq ($(ARCH), riscv64)
  TARGET := riscv64gc-unknown-none-elf
else ifeq ($(ARCH), loongarch64)
  TARGET := loongarch64-unknown-none
else
  $(error "ARCH" must be one of "x86_64", "riscv64", "aarch64" or "loongarch64")
endif

export AX_ARCH := $(ARCH)
export AX_PLATFORM := $(PLAT_NAME)
export AX_SMP := $(SMP)
export AX_MODE := $(MODE)
export AX_LOG := $(LOG)
export AX_TARGET := $(TARGET)
export AX_IP := $(IP)
export AX_GW := $(GW)
export AX_CONFIG_PATH := $(OUT_CONFIG)

OBJDUMP ?= rust-objdump -d --print-imm-hex --x86-asm-syntax=intel
OBJCOPY ?= rust-objcopy --binary-architecture=$(ARCH)
GDB ?= gdb

OUT_DIR ?= $(APP)
APP_NAME := $(shell basename $(APP))
LD_SCRIPT := $(TARGET_DIR)/$(TARGET)/$(MODE)/linker_$(PLAT_NAME).lds
OUT_ELF := $(OUT_DIR)/$(APP_NAME)_$(PLAT_NAME).elf
OUT_BIN := $(patsubst %.elf,%.bin,$(OUT_ELF))
FINAL_IMG := $(OUT_BIN)

all: build

include scripts/make/utils.mk
include scripts/make/config.mk
include scripts/make/build.mk
include scripts/make/qemu.mk
ifeq ($(PLAT_NAME), riscv64-visionfive2)
  include scripts/make/visionfive2.mk
endif

defconfig: _axconfig-gen
	$(call defconfig)

oldconfig: _axconfig-gen
	$(call oldconfig)

build: $(OUT_DIR) $(FINAL_IMG)

disasm:
	$(OBJDUMP) $(OUT_ELF) | less

run: build justrun

justrun:
	$(call run_qemu)

debug: build
	$(call run_qemu_debug) &
	sleep 1
	$(GDB) $(OUT_ELF) \
	  -ex 'target remote localhost:1234' \
	  -ex 'b rust_entry' \
	  -ex 'b rust_main' \
	  -ex 'continue' \
	  -ex 'disp /16i $$pc'

clippy: oldconfig
ifeq ($(origin ARCH), command line)
	$(call cargo_clippy,--target $(TARGET))
else
	$(call cargo_clippy)
endif

fmt:
	cargo fmt --all

disk_img:
ifneq ($(wildcard $(DISK_IMG)),)
	@printf "$(YELLOW_C)warning$(END_C): disk image \"$(DISK_IMG)\" already exists!\n"
else
	$(call make_disk_image,fat32,$(DISK_IMG))
endif

clean:
	rm -rf $(APP)/*.bin $(APP)/*.elf $(OUT_CONFIG)
	cargo clean

.PHONY: all defconfig oldconfig build disasm run justrun debug clippy fmt \
	disk_img clean visionfive2
