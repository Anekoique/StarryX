# ==============================================================================
# StarryX Build System
# ==============================================================================
AX_ROOT ?= $(PWD)/arceos
AX_TESTCASE ?= oscomp
ARCH ?= x86_64
LOG ?= off
FEATURES ?= fp_simd,lwext4_rs
export AX_TESTCASES_LIST=$(shell cat ./apps/$(AX_TESTCASE)/testcase_list | tr '\n' ',')

export NO_AXSTD := y
export AX_LIB := axfeat

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
  $(error ARCH must be one of: x86_64, aarch64, riscv64, loongarch64)
endif

# ==============================================================================
# Main Targets
# ==============================================================================
.DEFAULT_GOAL := all
all: oscomp_build

set_env:
	@sed -e "s|%AX_ROOT%|$(AX_ROOT)|g" xcore/src/config/config.toml.temp > .cargo/config.toml

vf2_config:
	@$(MAKE) defconfig ARCH=riscv64 PLAT_NAME=riscv64-visionfive2

oscomp_build:
	@echo "Building for OS Competition..."
	@mkdir -p .cargo
	@sed -e "s|%AX_ROOT%|$(AX_ROOT)|g" xcore/src/config/config.toml.temp > .cargo/config.toml
	@RUSTUP_TOOLCHAIN=nightly-2025-01-18 $(MAKE) oscomp_binary ARCH=riscv64 BUS=mmio
	@RUSTUP_TOOLCHAIN=nightly-2025-01-18 $(MAKE) oscomp_binary ARCH=loongarch64

