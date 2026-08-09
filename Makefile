TOOLCHAIN := nightly-2026-03-15
ROOT_DIR  := $(CURDIR)

# ----- Build configuration -----------------------------------------------------
ARCH       ?= riscv64
PLATFORM   ?=
SMP        ?= 1
MODE       ?= release
LOG        ?= off
V          ?=

A          ?=
APP        := $(abspath $(if $(strip $(A)),$(A),$(ROOT_DIR)/starry))
TARGET_DIR ?= $(ROOT_DIR)/target
EXTRA_CONFIG ?=
OUT_CONFIG ?=
FEATURES   ?= fp_simd,lwext4_rs

# System-test selection. The kernel is unchanged; a copied rootfs carries the
# generated /xtest bundle.
PROFILE       ?= smoke
CASE          ?=
INTERACTIVE   ?= 0
XTEST_TIMEOUT ?=
XTEST_DISK_IMG ?=

# Export user-selected xtest values without interpolating them into a shell
# recipe. The host runner validates each value before use.
export XTEST_ARCH := $(ARCH)
export XTEST_PROFILE := $(PROFILE)
export XTEST_CASE := $(CASE)
export XTEST_INTERACTIVE := $(INTERACTIVE)
export XTEST_DISK_IMG
export XTEST_HOST_TARGET_DIR := $(TARGET_DIR)/xtest-host

ifneq ($(strip $(XTEST_TIMEOUT)),)
export XTEST_RUN_TIMEOUT := $(XTEST_TIMEOUT)
endif

# ----- QEMU / runtime --------------------------------------------------------
BLK        ?= n
NET        ?= n
GRAPHIC    ?= n
BUS        ?= pci
MEM        ?= 1G
ACCEL      ?=
DISK_IMG   ?= $(ROOT_DIR)/disk.img
QEMU_LOG   ?= n
NET_DUMP   ?= n
NET_DEV    ?= user
VFIO_PCI   ?=
VHOST      ?= n
IP         ?= 10.0.2.15
GW         ?= 10.0.2.2

ROOTFS_URL   := https://github.com/Starry-OS/rootfs/releases/download/20260214
ROOTFS_IMG   := rootfs-$(ARCH).img
DOCKER_IMAGE := docker.educg.net/cg/os-contest:20250714

export RUSTUP_TOOLCHAIN := $(TOOLCHAIN)

# ----- Sanity checks ---------------------------------------------------------
ifeq ($(wildcard $(APP)),)
  $(error Application path "$(APP)" is not valid)
endif
ifeq ($(wildcard $(APP)/Cargo.toml),)
  $(error StarryX build expects a Rust app with Cargo.toml at "$(APP)")
endif

# ----- Resolve platform / features / paths -----------------------------------
include scripts/make/features.mk
include scripts/make/platform.mk

FEATURES_CLI  := $(subst $(space),$(comma),$(FEATURES))
QEMU_FEATURES := $(FEATURES_CLI),driver-virtio-blk
VF2_FEATURES  := $(FEATURES_CLI),driver-visionfive2-sd
export XTEST_KERNEL_FEATURES := $(QEMU_FEATURES)

OUT_CONFIG := $(abspath $(if $(strip $(OUT_CONFIG)),$(OUT_CONFIG),$(ROOT_DIR)/.xconfig.$(PLAT_NAME).toml))

ifeq ($(ARCH),riscv64)
  TARGET := riscv64gc-unknown-none-elf
else ifeq ($(ARCH),loongarch64)
  TARGET := loongarch64-unknown-none
else
  $(error "ARCH" must be one of "riscv64" or "loongarch64")
endif

# Exported as build-time environment consumed by XCore components.
export XCORE_ARCH        := $(ARCH)
export XCORE_PLATFORM    := $(PLAT_NAME)
export XCORE_SMP         := $(SMP)
export XCORE_MODE        := $(MODE)
export XCORE_LOG         := $(LOG)
export XCORE_TARGET      := $(TARGET)
export XCORE_IP          := $(IP)
export XCORE_GW          := $(GW)
export XCORE_CONFIG_PATH := $(OUT_CONFIG)

