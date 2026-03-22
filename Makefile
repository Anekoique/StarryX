TOOLCHAIN := nightly-2026-03-15
PROJECT := $(notdir $(CURDIR))
ROOT_DIR := $(CURDIR)

ARCH ?= riscv64
PLATFORM ?= $(PLAT_NAME)
SMP ?= 1
MODE ?= release
LOG ?= off
V ?=
TARGET_DIR ?= $(abspath $(ROOT_DIR)/target)
EXTRA_CONFIG ?=
OUT_CONFIG ?=

A ?=
APP ?= $(if $(A),$(abspath $(A)),$(ROOT_DIR))
FEATURES ?= fp_simd,lwext4_rs

BLK ?= n
NET ?= n
GRAPHIC ?= n
BUS ?= pci
MEM ?= 1G
ACCEL ?=
DISK_IMG ?= $(ROOT_DIR)/disk.img
QEMU_LOG ?= n
NET_DUMP ?= n
NET_DEV ?= user
VFIO_PCI ?=
VHOST ?= n

IP ?= 10.0.2.15
GW ?= 10.0.2.2

ROOTFS_URL := https://github.com/Starry-OS/rootfs/releases/download/20260214
ROOTFS_IMG := rootfs-$(ARCH).img
DOCKER_IMAGE := docker.educg.net/cg/os-contest:20250714

export RUSTUP_TOOLCHAIN := $(TOOLCHAIN)

ifeq ($(wildcard $(APP)),)
  $(error Application path "$(APP)" is not valid)
endif

ifeq ($(wildcard $(APP)/Cargo.toml),)
  $(error StarryX build expects a Rust app with Cargo.toml at "$(APP)")
endif

include scripts/make/features.mk
include scripts/make/platform.mk

FEATURES_CLI := $(subst $(space),$(comma),$(FEATURES))

ifeq ($(strip $(OUT_CONFIG)),)
  OUT_CONFIG := $(abspath $(ROOT_DIR)/.axconfig.$(PLAT_NAME).toml)
else
  OUT_CONFIG := $(abspath $(OUT_CONFIG))
endif

ifeq ($(ARCH), riscv64)
  TARGET := riscv64gc-unknown-none-elf
else ifeq ($(ARCH), loongarch64)
  TARGET := loongarch64-unknown-none
else
  $(error "ARCH" must be one of "riscv64" or "loongarch64")
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

all: rv

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
	$(call cargo_clippy_root)

fmt:
	cargo fmt --all

qemu_rootfs:
	@if [ ! -f $(ROOTFS_IMG) ]; then \
		curl -f -L $(ROOTFS_URL)/$(ROOTFS_IMG).xz -O; \
		xz -d $(ROOTFS_IMG).xz; \
	fi
	cp $(ROOTFS_IMG) $(DISK_IMG)

qemu_run: qemu_rootfs run

rv:
	@$(MAKE) ARCH=riscv64 BLK=y NET=y FEATURES=$(FEATURES_CLI),driver-virtio-blk qemu_run

la:
	@$(MAKE) ARCH=loongarch64 BLK=y NET=y FEATURES=$(FEATURES_CLI),driver-virtio-blk qemu_run

vf2:
	@$(MAKE) PLATFORM=riscv64-visionfive2 ARCH=riscv64 \
		BUS=mmio FEATURES=$(FEATURES_CLI),driver-visionfive2-sd LOG=$(LOG) SMP=2 visionfive2
	sudo cp StarryX_riscv64-visionfive2.bin /srv/tftp/

disk_img:
ifneq ($(wildcard $(DISK_IMG)),)
	@printf "$(YELLOW_C)warning$(END_C): disk image \"$(DISK_IMG)\" already exists!\n"
else
	$(call make_disk_image,fat32,$(DISK_IMG))
endif

clean:
	rm -rf $(APP)/*.bin $(APP)/*.elf $(OUT_CONFIG)
	cargo clean

docker:
	docker run --rm -it -v .:/code --entrypoint bash -w /code --privileged $(DOCKER_IMAGE)

.PHONY: all defconfig oldconfig build disasm run justrun debug clippy fmt \
	qemu_rootfs qemu_run rv la vf2 disk_img clean docker visionfive2
