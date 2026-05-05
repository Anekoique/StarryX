# `xtest` PLAN `02`

> Status: Revised
> Feature: `xtest`
> Iteration: `02`
> Owner: Executor
> Depends on:
> - Previous Plan: `01_PLAN.md`
> - Review: `01_REVIEW.md`
> - Master Directive: `none`

---

## Summary

Iteration 02 commits unambiguously to the **cargo-feature** form of the kernel build switch. PLAN 01's `option_env!` + `const &str` + `include_str!(CONST)` form does not compile: `include_str!` is a built-in macro that requires a string-literal token at expansion time and rejects named bindings — this is a long-standing Rust constraint, not a toolchain quirk. The whole PLAN is therefore re-stated with the feature-flag form as the only mechanism: `src/main.rs` becomes two `#[cfg(feature = "init-test")]`-gated `include_str!` arms; `make tests` / `make run-tests` add `--features init-test` to the cargo invocation; `make run` does not. `AX_INIT_SCRIPT` is gone from the design.

Six minor cleanups also land: dead `C-11` reservation removed, Phase-0 mid-PLAN edit mechanism specified, G-10 gets its own V-* entry, V-IT-7's "test-only flags" hedge dropped, C-14 gets a runnable check, NG-7 macOS-Docker note stays explicit (user already confirmed Docker for builds in design discussion).

## Log

[**Added**]
- New V-UT-9 (G-10 documentation grep) and V-UT-10 (C-14 digest pin grep) so every Goal and Constraint maps to a runnable Validation entry.
- Phase 0 acceptance gains an explicit "in-place edit + new `[**Phase 0 Results**]` heading in `## Log`" mechanism so the executor doesn't have to choose between silently rewriting an Approved Spec and opening another iteration just for factual data capture.

[**Changed**]
- `[**Build Switch**]` rewritten: only the `#[cfg(feature = "init-test")]` two-arm form remains. The `option_env!` block is deleted (it does not compile — see Response Matrix R-001).
- G-11 rewritten: "build switch" now means "the `init-test` cargo feature".
- C-12 rewritten to drop the `option_env!` mention; describes only the cargo-feature mechanism.
- Architecture diagram: the line `AX_INIT_SCRIPT=test.sh make build ARCH=...` becomes `cargo build --features init-test`; the `option_env!` annotation becomes `cfg(feature="init-test")`.
- API Surface: removed the `AX_INIT_SCRIPT` Make variable (no longer needed). `make tests`/`make run-tests` add `--features init-test` to the kernel build instead.
- Phase 0: removed the "decide which form … by trying both" sub-bullet. Replaced with a confirmation-only step: "build a one-line `src/main.rs` test that uses the two-arm form; verify `cargo build` and `cargo build --features init-test` both succeed and the embedded line differs."
- Phase 1, Phase 5, Failure Flow item 8, V-UT-8, V-IT-8, V-F-6 all reworded to use the cargo-feature mechanism instead of the env var.
- T-3' "Chosen" line specifies the cargo-feature form explicitly.
- V-IT-7 hedge "(and any test-only flags)" deleted; assertion is now strict equivalence except for the `-drive file=` argument.
- Acceptance Mapping for C-14 now points at V-UT-10 + V-IT-1 (no more "inspection of").
- Acceptance Mapping for G-10 now points at V-UT-9 (no more "Phase 6 deliverables").

