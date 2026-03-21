# Feature resolution for the retained axfeat-based build.

empty :=
space := $(empty) $(empty)
comma := ,

override FEATURES := $(strip $(subst $(comma),$(space),$(FEATURES)))

ax_feat :=

ifneq ($(filter $(LOG),off error warn info debug trace),)
  ax_feat += log-level-$(LOG)
else
  $(error "LOG" must be one of "off", "error", "warn", "info", "debug", "trace")
endif

ifeq ($(BUS),mmio)
  ax_feat += bus-mmio
endif

ifeq ($(shell test $(SMP) -gt 1; echo $$?),0)
  ax_feat += smp
endif

ax_feat += $(FEATURES)

AX_FEAT := $(strip $(addprefix axfeat/,$(ax_feat)))