# ----- Tool overrides --------------------------------------------------------
OBJDUMP ?= rust-objdump -d --print-imm-hex --x86-asm-syntax=intel
OBJCOPY ?= rust-objcopy --binary-architecture=$(ARCH)
GDB     ?= gdb

OUT_DIR   ?= $(abspath $(if $(strip $(A)),$(APP),$(ROOT_DIR)))
APP_NAME  := $(if $(strip $(A)),$(notdir $(APP)),$(notdir $(ROOT_DIR)))
LD_SCRIPT := $(TARGET_DIR)/$(TARGET)/$(MODE)/linker_$(PLAT_NAME).lds
OUT_ELF   := $(OUT_DIR)/$(APP_NAME)_$(PLAT_NAME).elf
OUT_BIN   := $(patsubst %.elf,%.bin,$(OUT_ELF))
FINAL_IMG := $(OUT_BIN)

# ----- Sub-makefiles ---------------------------------------------------------
include scripts/make/utils.mk
include scripts/make/config.mk
include scripts/make/build.mk
include scripts/make/qemu.mk

# ----- Targets ---------------------------------------------------------------
.DEFAULT_GOAL := rv

.PHONY: all defconfig oldconfig build disasm run justrun debug clippy fmt \
        rootfs qemu_rootfs qemu_run rv la vf2 disk_img clean docker test \
        _xtest_run

all: rv

defconfig: _xconfig-gen
	$(call defconfig)

oldconfig: _xconfig-gen
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

rootfs:
	@if [ ! -f $(ROOTFS_IMG) ]; then \
		curl -f -L $(ROOTFS_URL)/$(ROOTFS_IMG).xz -O; \
		xz -d $(ROOTFS_IMG).xz; \
	fi

qemu_rootfs: rootfs
	cp $(ROOTFS_IMG) $(DISK_IMG)

qemu_run: qemu_rootfs run

test: rootfs
	env -u RUSTFLAGS -u MAKEFLAGS -u MAKEOVERRIDES -u MFLAGS \
		CARGO_TARGET_DIR="$$XTEST_HOST_TARGET_DIR" \
		cargo run --manifest-path xtest/Cargo.toml --release -- run

# Private host-runner seam. Keep all kernel features, devices, and the copied
# image in one recursive build/run invocation.
_xtest_run:
	@test -n "$$XTEST_DISK_IMG" || { echo "error: XTEST_DISK_IMG is required" >&2; exit 2; }
	@case "$$XTEST_DISK_IMG" in /*) ;; *) echo "error: XTEST_DISK_IMG must be absolute" >&2; exit 2;; esac
	@case "$$XTEST_DISK_IMG" in *[!A-Za-z0-9_./-]*) echo "error: XTEST_DISK_IMG contains unsupported characters" >&2; exit 2;; esac
	@case "$$XTEST_KERNEL_FEATURES" in *[!A-Za-z0-9_,-]*) echo "error: kernel feature list contains unsupported characters" >&2; exit 2;; esac
	@$(MAKE) ARCH="$$XTEST_ARCH" BLK=y NET=y \
		FEATURES="$$XTEST_KERNEL_FEATURES" DISK_IMG="$$XTEST_DISK_IMG" run

rv:
	@$(MAKE) ARCH=riscv64 BLK=y NET=y FEATURES=$(QEMU_FEATURES) qemu_run

la:
	@$(MAKE) ARCH=loongarch64 BLK=y NET=y FEATURES=$(QEMU_FEATURES) qemu_run

vf2:
	@$(MAKE) PLATFORM=riscv64-visionfive2 ARCH=riscv64 \
		BUS=mmio SMP=2 FEATURES=$(VF2_FEATURES) build

disk_img:
ifneq ($(wildcard $(DISK_IMG)),)
	@printf "$(YELLOW_C)warning$(END_C): disk image \"$(DISK_IMG)\" already exists!\n"
else
	$(call make_disk_image,fat32,$(DISK_IMG))
endif

clean:
	rm -rf $(OUT_DIR)/*.bin $(OUT_DIR)/*.elf $(OUT_CONFIG)
	cargo clean

docker:
	docker run --rm -it -v .:/code --entrypoint bash -w /code --privileged $(DOCKER_IMAGE)