[**Removed**]
- `AX_INIT_SCRIPT` env var from the design (PLAN 01's G-11 / C-12 / API Surface / Architecture / Phases / Failure Flow / Validation references all redone).
- C-11 reservation line (workflow §4: Spec must be self-contained; SPEC will not show a phantom constraint).
- The `option_env!` code block from `[**Build Switch**]`.

[**Unresolved**]
- Whether the `os-contest:20250714` image actually contains a loongarch64 musl cross compiler (Phase 0 spike will answer; if missing, Phase 0's three pre-agreed fallbacks per R-003 from REVIEW 00 still apply).

[**Response Matrix**]

| Source | ID | Decision | Resolution |
|--------|----|----------|------------|
| Review 01 | R-001 | Accepted | The `option_env!` form is deleted from `[**Build Switch**]`. C-12 / G-11 / Phase 0 / Failure Flow item 8 / V-UT-8 / V-IT-8 / V-F-6 / Architecture diagram all rewritten to use only the `#[cfg(feature = "init-test")]` two-arm form. T-3' "Chosen" line names the cargo-feature form. The `build.rs`-generated literal alternative is mentioned in T-3' as a future option but not adopted (heavier refactor for no current benefit). |
| Review 01 | R-002 | Accepted | C-11 reservation line deleted. Constraint list now skips from C-10 to C-12 (sparse numbering preserves the Response-Matrix references in R-007 / TR-6 of REVIEW 00 and avoids cascading renumbers). Workflow §4 requires Spec self-containment, not dense numbering. |
| Review 01 | R-003 | Accepted | Phase 0 acceptance bullet gains: "Phase 0 results are appended to this PLAN's `## Log` under a new `[**Phase 0 Results**]` heading; the placeholder strings in `[**API Surface**]` are replaced in place. No new PLAN iteration is required — Phase 0 produces only factual capture, not design change." |
| Review 01 | R-004 | Accepted | New V-UT-9: `grep -F 'make tests' AGENTS.md` and `grep -F 'make run-tests' AGENTS.md` both return non-empty after Phase 6; `xtest/README.md` exists and is non-empty. G-10 row in Acceptance Mapping points at V-UT-9. |
| Review 01 | R-005 | Accepted | V-IT-7 reworded: "differ **only** in the `-drive file=` argument" (the `(and any test-only flags)` parenthetical is removed). C-15 no-drift constraint now bites. |
| Review 01 | R-006 | Accepted | New V-UT-10: `grep -E 'docker\.educg\.net/cg/os-contest@sha256:[a-f0-9]{64}' xtest/Makefile` returns one match; the same regex with `:20250714` (tag form) returns none. C-14 row in Acceptance Mapping points at V-UT-10 + V-IT-1. |
| Review 01 | R-007 | Acknowledged | NG-7 (no macOS-native, Docker required) stays as drafted. The user explicitly confirmed "use docker confirm" in the design discussion; this iteration just records that the constraint is intentional and that Docker-on-macOS works. No spec change. |
| Review 01 | TR-1 | Accepted | T-1 unchanged. |
| Review 01 | TR-2 | Accepted | T-2 unchanged. |
| Review 01 | TR-3 | Accepted | T-3' "Chosen" line rewritten to name the cargo-feature form explicitly. Direction unchanged (build-time switch, Option A); form changed (cargo feature, not env var). |
| Review 01 | TR-4 | Accepted | T-4 unchanged. |
| Review 01 | TR-5 | Accepted | T-5 unchanged. |
| Review 01 | TR-6 | Accepted | T-6 unchanged; V-IT-7 tightened per R-005. |

---

## Spec

[**Goals**]

- G-1: `xtest/` becomes the in-repo test environment producer; layout is `xtest/{c,testsuites,scripts}` (no other top-level dirs).
- G-2: Every removable piece of the old sdcard pipeline (`xtest/Makefile` (old), `xtest/Makefile.sub`, `xtest/config/`, `xtest/scripts/git_testcode.sh`) is deleted in this task; nothing in the repo references them after the change.
- G-3: First-party C tests live under `xtest/c/` as one `.c` per test; each compiles to one statically-linked ELF using the cross toolchain identified by the Phase 0 spike (`MUSL_CC_RV64` / `MUSL_CC_LA64`). A `xtest/scripts/run-c.sh` driver iterates them and reports pass/fail.
- G-4: A vendored subset of `oscomp/testsuites-for-oskernel @ pre-2025` lives under `xtest/testsuites/<suite>/`, one directory per suite. Sources are committed directly (no submodule, no fetch-on-build). Pinned upstream commit, per-suite license SPDX, and per-suite local-patches summary recorded in `xtest/testsuites/UPSTREAM.md`.
- G-5: `src/test.sh` is a POSIX `sh` script that lives in `src/` next to `src/init.sh`. It is **embedded into the kernel binary** (not into the rootfs) when the kernel is built with the `init-test` cargo feature. Inside the booted kernel it sets `PATH`/`LD_LIBRARY_PATH`/`HOME`, `cd`s into `/root/tests`, runs `./scripts/run-all.sh`, then `exec sh`s for an interactive prompt.
- G-6: `make tests ARCH={riscv64|loongarch64}` produces `tests-rootfs-$ARCH.img` deterministically with respect to the *staged tree* given the pinned toolchain (image bytes may differ due to ext4 timestamps; per-binary determinism rides on the Docker image digest pin in C-14).
- G-7: `make run-tests ARCH=...` (a) builds the kernel with `--features init-test`, (b) builds the test rootfs image, (c) boots StarryX in QEMU with `tests-rootfs-$ARCH.img` mounted as the root disk via the shared `run_qemu_with_disk` macro.
- G-8: `make run` and `src/init.sh` remain byte-for-byte unchanged after the task lands. The kernel ELF produced by `make run` (no `init-test` feature) is verified to embed exactly `src/init.sh` and nothing else (V-IT-6a + V-IT-8).
- G-9: On both `riscv64` and `loongarch64`, end-to-end smoke succeeds: the test rootfs boots under the test-built kernel, `run-all.sh` executes the `basic` suite plus all first-party C tests, and per-test pass/fail lines appear on the serial console. A test failing does **not** abort the run (C-8b).
- G-10: Documentation: `xtest/README.md` describes how to run the pipeline and how to add tests; `AGENTS.md` "Testing" section is updated to reference `make tests` / `make run-tests` and Docker dependency.
- G-11: Build switch — the kernel embeds `src/init.sh` by default; it embeds `src/test.sh` instead when built with the `init-test` cargo feature. `make run` does not enable the feature; `make tests` / `make run-tests` do.

- NG-1: Not building or modifying cross toolchains. The Phase 0 spike *records* what the contest Docker image already provides; it does not add tools.
- NG-2: Not rebuilding the Alpine rootfs from scratch. We bake on top of `Starry-OS/rootfs/rootfs-$ARCH.img` (per TR-2).
- NG-3: Not maintaining glibc-side test variants. Alpine is musl-only; `xtest/c/` and the vendored suites target musl.
- NG-4: Not introducing tier groupings (preliminary / final1 / final2). `run-all.sh` runs everything sequentially in a stable order.
- NG-5: Not changing how the kernel is built beyond the single new `init-test` cargo feature on the root crate. `scripts/make/build.mk`, target features, axconfig flow are untouched apart from the feature passthrough.
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
│   ├─ cargo build --features init-test ARCH=riscv64                │
│   │     └─ src/main.rs's #[cfg(feature="init-test")] arm picked   │
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
- The bridge between build side and runtime side is the **staging contract**: `xtest/build/<arch>/stage/root/tests/{c,testsuites,scripts}` has a fixed shape; `bake-image` copies it verbatim to `/root/tests` in the image. The bridge between build side and kernel-embed side is the **`init-test` feature contract**: `make tests` / `make run-tests` always pass `--features init-test` to the kernel cargo build; everything else does not.

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

Both `include_str!` arms take a string-literal token, which is the only form the macro accepts (`include_str!` is a built-in compiler macro whose argument grammar requires a literal at expansion time; named `const &str` bindings are rejected with `error: argument must be a string literal`).

The root crate's `Cargo.toml` gains:

```toml
[features]
init-test = []
```

`make run` / `make rv` / `make la` do not enable the feature. `make tests` / `make run-tests` pass `--features init-test` through to `cargo build` (via the existing `scripts/make/cargo.mk` `features` plumbing — see Implementation Phase 1).

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
        └── tests-rootfs-<arch>.img

src/
├── init.sh        # unchanged (byte-identical, embedded by default)
└── test.sh        # NEW — kernel-embedded boot script for make tests
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
TESTS_ROOTFS_IMG  := $(ROOT_DIR)/xtest/build/$(ARCH)/tests-rootfs-$(ARCH).img
```

Cargo features (added to root `Cargo.toml`):

```
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
DOCKER_IMAGE := docker.educg.net/cg/os-contest@sha256:<digest-from-Phase-0>
MUSL_CC_RV64 := <full path to riscv64 musl gcc, captured by Phase 0 spike>
MUSL_CC_LA64 := <full path to loongarch64 musl gcc, captured by Phase 0 spike>
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
- exports PATH=/bin:/sbin:/usr/bin:/usr/sbin
- exports LD_LIBRARY_PATH=/lib:/usr/lib
- exports HOME=/root
- cd /root/tests
- runs ./scripts/run-all.sh; never aborts on suite failure
- on completion, exec sh
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
- C-4: No top-level `Makefile` variable rename or removal. New variables follow existing naming style (`TESTS_ROOTFS_IMG` mirrors `ROOTFS_IMG`).
- C-5: `make tests` / `make run-tests` accept the same `ARCH` / `BLK` / `NET` / `MEM` / `LOG` overrides as `make rv` / `make la`; their internals reuse `scripts/make/qemu.mk` via the shared `run_qemu_with_disk` macro (C-15).
- C-6: `src/test.sh` and every `xtest/scripts/**/*.sh` are POSIX `sh` (Alpine ash-compatible). Lints clean under `dash -n`.
- C-7: `bake-image.sh` requires `--privileged` for loop-mount, matching the existing `xtest/Makefile`'s `docker` target.
- C-8a: **Build-time** — any single-test compile error or single-suite build error is recorded and exits non-zero so the image is **not** baked; but `build-c.sh` and `build-suites.sh` attempt every input before exiting so contributors see all failures at once.
- C-8b: **Run-time** — a failing/crashing/timing-out test is logged and the run continues. `run-*.sh` always exit 0 so `/test.sh` always reaches `exec sh`.
- C-9: `xtest/c/` test names are unique across subdirectories (we flatten to `c/<name>` in the staged tree, so two `mmap.c` in different subdirs are a build error caught by `build-c.sh`).
- C-10: First-party C tests link statically against musl using the cross compiler identified by the Phase 0 spike. `build-c.sh` invokes `$(MUSL_CC_<ARCH>) -static -I xtest/c/common -O2`. They must run on Alpine without LD shims or `/lib/ld-musl-*` patching.
- C-12: Build switch — the root crate exposes an `init-test` cargo feature. `src/main.rs` has two `#[cfg(feature = "init-test")]`-gated `include_str!` arms; the `init-test` arm embeds `src/test.sh`, the `not(...)` arm embeds `src/init.sh`. `make run` / `make rv` / `make la` do not pass the feature; `make tests` / `make run-tests` pass `--features init-test` to the kernel cargo build.
- C-13: Per-suite license preservation — every `xtest/testsuites/<s>/` subtree contains the upstream's license / copyright file(s) verbatim. `UPSTREAM.md` records each suite's SPDX identifier (e.g. `GPL-2.0-only` for LTP, `BSD-3-Clause` for iperf, ...).
- C-14: Docker image pin — `xtest/Makefile`'s `DOCKER_IMAGE` references the contest image by digest (`@sha256:...`), not by mutable tag. Digest captured at Phase 0 and recorded in `xtest/Makefile` and `xtest/README.md`.
- C-15: qemu.mk shared macro — `scripts/make/qemu.mk` factors the QEMU invocation through a `run_qemu_with_disk` macro that takes the disk image path as `$(1)`. `run_qemu` and `run_qemu_tests` are both one-line callers. Every other QEMU flag stays in the macro body so the two paths cannot drift.

---

## Runtime

[**Main Flow**] — `make run-tests ARCH=riscv64`

1. Top-level `Makefile`'s `run-tests` target depends on `tests` and on a `build` invocation that adds `--features init-test` to the cargo flags.
2. `tests` target: ensure upstream `rootfs-$ARCH.img` exists (delegate to existing `qemu_rootfs` logic factored into a shared make function), then `docker run … make -C xtest all ARCH=$(ARCH)`.
3. Inside Docker, `xtest/Makefile`'s `all` runs `build-c → build-suites → stage → bake-image`:
   a. `build-c.sh` finds every `xtest/c/**/*.c`, compiles each with `$(MUSL_CC_<ARCH>) -static -I xtest/c/common -O2 -o xtest/build/<arch>/c/<basename>` (per C-10).
   b. `build-suites.sh` iterates `xtest/testsuites/*/`; for each suite, dispatches per the per-suite recipe contract.
   c. `stage.sh` assembles `xtest/build/<arch>/stage/root/tests/{c,testsuites,scripts}` from the build outputs plus `xtest/scripts/run-*.sh` and `xtest/scripts/lib/`.
   d. `bake-image.sh` copies `rootfs-$ARCH.img` to `tests-rootfs-$ARCH.img`, loop-mounts it, `rsync -a` the staged tree into `/root/tests`, `umount`s, releases the loop device. **No init script is installed.**
4. Back on the host, `run-tests` invokes the kernel build with `--features init-test`. `src/main.rs`'s `#[cfg(feature = "init-test")]` arm activates, so `include_str!("test.sh")` embeds `src/test.sh` into the kernel ELF.
5. `run-tests` invokes `$(call run_qemu_with_disk,$(TESTS_ROOTFS_IMG))` from the new shared qemu macro.
6. StarryX boots. `main()` runs `run_user_app(["/bin/busybox","sh","-c", <embedded src/test.sh>])`.
7. The embedded `test.sh` exports env, `cd /root/tests`, runs `./scripts/run-all.sh`, then `exec sh`.
8. `run-all.sh` runs `run-c.sh` (iterates `c/*` ELFs, prints `[PASS] <name>` / `[FAIL] <name> exit=<n>`), then for each suite dir prints a plain group header / footer around `run-suite.sh <suite>` (which dispatches into the suite's own `run.sh`).
9. After all suites, `/test.sh` `exec sh`s into an interactive shell.

[**Failure Flow**]

1. **Docker not installed:** `xtest/Makefile`'s top guard (per C-1) prints "docker not found — install Docker and pull <image-url>" and exits non-zero. `make tests` fails fast with no half-built artifacts.
2. **Cross compile fails for a single C test:** `build-c.sh` collects the error, continues to the next file, and exits non-zero overall (per C-8a). Build aborts before image baking.
3. **Suite build fails:** `build-suites.sh` records the failing suite, prints its error, continues to the next suite, exits non-zero (per C-8a). Image is not baked.
4. **Image baking fails (mount, rsync, umount):** `bake-image.sh` traps and `umount`s, releases the loop device, deletes the partial output image, exits non-zero.
5. **A test binary crashes / segfaults at runtime:** `run-c.sh` / `run-suite.sh` capture exit status, print `[FAIL] <name> exit=<n>` (or signal name), continue (per C-8b). The full run completes.
6. **A suite hangs:** `lib/timeout.sh` wraps invocations with a per-suite timeout (default 600s, overridable by suite-local `TIMEOUT` env). On timeout, `[TIMEOUT] <name>` is printed; execution continues (per C-8b).
7. **`run-all.sh` itself errors:** `/test.sh` does not `set -e`; falls through to `exec sh` so the user retains an interactive prompt.
8. **`src/test.sh` is missing when `--features init-test` is set:** `cargo build --features init-test` fails at compile time because `include_str!("test.sh")` cannot resolve. The error names the missing path.

[**State Transitions**]

- `xtest/build/<arch>/` empty → populated, by `build-c` + `build-suites`.
- Build outputs → `stage/`, by `stage.sh` (rsync-style copy with stable file modes).
- `stage/` + `rootfs-$ARCH.img` → `tests-rootfs-$ARCH.img`, by `bake-image.sh`.
- Idle disk → mounted under `mnt/` → unmounted, inside `bake-image.sh` (always restored on failure via trap).
- Kernel ELF (default) → kernel ELF (test-build), by adding `--features init-test` to the cargo invocation.
- Old `xtest/Makefile`, `Makefile.sub`, `config/`, `scripts/git_testcode.sh` exist → deleted, by Phase 1.

---

## Implementation

[**Phase 0 — Toolchain Spike (single throwaway Docker session, no commits)**]

- Run `docker run --rm -it docker.educg.net/cg/os-contest:20250714 bash`; inside:
  - `find / -name 'gcc' 2>/dev/null` and filter to musl-targeted entries.
  - `find / -name 'libc.a' 2>/dev/null` for both arches.
  - From the host: `docker inspect --format '{{index .RepoDigests 0}}' docker.educg.net/cg/os-contest:20250714` to capture the digest.
- Confirm the cargo-feature build switch compiles: in a scratch checkout, edit `src/main.rs` to the two-arm `#[cfg(feature = "init-test")]` form, add the feature to root `Cargo.toml`, and run both `cargo build` and `cargo build --features init-test`. Both must succeed; the embedded init-script string in the resulting ELFs must differ (verified with `strings`).
- Record results into the PLAN's `[**API Surface**]` (`MUSL_CC_RV64`, `MUSL_CC_LA64`, `DOCKER_IMAGE` digest). **Mechanism:** Phase 0 results are appended to this PLAN's `## Log` under a new `[**Phase 0 Results**]` heading; the placeholder strings in `[**API Surface**]` are replaced in place. No new PLAN iteration is required — Phase 0 produces only factual capture, not design change.
- If loongarch64 musl cross is missing: pick one of (a) install during Phase 2, (b) accept libgcc on Alpine via the existing `Makefile.sub` ld-musl symlink trick, (c) drop loongarch C tests from G-3/G-9. Record the choice in the same `[**Phase 0 Results**]` heading.

Acceptance for Phase 0: `[**API Surface**]` has concrete strings for `MUSL_CC_RV64`, `MUSL_CC_LA64`, `DOCKER_IMAGE` digest; `[**Phase 0 Results**]` exists in `## Log`. Cargo-feature switch confirmed compileable. No code committed yet.

[**Phase 1 — demolition + skeleton (single commit)**]

- Delete `xtest/Makefile` (old), `xtest/Makefile.sub`, `xtest/config/`, `xtest/scripts/git_testcode.sh`.
- Verify with `git grep`: nothing in the repo references `Makefile.sub`, `busybox-config-`, `git_testcode`, `sdcard-rv.img`, or `sdcard-la.img`.
- Create new `xtest/` skeleton: empty `c/`, `testsuites/`, `scripts/{build,lib}/`, plus `xtest/Makefile`, `xtest/README.md`, `xtest/.gitignore` (ignores `build/`), `xtest/testsuites/UPSTREAM.md` (placeholder pending Phase 3).
- Add `src/test.sh` (placeholder that just `exec sh`s — full logic lands in Phase 4).
- Add the `init-test` cargo feature to root `Cargo.toml` and the two `#[cfg(feature="init-test")]` arms in `src/main.rs` per `[**Build Switch**]`. `make run` still embeds `init.sh` because the feature is off.
- Confirm the feature passthrough in `scripts/make/cargo.mk` / `scripts/make/build.mk`: `--features init-test` reaches `cargo build` cleanly when added to the relevant Make variable. (No structural change to those files — they already accept feature lists; we just need the test path to add `init-test` to the existing list.)
- Add top-level `make tests` / `make run-tests` targets (no-ops that print "not yet implemented" and exit 0; wired up properly in Phase 5).
- Top-level `Makefile` `.PHONY` line includes `tests run-tests`.

Acceptance for Phase 1: `git grep -E 'Makefile\.sub|busybox-config-|git_testcode|sdcard-rv|sdcard-la'` returns no matches; `make tests` and `make run-tests` resolve as targets (`make -n tests` succeeds); `make build ARCH=riscv64` (no feature) produces a kernel ELF whose embedded init-script string equals `src/init.sh`'s contents (V-IT-8 dry run); `make build ARCH=riscv64 FEATURES=init-test` (or whatever the project's existing feature flag passthrough syntax is) produces a kernel ELF whose embedded string equals `src/test.sh`'s.

[**Phase 2 — first-party C tests + build pipeline (host + Docker)**]

- Implement `xtest/scripts/build/build-c.sh` — finds all `.c` under `xtest/c/`, checks for duplicate basenames (errors per C-9), compiles each statically with `$(MUSL_CC_<ARCH>)` (per C-10), drops ELFs in `xtest/build/<arch>/c/`. Uses C-8a fail-collect-then-exit.
- Implement `xtest/scripts/build/stage.sh` and a partial `xtest/Makefile` (`build-c`, `stage`, `clean` targets only; `docker-shell` for debugging; Docker `command -v` guard per C-1; Docker image pinned by digest per C-14).
- Add 3–5 first-party C tests under `xtest/c/syscall/` (`getpid.c`, `clone_basic.c`, `mmap_anon.c`, `open_close.c`, `kill_self.c`) plus `common/assert.h`.
- Implement `xtest/scripts/run-c.sh` that iterates `/root/tests/c/*` ELFs and prints PASS/FAIL.

Acceptance for Phase 2: `make -C xtest build-c stage ARCH=riscv64` and `ARCH=loongarch64` both succeed inside Docker; `xtest/build/<arch>/stage/root/tests/c/` contains the ELFs (each verified `file` says `statically linked` and no `interpreter` is present).

[**Phase 3 — vendor upstream test suites**]

- Clone `https://github.com/oscomp/testsuites-for-oskernel` at the `pre-2025` branch tip; identify the suite subdirectories listed under `xtest/testsuites/` in the layout above; copy each into `xtest/testsuites/<suite>/`. Strip `.git`. **Preserve every upstream `LICENSE`/`COPYING`/`NOTICE` file verbatim** (per C-13).
- Fill `xtest/testsuites/UPSTREAM.md`: source URL, pinned commit, import date, per-suite SPDX identifier, per-suite local-patches summary.
- Implement `xtest/scripts/build/build-suites.sh` (per-suite Makefile/BUILD.sh dispatch with the "warn-if-both" rule per TR-4; copy-only fallback for script-only suites; C-8a fail-collect-then-exit).
- Implement `xtest/scripts/run-suite.sh` and `xtest/scripts/lib/timeout.sh`.
- Wire `xtest/Makefile`'s `build-suites` and `all` targets.

Acceptance for Phase 3: `make -C xtest all ARCH=riscv64` (and la) build inside Docker without errors and stage every suite plus all C tests. `git status` shows no build artifacts. Every `xtest/testsuites/<s>/` contains at least one of `{LICENSE, COPYING, COPYING.LIB, NOTICE}`.

[**Phase 4 — image baking + `src/test.sh`**]

- Implement `xtest/scripts/build/bake-image.sh` (cp rootfs, mount, rsync, umount, with trap-based cleanup; per the contract — no `TEST_SH` input).
- Wire `xtest/Makefile`'s `bake-image` target.
- Replace the placeholder `src/test.sh` with the real one (env exports, run-all dispatch, exec sh fallback). Lint with `dash -n`.

Acceptance for Phase 4: `make -C xtest bake-image ARCH=riscv64` produces `xtest/build/riscv64/tests-rootfs-riscv64.img`. Loop-mounting it manually shows `/root/tests/{c,testsuites,scripts}` populated and **no** `/test.sh` file at the root (per the contract).

[**Phase 5 — top-level wiring + qemu.mk refactor + boot smoke**]

- Refactor `scripts/make/qemu.mk`: extract the QEMU invocation into `run_qemu_with_disk = ... $(1) ...`. `run_qemu` becomes `$(call run_qemu_with_disk,$(DISK_IMG))`. Add `run_qemu_tests = $(call run_qemu_with_disk,$(TESTS_ROOTFS_IMG))`. Confirm `make -n run` is byte-identical (modulo whitespace) before and after the refactor.
- Promote `make tests` from Phase 1 placeholder to a real target that delegates to `xtest/Makefile`.
- Promote `make run-tests` from placeholder to: build-test-image + cargo-build-with-feature + `$(call run_qemu_tests)`.
- Factor the upstream-rootfs-fetch logic out of `qemu_rootfs` into a reusable make function so both `qemu_rootfs` (existing) and `tests` (new) call it.
- Boot smoke on `riscv64` and `loongarch64` (`make run-tests ARCH=...`); confirm the embedded `test.sh` runs, the C tests print PASS/FAIL, the `basic` suite prints its group header/footer with per-test results, and the run lands in an interactive `sh` afterwards.

Acceptance for Phase 5: G-7 + G-9 satisfied; both arches boot the test rootfs under the test-built kernel, run the suites, never abort, and drop to a shell. `make -n run` is byte-identical to its pre-refactor output. `make run` still works and its kernel ELF still embeds `src/init.sh` exactly (V-IT-8).

[**Phase 6 — documentation + verify pass**]

- Write `xtest/README.md` (what xtest is, how to run it, how to add a C test, how to add a suite, Docker requirement, image digest).
- Update `AGENTS.md` "Testing" section to reference `make tests` / `make run-tests` and Docker dependency.
- Fill `VERIFY.md` against the PRD's Outcome bullets and the Acceptance Mapping below.

Acceptance for Phase 6: documentation merged; VERIFY checklist all non-PENDING.

---

## Trade-offs

- T-1: **Vendor upstream suites vs. submodule vs. fetch-on-build.** Chosen: vendor (per user direction; reviewer TR-1 confirms across both iterations). Adv.: hermetic clones, offline builds, simple contributor workflow. Disadv.: large repo size; upstream sync is a manual diff. Provenance hardened by C-13.
- T-2: **Bake on top of upstream rootfs vs. build a new rootfs from scratch.** Confirmed: bake on top per reviewer TR-2 guidance. Adv.: minimal new build infrastructure; reuses Alpine, busybox, musl exactly as today. Disadv.: every test rootfs build re-copies a multi-MB image (negligible on modern disks).
- T-3': **Build-time switch — cargo-feature form vs. `option_env!` form vs. `build.rs`-generated literal.** Chosen: **cargo-feature form** (the only one that compiles cleanly given `include_str!`'s literal-only argument grammar). Adv.: keeps `make run` byte-identical (G-8 provable); isolates all test-rig changes inside the test build; matches the project's existing cargo-feature idiom; one-line `Cargo.toml` change. Disadv.: two kernel binaries (one with `init.sh`, one with `test.sh`) — acceptable since they're per-target outputs anyway. The `option_env!` form was rejected because `include_str!(CONST)` does not compile (R-001 in REVIEW 01). The `build.rs`-generated literal form is a future option if we ever need an arbitrary script path; not adopted now since the cargo feature form covers the binary case.
- T-4: **Per-suite contract — `Makefile` vs. `BUILD.sh` vs. uniform script.** Chosen: dual (Makefile preferred, BUILD.sh fallback, copy-only as last resort). Per TR-4 acceptance, `build-suites.sh` warns when both are present.
- T-5: **Failure semantics in `run-*.sh` — abort on first failure vs. continue.** Chosen: continue (C-8b). Adv.: full suite report each run; one failure doesn't hide later regressions. Disadv.: a hung test wastes time (mitigated by `lib/timeout.sh`); a kernel panic mid-run aborts everything anyway.
- T-6: **Where `run_qemu_tests` lives.** Chosen: refactor `scripts/make/qemu.mk` to a shared `run_qemu_with_disk` macro per TR-6 / C-15. Adv.: single source of truth for QEMU args; can't drift. Disadv.: a small qemu.mk refactor cost upfront.

---

## Validation

[**Unit Tests**]

- V-UT-1: `xtest/scripts/build/build-c.sh` against a fixture `xtest/c/` containing one passing `.c` and one deliberately-broken `.c`: passing `.c` produces an ELF; broken `.c` causes the script to record the error, continue, and exit non-zero with a clear summary citing the file (per C-8a).
- V-UT-2: `xtest/scripts/build/stage.sh` against a populated `xtest/build/<arch>/` fixture: produces the documented `stage/root/tests/{c,testsuites,scripts}` layout exactly; missing inputs fail with a clear error.
- V-UT-3: `xtest/scripts/build/bake-image.sh` against a tiny ext4 fixture image (created in the test): output image contains `/root/tests/sentinel` (from the staged tree); no `/test.sh` exists in the image; failure injection (rsync error) leaves no mounted loop device and no partial output image.
- V-UT-4: `dash -n src/test.sh` and `dash -n` over every `xtest/scripts/*.sh` and `xtest/scripts/**/*.sh` — POSIX shell syntax check (C-6).
- V-UT-5: `git grep -E 'Makefile\.sub|busybox-config-|git_testcode|sdcard-rv|sdcard-la'` over the repo returns no matches after Phase 1 (G-2).
- V-UT-6: `git status --porcelain xtest/build/` is empty after `make -C xtest all ARCH=riscv64` (C-2).
- V-UT-7: For every `xtest/testsuites/<s>/`, the directory contains at least one of `{LICENSE, COPYING, COPYING.LIB, NOTICE}` and `xtest/testsuites/UPSTREAM.md` has a row for that suite with non-empty `License (SPDX)` and `Local patches` cells (C-3 + C-13).
- V-UT-8: Build switch unit — `cargo build` (no feature) produces a kernel ELF that contains the first non-blank line of `src/init.sh` (via `strings`); `cargo build --features init-test` produces a kernel ELF that contains the first non-blank line of `src/test.sh` and **not** that of `src/init.sh` (G-11, C-12).
- V-UT-9: `grep -F 'make tests' AGENTS.md` and `grep -F 'make run-tests' AGENTS.md` both return non-empty after Phase 6; `xtest/README.md` exists and is non-empty (G-10).
- V-UT-10: `grep -E 'docker\.educg\.net/cg/os-contest@sha256:[a-f0-9]{64}' xtest/Makefile` returns one match; `grep -E 'docker\.educg\.net/cg/os-contest:[0-9]+' xtest/Makefile` (tag form) returns no matches (C-14).

[**Integration Tests**]

- V-IT-1: `make -C xtest all ARCH=riscv64` and `ARCH=loongarch64` inside Docker complete successfully on a clean checkout; `xtest/build/<arch>/tests-rootfs-<arch>.img` exists and is non-empty.
- V-IT-2: Loop-mount the produced image (in CI / inside Docker), assert `/root/tests/scripts/run-all.sh` exists and is executable, at least one `/root/tests/c/*` ELF exists, and **no** `/test.sh` exists in the image.
- V-IT-3: `make tests ARCH=...` from the top-level Makefile produces the same staged-tree contents (file paths + sha256 of each file) as `make -C xtest all`. Image bytes are not asserted (per G-6).
- V-IT-4: `make run-tests ARCH=riscv64` boots in QEMU with a wall-clock timeout, captures serial output, and asserts:
  - the line `cd /root/tests` (or equivalent first action of the embedded `test.sh`) is observed,
  - at least one `[PASS]` line from `run-c.sh` is observed,
  - the `basic` suite group header `==== basic ====` and matching `==== basic done ====` are observed,
  - the boot reaches the post-test `sh` prompt without panic.
- V-IT-5: Same as V-IT-4 but `ARCH=loongarch64`.
- V-IT-6a: `git diff main..HEAD -- src/init.sh` is empty (G-8).
- V-IT-7: `make -n run ARCH=riscv64` and `make -n run-tests ARCH=riscv64` produce QEMU command lines that differ **only** in the `-drive file=` argument; same `BLK`/`NET`/`MEM`/`LOG` plumbing and identical other flags (C-15, C-5).
- V-IT-8: After `cargo build` (no feature) for ARCH=riscv64, `strings $(OUT_ELF) | grep -F "$(awk 'NF{print;exit}' src/init.sh)"` matches; the same line from `src/test.sh` does **not** match. After `cargo build --features init-test` for ARCH=riscv64, the `src/test.sh` line matches and the `src/init.sh` line does not (G-8 + G-11).

[**Failure / Robustness Validation**]

- V-F-1: A first-party C test that `exit(1)`s prints `[FAIL] <name> exit=1` and the run continues to subsequent tests (C-8b).
- V-F-2: A first-party C test that segfaults prints `[FAIL] <name> signal=SEGV` (or equivalent) and the run continues (C-8b).
- V-F-3: A suite whose `run.sh` `sleep 9999`s is killed by `lib/timeout.sh` after the configured timeout; `[TIMEOUT] <suite>` is logged; the run continues (C-8b).
- V-F-4: `bake-image.sh` interrupted (SIGTERM) mid-rsync leaves no mounted loop device, no partial output image — verified by re-running `mount` and `losetup -a` after.
- V-F-5: `make tests` with Docker uninstalled prints the documented "docker not found — install Docker and pull <image-url>" error and exits non-zero (C-1, no half-built artifacts).
- V-F-6: After temporarily removing `src/test.sh`, `cargo build --features init-test ARCH=riscv64` fails at compile time with `include_str!` reporting the missing path (Failure Flow item 8).

[**Edge Case Validation**]

- V-E-1: Two `xtest/c/` files with the same basename in different subdirectories cause `build-c.sh` to fail with a "duplicate test name" error (C-9).
- V-E-2: Empty `xtest/c/` (no `.c` files) succeeds: `build-c.sh` produces an empty `c/` dir; `run-c.sh` emits `no first-party tests` and continues.
- V-E-3: `xtest/testsuites/<s>/` with neither `Makefile` nor `BUILD.sh` is copied verbatim by `build-suites.sh`. With both present, a `[WARN] suite <s>: both Makefile and BUILD.sh present; preferring Makefile` is printed (TR-4 acceptance).
- V-E-4: `make tests` re-run with no source changes produces the same staged-tree sha256 set as the previous run (G-6).
- V-E-5: `make tests ARCH=riscv64` followed by `make tests ARCH=loongarch64` both succeed without one wiping the other's `xtest/build/<arch>/` outputs.

[**Acceptance Mapping**]

| Goal / Constraint | Validation |
|-------------------|------------|
| G-1 (layout)                          | V-IT-1, V-IT-2, V-E-2 |
| G-2 (old pipeline deleted)            | V-UT-5 |
| G-3 (per-test C ELFs + run-c.sh)      | V-UT-1, V-IT-2, V-IT-4, V-F-1, V-F-2, V-E-1 |
| G-4 (vendored suites + UPSTREAM.md)   | V-IT-1, V-UT-7 |
| G-5 (`src/test.sh` embedded + run-all dispatch) | V-UT-4, V-UT-8, V-IT-4, V-IT-5 |
| G-6 (`make tests` deterministic staging) | V-IT-3, V-E-4 |
| G-7 (`make run-tests` boots image)    | V-IT-4, V-IT-5, V-IT-7 |
| G-8 (`make run` unchanged)            | V-IT-6a, V-IT-8 |
| G-9 (smoke on rv + la)                | V-IT-4, V-IT-5, V-F-1, V-F-2, V-F-3 |
| G-10 (documentation)                  | V-UT-9 |
| G-11 (build switch)                   | V-UT-8, V-IT-8, V-F-6 |
| C-1 (Docker-only host deps + fail-fast guard) | V-F-5 |
| C-2 (`build/` gitignored)             | V-UT-6 |
| C-3 (vendored + UPSTREAM.md)          | V-UT-7 |
| C-4 (no Makefile var renames)         | V-IT-7 |
| C-5 (ARCH/BLK/NET/MEM/LOG passthrough)| V-IT-4, V-IT-7 |
| C-6 (POSIX shell)                     | V-UT-4 |
| C-7 (Docker `--privileged`)           | V-IT-1; V-F-4 |
| C-8a (build-time fail-fast)           | V-UT-1, V-F-5 |
| C-8b (run-time fail-soft)             | V-F-1, V-F-2, V-F-3 |
| C-9 (unique C test names)             | V-E-1 |
| C-10 (static musl link)               | V-IT-2 (file inspection); V-IT-4 (runs on Alpine) |
| C-12 (build switch — cargo feature)   | V-UT-8, V-IT-8, V-F-6 |
| C-13 (per-suite license preservation) | V-UT-7 |
| C-14 (Docker image digest pin)        | V-UT-10, V-IT-1 |
| C-15 (qemu.mk shared macro)           | V-IT-7 |
