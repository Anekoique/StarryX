# Cargo build arguments

ifeq ($(V),1)
  verbose := -v
else ifeq ($(V),2)
  verbose := -vv
else
  verbose :=
endif

build_args-release := --release

build_args := \
  -Z unstable-options \
  --target $(TARGET) \
  --target-dir $(TARGET_DIR) \
  $(build_args-$(MODE)) \
  $(verbose)

RUSTFLAGS := -A unsafe_op_in_unsafe_fn
RUSTFLAGS_LINK_ARGS := -C link-arg=-T$(LD_SCRIPT) -C link-arg=-no-pie -C link-arg=-znostart-stop-gc

define cargo_build
  $(call run_cmd,cargo build,$(build_args) --manifest-path "$(1)/Cargo.toml" --features "$(strip $(2))" $(if $(strip $(ROOT_FEATURES)),--features "$(strip $(ROOT_FEATURES))",))
endef

clippy_args := -A clippy::new_without_default -A unsafe_op_in_unsafe_fn

define cargo_clippy_root
  $(call run_cmd,cargo clippy,$(build_args) --manifest-path "$(APP)/Cargo.toml" --features "$(strip $(AX_FEAT))" -- $(clippy_args))
endef
