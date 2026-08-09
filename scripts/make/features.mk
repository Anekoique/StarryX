# Feature resolution for the retained xfeat-based build.

empty :=
space := $(empty) $(empty)
comma := ,

override FEATURES := $(strip $(subst $(comma),$(space),$(FEATURES)))

xcore_features :=

ifneq ($(filter $(LOG),off error warn info debug trace),)
  xcore_features += log-level-$(LOG)
else
  $(error "LOG" must be one of "off", "error", "warn", "info", "debug", "trace")
endif

ifeq ($(BUS),mmio)
  xcore_features += bus-mmio
endif

ifeq ($(shell test $(SMP) -gt 1; echo $$?),0)
  xcore_features += smp
endif

xcore_features += $(FEATURES)

XCORE_FEATURES := $(strip $(addprefix xfeat/,$(xcore_features)))
