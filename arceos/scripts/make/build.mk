# Main build script

include scripts/make/cargo.mk

rust_package := $(shell sed -n 's/^name = "\([a-z0-9A-Z_\-]*\)"/\1/p' $(APP)/Cargo.toml | head -n 1)
rust_elf := $(TARGET_DIR)/$(TARGET)/$(MODE)/$(rust_package)

ifneq ($(filter $(MAKECMDGOALS),defconfig oldconfig clippy),)
  $(if $(V), $(info RUSTFLAGS: "$(RUSTFLAGS)"))
else
  ifneq ($(V),)
    $(info APP: "$(APP)")
    $(info FEATURES: "$(FEATURES)")
    $(info arceos features: "$(AX_FEAT)")
  endif
  RUSTFLAGS += $(RUSTFLAGS_LINK_ARGS)
  $(if $(V), $(info RUSTFLAGS: "$(RUSTFLAGS)"))
endif
export RUSTFLAGS

_cargo_build: oldconfig
	@printf "    $(GREEN_C)Building$(END_C) App: $(APP_NAME), Arch: $(ARCH), Platform: $(PLAT_NAME)\n"
	$(call cargo_build,$(APP),$(AX_FEAT))
	@cp $(rust_elf) $(OUT_ELF)

$(OUT_DIR):
	$(call run_cmd,mkdir,-p $@)

$(OUT_BIN): _cargo_build
	$(call run_cmd,$(OBJCOPY),$(OUT_ELF) --strip-all -O binary $@)

.PHONY: _cargo_build
