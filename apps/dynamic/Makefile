# Build testcases for dynamic linking programs with musl

ARCH ?= riscv64
# Use musl for dynamic linking
PREFIX := $(ARCH)-linux-musl

# Build target for c programs with dynamic linking
CC := $(PREFIX)-gcc

# Get musl sysroot and dynamic linker paths
MUSL_SYSROOT := $(shell $(CC) -print-sysroot)
MUSL_LIB_DIR := $(MUSL_SYSROOT)/lib
DYNAMIC_LINKER := /lib/ld-musl-$(ARCH).so.1

# Dynamic linking flags for musl
CFLAGS := -Wall -O2 -fPIC
LDFLAGS := -L$(MUSL_LIB_DIR) -L../../lib -Wl,-rpath,/lib -Wl,-rpath,../../lib -Wl,--dynamic-linker=$(DYNAMIC_LINKER)

# Standard libraries for dynamic linking with musl
LIBS := -lc -lpthread -lm

# Source and build directories
SRC_DIR := .
BUILD_DIR := build
TARGET_DIR := $(BUILD_DIR)/$(ARCH)

all: build

build: build_dir build_c copy_libs

build_dir:
	@mkdir -p $(BUILD_DIR)
	@mkdir -p $(TARGET_DIR)

build_c:
	@echo "Building dynamic linking C programs for $(ARCH)..."
	@for src in $(wildcard *.c); do \
		if [ -f "$$src" ]; then \
			echo "Building $$src"; \
			name=$$(basename $$src .c); \
			$(CC) $(CFLAGS) -o $(TARGET_DIR)/$${name}_c $$src $(LDFLAGS) $(LIBS); \
		fi \
	done
	@echo "Dynamic linking build complete."

# Copy required libraries to build directory
copy_libs:
	@echo "Copying musl dynamic libraries from toolchain..."
	@mkdir -p $(TARGET_DIR)/lib
	@# Copy musl libraries from toolchain
	@if [ -f "$(MUSL_LIB_DIR)/libc.so" ]; then \
		echo "  Copying musl libc.so"; \
		cp -f $(MUSL_LIB_DIR)/libc.so $(TARGET_DIR)/lib/; \
	fi
	@if [ -f "$(MUSL_LIB_DIR)/ld-musl-$(ARCH).so.1" ]; then \
		echo "  Copying musl dynamic linker"; \
		cp -f $(MUSL_LIB_DIR)/ld-musl-$(ARCH).so.1 $(TARGET_DIR)/lib/; \
	else \
		echo "  Creating dynamic linker symlink"; \
		ln -sf libc.so $(TARGET_DIR)/lib/ld-musl-$(ARCH).so.1; \
	fi
	@# Copy additional libraries if they exist in musl toolchain
	@if [ -f "$(MUSL_LIB_DIR)/libpthread.so" ]; then \
		echo "  Copying libpthread.so"; \
		cp -f $(MUSL_LIB_DIR)/libpthread.so $(TARGET_DIR)/lib/; \
	fi
	@if [ -f "$(MUSL_LIB_DIR)/libm.so" ]; then \
		echo "  Copying libm.so"; \
		cp -f $(MUSL_LIB_DIR)/libm.so $(TARGET_DIR)/lib/; \
	fi
	@# Copy any additional libraries from ../../lib if it exists
	@if [ -d "../../lib" ]; then \
		echo "  Copying additional libraries from ../../lib"; \
		cp -f ../../lib/*.so* $(TARGET_DIR)/lib/ 2>/dev/null || true; \
	fi

# Test a specific program  
test: build copy_libs
	@echo "Testing musl dynamic programs..."
	@export LD_LIBRARY_PATH="../../lib:$(TARGET_DIR)/lib"; \
	for prog in $(TARGET_DIR)/*_c; do \
		if [ -x "$$prog" ]; then \
			echo "Running $$prog with musl"; \
			$$prog || echo "Warning: $$prog failed"; \
		fi \
	done

# Alternative static build for fallback
build_static:
	@echo "Building static linking C programs for $(ARCH) (fallback)..."
	@for src in $(wildcard *.c); do \
		if [ -f "$$src" ]; then \
			echo "Building $$src (static)"; \
			name=$$(basename $$src .c); \
			$(CC) $(CFLAGS) -static -o $(TARGET_DIR)/$${name}_static $$src $(LIBS); \
		fi \
	done

clean:
	@rm -rf $(BUILD_DIR)

# Show build information
info:
	@echo "=== Musl Dynamic Linking Build Configuration ==="
	@echo "Architecture: $(ARCH)"
	@echo "Compiler: $(CC)"
	@echo "Prefix: $(PREFIX)"
	@echo "Musl Sysroot: $(MUSL_SYSROOT)"
	@echo "Musl Lib Dir: $(MUSL_LIB_DIR)"
	@echo "Dynamic Linker: $(DYNAMIC_LINKER)"
	@echo "C Flags: $(CFLAGS)"
	@echo "LD Flags: $(LDFLAGS)"
	@echo "Libraries: $(LIBS)"
	@echo "Build Dir: $(TARGET_DIR)"
	@echo ""
	@echo "Available targets:"
	@echo "  build        - Build dynamic programs (includes copy_libs)"
	@echo "  build_static - Build static programs (fallback)"
	@echo "  copy_libs    - Copy musl libraries from toolchain"
	@echo "  test         - Test dynamic programs"
	@echo "  clean        - Clean build directory"
	@echo "  info         - Show this information"

.PHONY: all build build_dir build_c copy_libs test clean info build_static