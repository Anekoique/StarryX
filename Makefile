TOOLCHAIN := nightly-2026-03-15
PROJECT := $(notdir $(CURDIR))

export ARCH ?= riscv64
export LOG ?= off
export FEATURES ?= fp_simd,lwext4_rs

N ?= 1

ROOTFS_URL := https://github.com/Starry-OS/rootfs/releases/download/20260214
ROOTFS_IMG := rootfs-$(ARCH).img
DOCKER_IMAGE := docker.educg.net/cg/os-contest:20250714

ARCEOS_MAKE := RUSTUP_TOOLCHAIN=$(TOOLCHAIN) $(MAKE) -C arceos

all: rv

qemu_run:
	@if [ ! -f $(ROOTFS_IMG) ]; then \
		curl -f -L $(ROOTFS_URL)/$(ROOTFS_IMG).xz -O; \
		xz -d $(ROOTFS_IMG).xz; \
	fi
	cp $(ROOTFS_IMG) arceos/disk.img
	@$(ARCEOS_MAKE) run

rv:
	@$(MAKE) ARCH=riscv64 BLK=y NET=y FEATURES=$(FEATURES),driver-virtio-blk qemu_run

la:
	@$(MAKE) ARCH=loongarch64 BLK=y NET=y FEATURES=$(FEATURES),driver-virtio-blk qemu_run

vf2:
	@$(ARCEOS_MAKE) PLATFORM=riscv64-visionfive2 ARCH=riscv64 \
		BUS=mmio FEATURES=$(FEATURES),driver-visionfive2-sd LOG=$(LOG) SMP=2 build
	sudo cp StarryX_riscv64-visionfive2.bin /srv/tftp/

build run debug disasm defconfig:
	@$(ARCEOS_MAKE) $@

docker:
	docker run --rm -it -v .:/code --entrypoint bash -w /code --privileged $(DOCKER_IMAGE)

.PHONY: all qemu_run rv la vf2 sdcard clippy build run debug disasm defconfig docker
