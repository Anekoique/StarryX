
[**Goals**]

- G-1: `xtest/` becomes the in-repo test environment producer; layout is `xtest/{c,scripts}` (no other top-level dirs). _[Iter-3 scope reduction: `testsuites/` half withdrawn at user direction — see VERIFY.md V-001.]_
- G-2: Every removable piece of the old sdcard pipeline (`xtest/Makefile` (old), `xtest/Makefile.sub`, `xtest/config/`, `xtest/scripts/git_testcode.sh`) is deleted in this task; nothing in the repo references them after the change.
- G-3: First-party C tests live under `xtest/c/` as one `.c` per test; each compiles to one statically-linked ELF using the cross toolchain identified by the Phase 0 spike (`MUSL_CC_RV64` / `MUSL_CC_LA64`). A `xtest/scripts/run-c.sh` driver iterates them and reports pass/fail.
- G-4: ~~Vendored OS-COMP testsuites~~ **WITHDRAWN** — at user direction, the `testsuites/` half of the design is deferred to a follow-up task. See VERIFY.md V-001 for evidence (all 11 suites were end-to-end-built and exercised before withdrawal) and V-002 for per-suite cross-musl patch knowledge captured for the follow-up.
- G-5: `src/test.sh` is a POSIX `sh` script that lives in `src/` next to `src/init.sh`. It is **embedded into the kernel binary** (not into the rootfs) when the kernel is built with the `init-test` cargo feature. Inside the booted kernel it sets `PATH`/`LD_LIBRARY_PATH`/`HOME`, `cd`s into `/root/tests`, runs `./scripts/run-all.sh`, then `exec sh`s for an interactive prompt.
- G-6: `make tests ARCH={riscv64|loongarch64}` produces `tests-rootfs-$ARCH.img` deterministically with respect to the *staged tree* given the pinned toolchain (image bytes may differ due to ext4 timestamps; per-binary determinism rides on the Docker image digest pin in C-14).
- G-7: `make run-tests ARCH=...` (a) builds the kernel with `ROOT_FEATURES=init-test` (which threads `--features init-test` into the cargo invocation), (b) builds the test rootfs image, (c) boots StarryX in QEMU with `tests-rootfs-$ARCH.img` mounted as the root disk via the shared `run_qemu_with_disk` macro.
- G-8: `make run` and `src/init.sh` remain byte-for-byte unchanged after the task lands. The kernel ELF produced by `make run` (no `init-test` feature) is verified to embed exactly `src/init.sh` and nothing else (V-IT-6a + V-IT-8). `src/init.sh` gains a single comment line (`# id: starry-init`) — see G-12 for the symmetric note about why this is still G-8-compatible.
- G-9: On both `riscv64` and `loongarch64`, end-to-end smoke succeeds: the test rootfs boots under the test-built kernel, `run-all.sh` executes all first-party C tests, and per-test pass/fail lines appear on the serial console. A test failing does **not** abort the run (C-8b). _[Iter-3: the `basic` suite reference is dropped per G-4 withdrawal.]_
- G-10: Documentation: `xtest/README.md` describes how to run the pipeline and how to add tests; `AGENTS.md` "Testing" section is updated to reference `make tests` / `make run-tests` and Docker dependency.
- G-11: Build switch — the kernel embeds `src/init.sh` by default; it embeds `src/test.sh` instead when built with the `init-test` cargo feature. `make run` does not enable the feature; `make tests` / `make run-tests` set `ROOT_FEATURES := init-test` which threads `--features init-test` into the cargo invocation.
- G-12: Init-script identity — both `src/init.sh` and `src/test.sh` carry an `# id: starry-init` / `# id: starry-test` marker comment near the top so V-UT-8 / V-IT-8 / Phase 1 acceptance can mechanically verify the kernel ELF embedded the right script. **G-8's "byte-for-byte unchanged" applies post-marker addition**: the marker is the only one-line edit to `src/init.sh` allowed by this task; once added, no further changes to that file are permitted by the task scope. (See V-IT-6a's revised wording.)

