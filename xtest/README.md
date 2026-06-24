# xtest — StarryX test environment

`xtest/` produces a test rootfs image that boots StarryX with first-party
C tests under `/root/tests/c` plus the vendored `iozone` benchmark under
`/root/tests/iozone`. The kernel side is selected at build time via the
`init-test` cargo feature, so `make run` (the normal boot path) is unaffected.

## Layout

```
xtest/
├── Makefile              build pipeline (Docker-driven)
├── README.md
├── .gitignore            ignores build/
├── c/                    first-party C tests (one .c per test)
│   ├── common/           shared header (xtest_assert / xtest_eq)
│   ├── syscall/          getpid, clone_basic, ...
│   ├── signal/           kill_self, ...
│   ├── mm/               mmap_anon, ...
│   └── fs/               open_close, ...
├── iozone/               vendored iozone benchmark (src/ + LICENSE + README)
├── scripts/
│   ├── build/
│   │   ├── build-c.sh      in-Docker: cross-compile xtest/c/**/*.c
│   │   ├── build-iozone.sh in-Docker: cross-compile xtest/iozone/src
│   │   ├── stage.sh        assemble the staged tree
│   │   └── bake-image.sh   in-Docker: build a fresh ext4 image and rsync
│   ├── run-all.sh        in-guest: drive the whole run
│   ├── run-c.sh          in-guest: iterate /root/tests/c/
│   └── run-iozone.sh     in-guest: bounded iozone smoke pass
└── build/                gitignored: per-arch outputs and the test rootfs image
```

## Quick start

Requires Docker (the build runs inside the contest image) **and** a host
musl cross toolchain (the kernel build's `lwext4_rust` invokes
`riscv64-linux-musl-gcc` / `loongarch64-linux-musl-gcc` on the host —
same constraint that `make rv` / `make la` already have).

```sh
make tests       ARCH=riscv64           # build xtest/build/riscv64/tests-rootfs-riscv64.img
make run-tests   ARCH=riscv64           # build kernel with init-test feature, build image, boot
```

The kernel embeds `src/test.sh` instead of `src/init.sh` when built with
`ROOT_FEATURES=init-test`. `make run` does not set `ROOT_FEATURES` and is
unaffected by anything in this directory.

## Adding a first-party C test

Drop a single-file program under `xtest/c/<group>/<name>.c`. It will be
cross-compiled statically against musl by `build-c.sh` and run by
`run-c.sh`. Test names must be unique across `xtest/c/` (we flatten to
`c/<basename>` in the staged tree). A non-zero exit prints `[FAIL] <name>
exit=<n>`; a signal-kill prints `[FAIL] <name> signal=<NAME>`.

```c
// xtest/c/syscall/getpid.c
#include "common/assert.h"
#include <unistd.h>
int main(void) {
    pid_t p = getpid();
    xtest_assert(p > 0);
    return 0;
}
```

## Make targets

```
make -C xtest all          ARCH=riscv64|loongarch64
make -C xtest build-c      ARCH=...
make -C xtest build-iozone ARCH=...
make -C xtest stage        ARCH=...
make -C xtest bake-image   ARCH=...
make -C xtest clean        ARCH=...
make -C xtest docker-shell                      # interactive shell in the contest image
```

## iozone

The vendored iozone benchmark lives under `xtest/iozone/` (see its README for
provenance and license). `build-iozone.sh` cross-compiles it to a static-PIE
musl ELF; `run-iozone.sh` runs a bounded smoke pass in the guest and reports
`[PASS]/[FAIL] iozone`. Building it needs the contest Docker image — a stock
host musl toolchain whose `libc.a` is not PIC-capable cannot link `-static-pie`.

## Docker image

Pinned by digest:

```
docker.educg.net/cg/os-contest@sha256:742479b5cd11b24501e2eccf5d409b78b76ba7aabcb87f815bbd5908a288313b
```

Cross compilers used:

- `riscv64`: `/opt/riscv64-linux-musl-cross/bin/riscv64-linux-musl-gcc`
- `loongarch64`: `/opt/loongarch64-linux-musl-cross/bin/loongarch64-linux-musl-gcc`

Image is `linux/amd64` — on Apple Silicon hosts Docker emulates with
qemu-x86_64 (slow but correct). Native amd64 hosts (CI) run at full speed.
