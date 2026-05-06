# Regenerate the vDSO blobs.
#
# Run after changing source under `xmodules/xvdso/`. Drops the rebuilt
# `.so` files in-place under `xcore/src/vdso/blobs/` (committed and
# embedded into the kernel via `.incbin`).

VDSO_BLOBS_DIR   := $(ROOT_DIR)/xcore/src/vdso/blobs
VDSO_MANIFEST    := $(ROOT_DIR)/xmodules/xvdso/Cargo.toml
VDSO_TARGETS_DIR := $(ROOT_DIR)/xmodules/xvdso/targets
VDSO_TARGET_DIR  := $(ROOT_DIR)/target/vdso-build

define vdso_build
	env -u RUSTFLAGS \
	  cargo build \
	    --manifest-path $(VDSO_MANIFEST) \
	    --target $(VDSO_TARGETS_DIR)/$(1).json \
	    --target-dir $(VDSO_TARGET_DIR)/$(2) \
	    -Z build-std=core \
	    -Z json-target-spec \
	    --release
	@cp $(VDSO_TARGET_DIR)/$(2)/$(1)/release/libxvdso.so \
	    $(VDSO_BLOBS_DIR)/vdso-$(2).so
endef

.PHONY: regenerate-vdso-blobs

regenerate-vdso-blobs:
	$(call vdso_build,riscv64gc-unknown-none-vdso,riscv64)
	$(call vdso_build,loongarch64-unknown-none-vdso,loongarch64)
