# xtest — test-rootfs build pipeline.
#
# All cross-compilation runs inside the contest Docker image, pinned by digest.
# Host needs Docker + GNU make + a POSIX shell; the toolchain lives in the image.

ARCH ?= riscv64

DOCKER_IMAGE := docker.educg.net/cg/os-contest@sha256:742479b5cd11b24501e2eccf5d409b78b76ba7aabcb87f815bbd5908a288313b

# Per-arch musl cross compilers (paths inside the contest image).
ifeq ($(ARCH),riscv64)
  MUSL_CC := /opt/riscv64-linux-musl-cross/bin/riscv64-linux-musl-gcc
else ifeq ($(ARCH),loongarch64)
  MUSL_CC := /opt/loongarch64-linux-musl-cross/bin/loongarch64-linux-musl-gcc
else
  $(error ARCH must be one of "riscv64" or "loongarch64")
endif

ROOT_DIR    := $(abspath $(dir $(lastword $(MAKEFILE_LIST)))/..)
BUILD_DIR   := $(ROOT_DIR)/xtest/build/$(ARCH)
STAGE_DIR   := $(BUILD_DIR)/stage
TESTS_IMG   := $(BUILD_DIR)/tests-rootfs-$(ARCH).img
ROOTFS_IMG  := $(ROOT_DIR)/rootfs-$(ARCH).img

# Repo bind-mounted at /code; matches the contest's docker convention.
DOCKER_RUN := docker run --rm --privileged \
    -v $(ROOT_DIR):/code \
    -w /code \
    -e ARCH=$(ARCH) \
    -e MUSL_CC=$(MUSL_CC) \
    $(DOCKER_IMAGE)

.PHONY: all build-c build-suites stage bake-image clean docker-shell

all: build-c build-suites stage bake-image

build-c:
	@command -v docker >/dev/null 2>&1 || { echo "error: docker not found — install Docker and pull $(DOCKER_IMAGE)" >&2; exit 1; }
	$(DOCKER_RUN) sh /code/xtest/scripts/build/build-c.sh

build-suites:
	@command -v docker >/dev/null 2>&1 || { echo "error: docker not found — install Docker and pull $(DOCKER_IMAGE)" >&2; exit 1; }
	$(DOCKER_RUN) env ROOT_DIR=/code sh /code/xtest/scripts/build/build-suites.sh

stage:
	ARCH=$(ARCH) ROOT_DIR=$(ROOT_DIR) sh $(ROOT_DIR)/xtest/scripts/build/stage.sh

bake-image:
	@test -f $(ROOTFS_IMG) || { echo "error: $(ROOTFS_IMG) missing — run \`make qemu_rootfs ARCH=$(ARCH)\` from the repo root" >&2; exit 1; }
	$(DOCKER_RUN) env \
		ARCH=$(ARCH) \
		ROOTFS_IMG=/code/$(notdir $(ROOTFS_IMG)) \
		STAGE_DIR=$(subst $(ROOT_DIR),/code,$(STAGE_DIR)) \
		OUT_IMG=$(subst $(ROOT_DIR),/code,$(TESTS_IMG)) \
		sh /code/xtest/scripts/build/bake-image.sh

clean:
	rm -rf $(BUILD_DIR)

docker-shell:
	$(DOCKER_RUN) bash