oscomp_binary: defconfig
	@echo "Building for $(ARCH) architecture..."
	@if [ -d "$(PWD)/bin" ]; then cp -r $(PWD)/bin/* /root/.cargo/bin; fi
	@$(MAKE) -C $(AX_ROOT) A=$(PWD) build
	@if [ "$(ARCH)" = "riscv64" ]; then \
		cp $$(basename $(PWD))_$(ARCH)-qemu-virt.bin kernel-rv; \
	else \
		cp $$(basename $(PWD))_$(ARCH)-qemu-virt.elf kernel-la; \
	fi

vf2: vf2_config
	@echo "Building for VisionFive2..."
	@RUSTUP_TOOLCHAIN=nightly-2025-01-18 \
		$(MAKE) -C $(AX_ROOT) A=$(PWD) visionfive2 \
			PLAT_NAME=riscv64-visionfive2 ARCH=riscv64 BUS=mmio \
			FEATURES=$(FEATURES),driver-visionfive2-sd
	@mv $(AX_ROOT)/arceos-vf2.itb $(PWD)/starryx-vf2.itb

# ==============================================================================
# Run Targets
# ==============================================================================
oscomp_run: defconfig setup_disk_image set_env
	@echo "Running OS competition test..."
	@$(MAKE) AX_TESTCASE=oscomp BLK=y NET=y FEATURES=$(FEATURES),driver-virtio-blk LOG=$(LOG) run

rv: defconfig set_env
	@echo "Running OS competition test for RISC-V..."
	@cp $(PWD)/sdcard-rv.img $(AX_ROOT)/disk.img
	@$(MAKE) AX_TESTCASE=oscomp BLK=y NET=y FEATURES=$(FEATURES),driver-virtio-blk LOG=$(LOG) run

la: defconfig set_env
	@echo "Running OS competition test for LoongArch..."
	@cp $(PWD)/sdcard-la.img $(AX_ROOT)/disk.img
	@$(MAKE) AX_TESTCASE=oscomp BLK=y NET=y FEATURES=$(FEATURES),driver-virtio-blk LOG=$(LOG) run

oscomp_debug: defconfig setup_disk_image
	@echo "Starting debug session..."
	@$(MAKE) AX_TESTCASE=oscomp BLK=y NET=y FEATURES=$(FEATURES) LOG=$(LOG) debug

# ==============================================================================
# User Application Targets
# ==============================================================================
user_apps:
	@echo "Building user applications for $(AX_TESTCASE)..."
	@$(MAKE) -C ./apps/$(AX_TESTCASE) ARCH=$(ARCH) build
	@echo "Creating disk image..."
	@if [ -z "$$(command -v sudo)" ]; then \
		./build_img.sh -a $(ARCH) -file ./apps/$(AX_TESTCASE)/build/$(ARCH) -s 20 -fs ext4; \
	else \
		sudo ./build_img.sh -a $(ARCH) -file ./apps/$(AX_TESTCASE)/build/$(ARCH) -s 20 -fs ext4; \
	fi
	@mv ./disk.img $(AX_ROOT)/disk.img

run_apps:
	@$(MAKE) AX_TESTCASE=$(AX_TESTCASE) ARCH=$(ARCH) BLK=y NET=y FEATURES=$(FEATURES) LOG=$(LOG) ACCEL=n run

# ==============================================================================
# Development Targets
# ==============================================================================
clippy: defconfig
	@echo "Running clippy checks..."
	@AX_CONFIG_PATH=$(PWD)/.axconfig.toml cargo clippy \
		--target $(TARGET) --all-features -- -D warnings -A clippy::new_without_default

defconfig build run justrun debug disasm:
	@$(MAKE) -C $(AX_ROOT) A=$(PWD) $@

setup_disk_image:
	@echo "Setting up disk image for $(ARCH)..."
	@if [ ! -f $(PWD)/sdcard-$(ARCH).img ]; then \
		echo "Downloading disk image..."; \
		wget https://github.com/Azure-stars/testsuits-for-oskernel/releases/download/v0.2/sdcard-$(ARCH).img.gz; \
		gunzip $(PWD)/sdcard-$(ARCH).img.gz; \
	fi
	@cp $(PWD)/sdcard-$(ARCH).img $(AX_ROOT)/disk.img

DOCKER ?= docker.educg.net/cg/os-contest:20250714
docker:
	docker run --rm -it -v .:/code --entrypoint bash -w /code --privileged $(DOCKER)

# ==============================================================================
# Clean Target
# ==============================================================================
clean:
	@echo "Cleaning build artifacts..."
	@$(MAKE) -C $(AX_ROOT) A=$(PWD) ARCH=$(ARCH) clean
	@for dir in $$(ls ./apps 2>/dev/null || echo ""); do \
		if [ -d "./apps/$$dir" ] && [ -f "./apps/$$dir/Makefile" ]; then \
			echo "Cleaning $$dir..."; \
			$(MAKE) -C ./apps/$$dir clean; \
		fi; \
	done
	@cargo clean
	@rm -f kernel-rv kernel-la .cargo/config.toml
	@echo "Clean completed!"

# ==============================================================================
# Help Target
# ==============================================================================
help:
	@echo "StarryX Build System"
	@echo "==================="
	@echo ""
	@echo "Main targets:"
	@echo "  all              - Build for OS competition (default)"
	@echo "  oscomp_build     - Build kernels for OS competition"
	@echo "  oscomp_run       - Run OS competition test"
	@echo "  rv               - Run RISC-V specific test"
	@echo "  la               - Run LoongArch specific test"
	@echo "  oscomp_debug     - Debug OS competition"
	@echo ""
	@echo "Development targets:"
	@echo "  build            - Build the kernel"
	@echo "  run              - Build and run the kernel"
	@echo "  debug            - Build and debug the kernel"
	@echo "  clippy           - Run code linting"
	@echo "  clean            - Clean all build artifacts"
	@echo ""
	@echo "User application targets:"
	@echo "  user_apps        - Build user applications"
	@echo "  run_apps         - Run user applications"
	@echo ""
	@echo "Configuration variables:"
	@echo "  ARCH=<arch>      - Target architecture (x86_64, aarch64, riscv64, loongarch64)"
	@echo "  AX_TESTCASE=<tc> - Test case to build (default: oscomp)"
	@echo "  LOG=<level>      - Log level (default: off)"
	@echo "  FEATURES=<feat>  - Build features (default: fp_simd,lwext4_rs)"
	@echo ""
	@echo "Examples:"
	@echo "  make oscomp_run ARCH=riscv64 LOG=info"
	@echo "  make user_apps AX_TESTCASE=libc ARCH=aarch64"
	@echo "  make clean ARCH=x86_64"

# ==============================================================================
# Phony Targets
# ==============================================================================
.PHONY: all oscomp_build oscomp_binary
.PHONY: oscomp_run rv la oscomp_debug
.PHONY: user_apps run_apps
.PHONY: clippy defconfig build run justrun debug disasm
.PHONY: setup_disk_image clean help
