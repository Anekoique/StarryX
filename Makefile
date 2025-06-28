# StarryX Build System
AX_ROOT ?= $(PWD)/arceos
AX_TESTCASE ?= oscomp
ARCH ?= x86_64
LOG ?= off
FEATURES ?= fp_simd,lwext4_rs
EXTRA_CONFIG ?= $(PWD)/configs/$(ARCH).toml

# Build configuration
export NO_AXSTD := y
export AX_LIB := axfeat

# Output files
DIR := $(shell basename $(PWD))
OUT_ELF := $(DIR)_$(ARCH)-qemu-virt.elf
OUT_BIN := $(DIR)_$(ARCH)-qemu-virt.bin

# Target architecture mapping
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
  $(error ARCH must be one of x86_64, aarch64, riscv64, loongarch64)
endif

# Image download URL
IMG_URL := https://github.com/Azure-stars/testsuits-for-oskernel/releases/download/v0.2/sdcard-$(ARCH).img.gz

# Default target
all: oscomp_build

# Build binary for specific architecture
oscomp_binary: ax_root defconfig
	@echo "Building for $(ARCH) architecture..."
	@if [ -d "$(PWD)/bin" ]; then cp -r $(PWD)/bin/* /root/.cargo/bin; fi
	@make -C $(AX_ROOT) A=$(PWD) EXTRA_CONFIG=$(EXTRA_CONFIG) build
	@if [ "$(ARCH)" = "riscv64" ]; then \
		cp $(OUT_BIN) kernel-rv; \
	else \
		cp $(OUT_ELF) kernel-la; \
	fi

# Build for OS competition (both architectures)
oscomp_build:
	@echo "Building for OS Competition..."
	@mkdir -p .cargo
	@sed -e "s|%AX_ROOT%|$(AX_ROOT)|g" configs/config.toml.temp > .cargo/config.toml
	@RUSTUP_TOOLCHAIN=nightly-2025-01-18 $(MAKE) oscomp_binary ARCH=riscv64 AX_TESTCASE=oscomp BUS=mmio FEATURES=$(FEATURES)
	@RUSTUP_TOOLCHAIN=nightly-2025-01-18 $(MAKE) oscomp_binary ARCH=loongarch64 AX_TESTCASE=oscomp FEATURES=$(FEATURES)

# Build user applications
user_apps:
	@echo "Building user applications for $(AX_TESTCASE)..."
	@make -C ./apps/$(AX_TESTCASE) ARCH=$(ARCH) build
	@echo "Creating disk image..."
	@if [ -z "$(shell command -v sudo)" ]; then \
		./build_img.sh -a $(ARCH) -file ./apps/$(AX_TESTCASE)/build/$(ARCH) -s 20 -fs ext4; \
	else \
		sudo ./build_img.sh -a $(ARCH) -file ./apps/$(AX_TESTCASE)/build/$(ARCH) -s 20 -fs ext4; \
	fi
	@mv ./disk.img $(AX_ROOT)/disk.img

# Run user applications
run_apps:
	@make AX_TESTCASE=$(AX_TESTCASE) ARCH=$(ARCH) BLK=y NET=y FEATURES=$(FEATURES) LOG=$(LOG) ACCEL=n run

# Code linting
clippy: defconfig
	@echo "Running clippy checks..."
	@AX_CONFIG_PATH=$(PWD)/.axconfig.toml cargo clippy --target $(TARGET) --all-features -- -D warnings -A clippy::new_without_default

# Core build targets (delegate to ArceOS)
defconfig build run justrun debug disasm: ax_root
	@make -C $(AX_ROOT) A=$(PWD) EXTRA_CONFIG=$(EXTRA_CONFIG) $@

# Download and setup disk image
setup_disk_image:
	@echo "Setting up disk image for $(ARCH)..."
	@if [ ! -f $(PWD)/sdcard-$(ARCH).img ]; then \
		echo "Downloading disk image..."; \
		wget $(IMG_URL); \
		gunzip $(PWD)/sdcard-$(ARCH).img.gz; \
	fi
	@cp $(PWD)/sdcard-$(ARCH).img $(AX_ROOT)/disk.img

# Run OS competition test
oscomp_run: ax_root defconfig setup_disk_image
	@echo "Running OS competition test..."
	@$(MAKE) AX_TESTCASE=oscomp BLK=y NET=y FEATURES=$(FEATURES) LOG=$(LOG) run

# Run OS competition test (alternative setup)
rv: ax_root defconfig
	@echo "Running OS competition test for RISC-V"
	@cp $(PWD)/sdcard-rv.img $(AX_ROOT)/disk.img
	@$(MAKE) AX_TESTCASE=oscomp BLK=y NET=y FEATURES=$(FEATURES) LOG=$(LOG) run

la: ax_root defconfig
	@echo "Running OS competition test for LoongArch"
	@cp $(PWD)/sdcard-la.img $(AX_ROOT)/disk.img
	@$(MAKE) AX_TESTCASE=oscomp BLK=y NET=y FEATURES=$(FEATURES) LOG=$(LOG) run

# Debug OS competition
oscomp_debug: ax_root defconfig setup_disk_image
	@echo "Starting debug session..."
	@$(MAKE) AX_TESTCASE=oscomp BLK=y NET=y FEATURES=$(FEATURES) LOG=$(LOG) debug

clean: ax_root
	@echo "Cleaning build artifacts..."
	@make -C $(AX_ROOT) A=$(PWD) ARCH=$(ARCH) clean
	@for dir in $(shell ls ./apps 2>/dev/null || echo ""); do \
		if [ -d "./apps/$$dir" ] && [ -f "./apps/$$dir/Makefile" ]; then \
			echo "Cleaning $$dir..."; \
			make -C ./apps/$$dir clean; \
		fi; \
	done
	@cargo clean
	@rm -f kernel-rv kernel-la
	@rm -f .cargo/config.toml
	@echo "Clean completed!"

help:
	@echo "StarryX Build System"
	@echo "==================="
	@echo ""
	@echo "Main targets:"
	@echo "  all              - Build for OS competition (default)"
	@echo "  oscomp_build     - Build kernels for OS competition"
	@echo "  oscomp_run       - Run OS competition test"
	@echo "  rv               - Run OS competition test for RISC-V"
	@echo "  la               - Run OS competition test for LoongArch"
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
	@echo "  FEATURES=<feat>  - Build features (default: fp_simd)"
	@echo ""
	@echo "Example:"
	@echo "  make oscomp_run ARCH=riscv64 LOG=info"

.PHONY: all build run justrun debug disasm clean clippy
.PHONY: oscomp_build oscomp_run rv la oscomp_debug
.PHONY: ax_root user_apps run_apps setup_cargo