# Architecture and platform resolving.
#
# Resolves PLAT_NAME / PLAT_CONFIG / ARCH from one of:
#   * default per-ARCH platform when PLATFORM is empty,
#   * a builtin platform name (e.g. PLATFORM=riscv64-qemu-virt),
#   * a path to a custom platform .toml file.

builtin_platforms := $(patsubst arceos/configs/platforms/%.toml,%,$(wildcard arceos/configs/platforms/*))

ifeq ($(PLATFORM),)
  ifeq ($(ARCH),riscv64)
    PLAT_NAME := riscv64-qemu-virt
  else ifeq ($(ARCH),loongarch64)
    PLAT_NAME := loongarch64-qemu-virt
  else
    $(error "ARCH" must be one of "riscv64" or "loongarch64")
  endif
  PLAT_CONFIG := arceos/configs/platforms/$(PLAT_NAME).toml
else ifneq ($(wildcard $(PLATFORM)),)
  # Custom platform supplied as a path to a .toml file.
  PLAT_CONFIG := $(PLATFORM)
  PLAT_NAME   := $(patsubst "%",%,$(shell axconfig-gen $(PLAT_CONFIG) -r platform))
  _arch       := $(patsubst "%",%,$(shell axconfig-gen $(PLAT_CONFIG) -r arch))
else ifneq ($(filter $(PLATFORM),$(builtin_platforms)),)
  # Builtin platform (matches a file under arceos/configs/platforms/).
  PLAT_NAME   := $(PLATFORM)
  PLAT_CONFIG := arceos/configs/platforms/$(PLAT_NAME).toml
  _arch       := $(word 1,$(subst -, ,$(PLATFORM)))
else
  $(error "PLATFORM" must be one of "$(builtin_platforms)" or a valid path to a toml file)
endif

# When PLATFORM was supplied, _arch is set above and may need to override or
# validate ARCH from the command line.
ifdef _arch
  ifeq ($(origin ARCH),command line)
    ifneq ($(ARCH),$(_arch))
      $(error "ARCH=$(ARCH)" is not compatible with "PLATFORM=$(PLATFORM)")
    endif
  endif
  ARCH := $(_arch)
endif
