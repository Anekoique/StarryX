export ARCH := riscv64
export LOG := off
export N := 1
export A := $(PWD)
export FEATURES := fp_simd,lwext4_rs
export NO_AXSTD := y
export AX_LIB := axfeat

IMG_URL = https://github.com/oscomp/testsuits-for-oskernel/releases/download/pre-20250615/
DOCKER = docker.educg.net/cg/os-contest:20250714

ifeq ($(ARCH), riscv64)
	IMG := sdcard-rv.img
else ifeq ($(ARCH), loongarch64)
	IMG := sdcard-la.img
else
	$(error Unsupported architecture: $(ARCH))
endif

all: oscomp

oscomp:
	@mkdir -p .cargo
	@cp xcore/src/config/config.toml.temp .cargo/config.toml
	@if [ -d bin ]; then cp -r bin/* ~/.cargo/bin; fi
	@RUSTUP_TOOLCHAIN=nightly-2025-01-18 $(MAKE) ARCH=riscv64 BUS=mmio build
	cp $$(basename $(PWD))_riscv64-qemu-virt.bin kernel-rv
	@RUSTUP_TOOLCHAIN=nightly-2025-01-18 $(MAKE) ARCH=loongarch64 build
	cp $$(basename $(PWD))_loongarch64-qemu-virt.elf kernel-la

qemu_run:
	@if [ ! -f $(IMG) ]; then \
		wget $(IMG_URL)/$(IMG).xz; \
		xz -d $(IMG).xz; \
	fi
	cp $(IMG) arceos/disk.img
	$(MAKE) run

rv:
	@$(MAKE) ARCH=riscv64 BLK=y NET=y FEATURES=$(FEATURES),driver-virtio-blk qemu_run

la:
	@$(MAKE) ARCH=loongarch64 BLK=y NET=y FEATURES=$(FEATURES),driver-virtio-blk qemu_run

vf2:
	@RUSTUP_TOOLCHAIN=nightly-2025-01-18 $(MAKE) PLAT_NAME=riscv64-visionfive2 ARCH=riscv64 \
		BUS=mmio FEATURES=$(FEATURES),driver-ramdisk LOG=$(LOG) SMP=2 build
	sudo cp StarryX_riscv64-visionfive2.bin /srv/tftp/

clippy: 
	@AX_CONFIG_PATH=.axconfig.toml cargo clippy --all-features -- -D warnings -A clippy::new_without_default

switch:
	cp sdcard-rv$(N).img.bak sdcard-rv.img
	cp sdcard-la$(N).img.bak sdcard-la.img

build run debug disasm: defconfig
	@$(MAKE) -C arceos $@

defconfig:
	@$(MAKE) -C arceos $@

docker:
	docker run --rm -it -v .:/code --entrypoint bash -w /code --privileged $(DOCKER)

.PHONY: all oscomp qemu_run rv la vf2 clippy switch build run debug disasm defconfig docker