- NG-1: Not building or modifying cross toolchains. The Phase 0 spike *records* what the contest Docker image already provides; it does not add tools.
- NG-2: Not rebuilding the Alpine rootfs from scratch. We bake on top of `Starry-OS/rootfs/rootfs-$ARCH.img` (per TR-2).
- NG-3: Not maintaining glibc-side test variants. Alpine is musl-only; `xtest/c/` and the vendored suites target musl.
- NG-4: Not introducing tier groupings (preliminary / final1 / final2). `run-all.sh` runs everything sequentially in a stable order.
- NG-5: Not changing how the kernel is built beyond the single new `init-test` cargo feature plus its minimal Make seam (one new top-level variable `ROOT_FEATURES` and one extension to `cargo.mk`'s `cargo_build` macro). `scripts/make/build.mk`, target features, axconfig flow are untouched.
- NG-6: Not removing or repurposing `qemu_rootfs` / the upstream rootfs download. `make run` keeps using it.
- NG-7: Not adding host-OS support beyond what the contest Docker image enables (Linux + Docker). macOS hosts go through Docker on macOS (Docker Desktop / Colima both work). User confirmed this trade-off in design discussion.
- NG-8: Not reinstating any OS-COMP scoring markers. The runtime emits a plain `[PASS]/[FAIL]` format only.

[**Architecture**]

```
HOST (Linux + Docker, or any system that can run the contest Docker image)
┌──────────────────────────────────────────────────────────────────┐
│ make tests ARCH=riscv64                                           │
│   └─ docker run … docker.educg.net/cg/os-contest@sha256:<digest> \│
│        make -C xtest build-all ARCH=riscv64                       │
│         ├─ build-c        → xtest/build/<arch>/c/<name>           │
│         ├─ build-suites   → xtest/build/<arch>/testsuites/<s>/…   │
│         └─ stage          → xtest/build/<arch>/stage/root/tests/  │
│      then bake-image:                                             │
│        cp rootfs-<arch>.img tests-rootfs-<arch>.img               │
│        mount tests-rootfs-<arch>.img → mnt/                       │
│        rsync stage/root/tests/* into mnt/root/tests/              │
│        umount, release                                            │
│                                                                    │
│ make run-tests ARCH=riscv64                                       │
│   ├─ make tests                                                   │
│   ├─ make build ARCH=riscv64 ROOT_FEATURES=init-test              │
│   │     └─ cargo.mk's cargo_build appends --features init-test    │
│   │        src/main.rs's #[cfg(feature="init-test")] arm picked   │
│   │        → include_str!("test.sh") embeds src/test.sh in kernel │
│   └─ $(call run_qemu_with_disk,$(TESTS_ROOTFS_IMG))               │
└──────────────────────────────────────────────────────────────────┘
                              │
                              ▼
GUEST (StarryX kernel that embeds src/test.sh; root disk = tests-rootfs)
┌──────────────────────────────────────────────────────────────────┐
│ kernel main() → run_user_app(["/bin/busybox","sh","-c",embedded_test_sh])
│   └─ embedded_test_sh:                                            │
│        export PATH / LD_LIBRARY_PATH / HOME                       │
│        cd /root/tests                                             │
│        ./scripts/run-all.sh                                       │
│          ├─ ./scripts/run-c.sh           (first-party C tests)    │
│          └─ for s in testsuites/*; do                             │
│               ./scripts/run-suite.sh "$s"                         │
│             done   (per-test pass/fail; plain group headers)      │
│        exec sh   (post-test interactive shell)                    │
└──────────────────────────────────────────────────────────────────┘
```

Module decoupling:
- **Build side** (`xtest/Makefile` + `xtest/scripts/build/*.sh`) only knows about cross-compilation and image baking. It does not embed or know about the kernel boot script.
- **Kernel-embed side** (`src/test.sh` + `src/main.rs`'s two `#[cfg(feature="init-test")]` arms) only knows how to dispatch to the in-image test entry point. It does not know how the rootfs was built.
- **Runtime side** (`xtest/scripts/run-*.sh`) only knows how to discover and execute test binaries and emit a stable `[PASS]/[FAIL]` plain-text format with per-suite group headers. It does not know how anything was built.
- The bridge between build side and runtime side is the **staging contract**: `xtest/build/<arch>/stage/root/tests/{c,testsuites,scripts}` has a fixed shape; `bake-image` copies it verbatim to `/root/tests` in the image. The bridge between build side and kernel-embed side is the **`init-test` feature contract** carried by the new `ROOT_FEATURES` Make variable: `make tests` / `make run-tests` set `ROOT_FEATURES := init-test` for the kernel build step; everything else leaves it unset.

[**Build Switch**]

`src/main.rs` line 64 changes from:

```rust
let init = include_str!("init.sh");
```

to:

```rust
#[cfg(feature = "init-test")]
let init = include_str!("test.sh");
#[cfg(not(feature = "init-test"))]
let init = include_str!("init.sh");
```

Both `include_str!` arms take a string-literal token, which is the only form the macro accepts.

The root crate's `Cargo.toml` gains:

```toml
[features]
init-test = []
```

The Make-side passthrough is via a new top-level `ROOT_FEATURES` variable threaded into `scripts/make/cargo.mk`'s `cargo_build` macro (see API Surface and Phase 1). `make run` / `make rv` / `make la` do not set `ROOT_FEATURES`. `make tests` / `make run-tests` set `ROOT_FEATURES := init-test`.

[**Data Structure**]

```
xtest/
├── Makefile                       # build pipeline (top of xtest)
├── README.md                      # what this is, how to run it
├── .gitignore                     # ignores build/
├── c/
│   ├── README.md
│   ├── common/
│   │   ├── assert.h               # tiny xtest_assert / xtest_eq macros
│   │   └── fork_helper.h          # optional shared helpers
│   ├── syscall/
│   │   ├── getpid.c
│   │   ├── clone_basic.c
│   │   └── ...
│   ├── signal/
│   │   ├── kill_self.c
│   │   └── ...
│   ├── mm/
│   │   └── mmap_anon.c
│   └── fs/
│       └── open_close.c
├── testsuites/
│   ├── UPSTREAM.md                # URL + commit + import date + per-suite SPDX + patches
│   ├── basic/                     # vendored from oscomp pre-2025 (preserves LICENSE/COPYING)
│   ├── busybox/
│   ├── libctest/
│   ├── libcbench/
│   ├── lua/
│   ├── iozone/
│   ├── iperf/
│   ├── netperf/
│   ├── cyclictest/
│   ├── lmbench/
│   ├── ltp/
│   ├── splice/
│   ├── copy-file-range/
│   └── interrupts/
├── scripts/
│   ├── build/
│   │   ├── build-c.sh             # in-Docker: compile xtest/c/**/*.c with $MUSL_CC_<ARCH>
│   │   ├── build-suites.sh        # in-Docker: build each suite (warns if both Makefile + BUILD.sh)
│   │   ├── stage.sh               # assemble xtest/build/<arch>/stage/
│   │   └── bake-image.sh          # cp rootfs, mount, rsync stage, umount
│   ├── run-all.sh                 # in-guest: drive the whole run
│   ├── run-c.sh                   # in-guest: iterate /root/tests/c/
│   ├── run-suite.sh               # in-guest: drive one suite dir
│   └── lib/
│       └── timeout.sh             # bounded execution helper
└── build/                         # gitignored; per-arch build + stage + image
    └── <arch>/
        ├── c/                     # compiled ELFs
        ├── testsuites/            # built suite outputs
        ├── stage/root/tests/      # final tree copied into the image
        └── tests-rootfs-<arch>.img   # (exposed as TESTS_ROOTFS_IMG; see API Surface)

src/
├── init.sh        # one-line edit: adds `# id: starry-init` near the top (G-12)
└── test.sh        # NEW — kernel-embedded boot script for make tests; carries `# id: starry-test`
```

Per-suite recipe contract: every suite under `xtest/testsuites/<s>/` is expected to expose either a `Makefile` (preferred) or a `BUILD.sh` script. Either must drop its outputs under `xtest/build/<arch>/testsuites/<s>/`. If both are present, `build-suites.sh` warns and prefers `Makefile`. Script-only suites with neither are copied verbatim.

[**API Surface**]

Top-level `Makefile` — new public targets:

```
make tests           ARCH=riscv64|loongarch64    # build tests-rootfs-$ARCH.img
make run-tests       ARCH=riscv64|loongarch64    # build kernel (with feature) + image + boot
```

New top-level Make variables:

```
TESTS_ROOTFS_IMG := $(ROOT_DIR)/xtest/build/$(ARCH)/tests-rootfs-$(ARCH).img
ROOT_FEATURES    ?=                              # space-separated root-crate cargo features
                                                 # (NOT axfeat/-prefixed; threaded straight to cargo)
                                                 # set to `init-test` by make tests/make run-tests
```

`scripts/make/cargo.mk` change — `cargo_build` macro extension:

```make
# Before (verbatim from current cargo.mk line 23-25):
define cargo_build
  $(call run_cmd,cargo build,$(build_args) --manifest-path "$(1)/Cargo.toml" --features "$(strip $(2))")
endef

# After:
define cargo_build
  $(call run_cmd,cargo build,$(build_args) --manifest-path "$(1)/Cargo.toml" --features "$(strip $(2))" $(if $(strip $(ROOT_FEATURES)),--features "$(strip $(ROOT_FEATURES))",))
endef
```

Cargo features (added to root `Cargo.toml`):

```toml
[features]
init-test = []   # when set, src/main.rs embeds src/test.sh instead of src/init.sh
```

`xtest/Makefile` — public targets:

```
make -C xtest all          ARCH=...   # build-c + build-suites + stage + bake-image
make -C xtest build-c      ARCH=...
make -C xtest build-suites ARCH=...
make -C xtest stage        ARCH=...
make -C xtest bake-image   ARCH=...
make -C xtest clean        ARCH=...
make -C xtest docker-shell             # interactive shell in the contest image
```

`xtest/Makefile` — public variables (filled by Phase 0 spike):

```
DOCKER_IMAGE := docker.educg.net/cg/os-contest@sha256:742479b5cd11b24501e2eccf5d409b78b76ba7aabcb87f815bbd5908a288313b
MUSL_CC_RV64 := /opt/riscv64-linux-musl-cross/bin/riscv64-linux-musl-gcc
MUSL_CC_LA64 := /opt/loongarch64-linux-musl-cross/bin/loongarch64-linux-musl-gcc
```

`xtest/scripts/build/bake-image.sh` contract:

```
Inputs:
  ARCH                         riscv64 | loongarch64
  ROOTFS_IMG                   path to upstream rootfs-$ARCH.img
  STAGE_DIR                    path to xtest/build/<arch>/stage/
  OUT_IMG                      path to write tests-rootfs-$ARCH.img
Outputs:
  $OUT_IMG: copy of $ROOTFS_IMG with $STAGE_DIR/root/tests/* copied to /root/tests.
            No init script is installed into the image — that lives in the kernel ELF.
Side effects:
  Mount/umount under $XTEST_BUILD/<arch>/mnt/ inside the Docker container.
  On failure: trap-based cleanup (umount, losetup -d, rm partial $OUT_IMG).
```

`src/test.sh` contract (POSIX `sh`, embedded into the kernel by `include_str!`):

```
- carries `# id: starry-test` marker near the top (per G-12 / C-16)
- exports PATH=/bin:/sbin:/usr/bin:/usr/sbin
- exports LD_LIBRARY_PATH=/lib:/usr/lib
- exports HOME=/root
- cd /root/tests
- runs ./scripts/run-all.sh; never aborts on suite failure
- on completion, exec sh
```

`src/init.sh` contract addition (the only edit to this file in the task):

```
- carries `# id: starry-init` marker near the top (per G-12 / C-16)
- everything else byte-identical to current src/init.sh
```

`xtest/scripts/run-all.sh` contract:

```
- prints "==== c ====" then runs ./scripts/run-c.sh then prints "==== c done ===="
- for each subdirectory of testsuites/:
    - prints "==== <suite> ===="
    - runs ./scripts/run-suite.sh <suite>
    - prints "==== <suite> done ===="
- per-test results in `[PASS] <name>` / `[FAIL] <name> exit=<n>` / `[TIMEOUT] <name>` form
- exits 0 unconditionally (per C-8b)
```

`scripts/make/qemu.mk` macro contract (added per C-15):

```
run_qemu_with_disk = $(QEMU) … -drive file=$(1),... <rest of qemu_args-y unchanged>
run_qemu          = $(call run_qemu_with_disk,$(DISK_IMG))
run_qemu_tests    = $(call run_qemu_with_disk,$(TESTS_ROOTFS_IMG))
```

[**Constraints**]

- C-1: All cross-compilation runs **inside** the contest Docker image (pinned by digest per C-14). `xtest/Makefile` checks `command -v docker` and fails fast with a clear error citing the contest image URL when Docker is missing.
- C-2: `xtest/build/` is gitignored. No build outputs are committed.
- C-3: `xtest/testsuites/` is **vendored**: upstream sources committed verbatim. Each suite preserves its upstream `LICENSE`/`COPYING`/`NOTICE` files (per C-13). `xtest/testsuites/UPSTREAM.md` records URL + commit + import date + per-suite SPDX identifier + per-suite local-patches summary.
- C-4: No top-level `Makefile` variable rename or removal. New variables follow existing naming style (`TESTS_ROOTFS_IMG` mirrors `ROOTFS_IMG`; `ROOT_FEATURES` mirrors `FEATURES`).
- C-5: `make tests` / `make run-tests` accept the same `ARCH` / `BLK` / `NET` / `MEM` / `LOG` overrides as `make rv` / `make la`; their internals reuse `scripts/make/qemu.mk` via the shared `run_qemu_with_disk` macro (C-15).
- C-6: `src/test.sh` and every `xtest/scripts/**/*.sh` are POSIX `sh` (Alpine ash-compatible). Lints clean under `dash -n`.
- C-7: `bake-image.sh` requires `--privileged` for loop-mount, matching the existing `xtest/Makefile`'s `docker` target.
- C-8a: **Build-time** — any single-test compile error or single-suite build error is recorded and exits non-zero so the image is **not** baked; but `build-c.sh` and `build-suites.sh` attempt every input before exiting so contributors see all failures at once.
- C-8b: **Run-time** — a failing/crashing/timing-out test is logged and the run continues. `run-*.sh` always exit 0 so `/test.sh` always reaches `exec sh`.
- C-9: `xtest/c/` test names are unique across subdirectories (we flatten to `c/<name>` in the staged tree, so two `mmap.c` in different subdirs are a build error caught by `build-c.sh`).
- C-10: First-party C tests link statically against musl using the cross compiler identified by the Phase 0 spike. `build-c.sh` invokes `$(MUSL_CC_<ARCH>) -static -I xtest/c/common -O2`. They must run on Alpine without LD shims or `/lib/ld-musl-*` patching.
- C-12: Build switch — the root crate exposes an `init-test` cargo feature. `src/main.rs` has two `#[cfg(feature = "init-test")]`-gated `include_str!` arms; the `init-test` arm embeds `src/test.sh`, the `not(...)` arm embeds `src/init.sh`. The Make-side seam is the new top-level `ROOT_FEATURES` variable threaded into `scripts/make/cargo.mk`'s `cargo_build` macro (extended to append `--features "$(ROOT_FEATURES)"` when non-empty). `make run` / `make rv` / `make la` do not set `ROOT_FEATURES`; `make tests` / `make run-tests` set `ROOT_FEATURES := init-test`. The existing `FEATURES` / `AX_FEAT` plumbing is untouched.
- C-13: Per-suite license preservation — every `xtest/testsuites/<s>/` subtree contains the upstream's license / copyright file(s) verbatim. `UPSTREAM.md` records each suite's SPDX identifier (e.g. `GPL-2.0-only` for LTP, `BSD-3-Clause` for iperf, ...).
- C-14: Docker image pin — `xtest/Makefile`'s `DOCKER_IMAGE` references the contest image by digest (`@sha256:...`), not by mutable tag. Digest captured at Phase 0 and recorded in `xtest/Makefile` and `xtest/README.md`.
- C-15: qemu.mk shared macro — `scripts/make/qemu.mk` factors the QEMU invocation through a `run_qemu_with_disk` macro that takes the disk image path as `$(1)`. `run_qemu` and `run_qemu_tests` are both one-line callers. Every other QEMU flag stays in the macro body so the two paths cannot drift.
- C-16: Init-script ID markers — `src/init.sh` contains a `# id: starry-init` comment near the top; `src/test.sh` contains `# id: starry-test`. The markers are unique per script, never appear in the other script, and are preserved verbatim in the kernel ELF (since they're inside the `include_str!`-embedded string). V-UT-8 / V-IT-8 / Phase 1 acceptance use these markers to verify the build switch selected the right script.

---
