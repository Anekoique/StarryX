# `xtest` PLAN `01`

> Status: Revised
> Feature: `xtest`
> Iteration: `01`
> Owner: Executor
> Depends on:
> - Previous Plan: `00_PLAN.md`
> - Review: `00_REVIEW.md`
> - Master Directive: `none`

---

## Summary

Iteration 01 fixes the load-bearing flaw in PLAN 00: `src/init.sh` is **embedded into the kernel binary** at compile time via `include_str!`, not loaded from the rootfs. Therefore `/test.sh` cannot be "the script the existing init invokes." This iteration replaces that mechanism with a **build-time switch** — `src/main.rs`'s `include_str!` macro path is selected via an `AX_INIT_SCRIPT` env var read at compile time (defaulting to `init.sh`); `make tests` exports `AX_INIT_SCRIPT=test.sh`, producing a kernel binary that embeds `src/test.sh`. `make run` is unaffected. The bake step's responsibility shrinks: it only stages the test tree under `/root/tests` — it no longer installs any in-image init script.

Also addressed: stale OS-COMP marker line in module decoupling, unverified musl cross-prefix story (now a Phase 0 spike whose result is recorded as named cross-prefix Make variables per arch), Docker-mandate documentation, per-suite license preservation, qemu.mk macro refactor, Docker image digest pinning, build-time vs run-time fail semantics split, Acceptance Mapping gaps, and the G-8 verification shape.

## Log

[**Added**]
- New Phase 0 (Toolchain Spike) — verifies the musl cross prefixes inside the contest Docker image and pins them as `MUSL_CC_RV64` / `MUSL_CC_LA64` Make variables before any C-test build code is written.
- New Spec section `[**Build Switch**]` (G-11, C-12) describing the `AX_INIT_SCRIPT` build-time mechanism.
- Constraint split: C-8 → C-8a (build-time fail-fast) + C-8b (run-time fail-soft).
- New constraints: C-12 (build-time switch), C-13 (per-suite license preservation), C-14 (Docker image digest pin), C-15 (qemu.mk shared macro contract).
- New validations: V-UT-5 (G-2 deletion check), V-UT-6 (C-2 gitignore check), V-UT-7 (C-3 + C-13 per-suite license check), V-UT-8 (G-11 build-switch unit), V-IT-7 (C-15 qemu.mk parity), V-IT-8 (`strings` check on the kernel ELF), V-F-6 (missing-script `cargo build` failure).

[**Changed**]
- G-5 rewritten: `src/test.sh` is **not** placed in the rootfs; it is embedded into the test-build kernel binary by the build switch.
- G-7 rewritten: `make run-tests` builds the kernel **with `AX_INIT_SCRIPT=test.sh`** plus the test rootfs image.
- G-8 sharpened: verified by both diff (V-IT-6a) and ELF inspection (V-IT-8).
- NG-5 narrowed: explicitly *does* introduce `AX_INIT_SCRIPT` as a new build-time env var read by `option_env!` in `src/main.rs`.
- T-3 deleted as drafted; replaced with T-3' covering "build-time switch (Option A) vs init.sh dispatcher (Option B)" — Option A chosen.
- T-2's "(assumed)" promoted to "Confirmed: bake on top per reviewer guidance" with rationale.
- T-6's "may end up as a sibling macro" hardened into a concrete commitment (Phase 5 refactor, see C-15).
- Architecture diagram + module decoupling text scrubbed of every "OS-COMP marker" reference (R-002).
- Phase 1 acceptance promoted "old pipeline fully dead and unreferenced" from implicit to explicit (R-004 option a).
- Failure Flow item 5 now explicitly references C-8b instead of generic C-8.
- Acceptance Mapping rewritten to cite V-* entries for every Goal and Constraint (R-010).
- bake-image.sh contract: removed "install src/test.sh -> /test.sh" — the test script is no longer rootfs-resident.

[**Removed**]
- Old G-5 wording referencing `/test.sh` as a rootfs file.
- Old T-3 "in-image `/test.sh` location" trade-off (the question doesn't apply once R-001 is resolved).
- bake-image.sh's `TEST_SH` input parameter.

[**Unresolved**]
- Whether the `os-contest:20250714` image actually contains a loongarch64 musl cross compiler (Phase 0 spike will answer; if missing, Phase 0's exit branches into one of three pre-agreed fallbacks per R-003).

[**Response Matrix**]

| Source | ID | Decision | Resolution |
|--------|----|----------|------------|
| Review | R-001 | Accepted | Build-time switch (Option A) adopted. New G-11 + C-12 + V-UT-8 + V-IT-8. `src/main.rs` will read `option_env!("AX_INIT_SCRIPT")` (default `"init.sh"`) inside `include_str!`. `make tests`/`make run-tests` set it to `test.sh`. `make run` and the kernel ELF for `make run` stay byte-identical. T-3 replaced. NG-5 lifted accordingly. |
| Review | R-002 | Accepted | "Emit OS-COMP markers" line in Architecture/Module decoupling deleted; replaced with "emit a stable `[PASS] <name>` / `[FAIL] <name> exit=<n>` plain-text format with per-suite group headers." `grep` over the new PLAN finds no `OS COMP|marker|contest` references. |
| Review | R-003 | Accepted | New Phase 0 (Toolchain Spike) added. Spike runs `find / -name 'gcc'` and `find / -name 'libc.a'` inside the pinned Docker image; results encoded as `MUSL_CC_RV64`/`MUSL_CC_LA64` Make vars. C-10 hardened. If loongarch64 musl is missing, the spike picks one of (a) install during Phase 2, (b) accept libgcc on Alpine via the existing `Makefile.sub` ld-musl symlink trick, (c) drop loongarch C tests from G-3/G-9. |
| Review | R-004 | Accepted | Phase 1 acceptance gains an explicit Spec note + V-UT-5 (`git grep` returns empty). |
| Review | R-005 | Accepted | C-3 hardened with C-13 + V-UT-7. `xtest/testsuites/UPSTREAM.md` schema extended with per-suite `License (SPDX)` and `Local patches` columns. |
| Review | R-006 | Accepted | C-1 expanded: `xtest/Makefile` checks `command -v docker` and fails fast with a clear error citing the contest image URL. Phase 6 documentation updates `AGENTS.md` "Testing" to call out Docker dependency. No `XTEST_NO_DOCKER` escape hatch (keep Docker-only to avoid silent toolchain drift). |
| Review | R-007 | Accepted | C-15 added: `scripts/make/qemu.mk` gains `run_qemu_with_disk` macro that takes the disk path as `$(1)`; `run_qemu` and `run_qemu_tests` are both one-line callers. Phase 5 first bullet rewritten. V-IT-7 diffs `make -n run` against `make -n run-tests` (only the disk path differs). |
| Review | R-008 | Accepted | C-14 added: pin the contest Docker image by digest. G-6 weakened to "deterministic *staging* given pinned toolchain"; V-IT-3 reframed to assert *staged-tree equality* (file paths + per-file sha256), not image-byte equality. |
| Review | R-009 | Accepted | C-8 split into C-8a + C-8b; Failure Flow items 3 and 5 cite the right one. |
| Review | R-010 | Accepted | Acceptance Mapping rewritten. New V-UT-5 (G-2), V-UT-6 (C-2), V-UT-7 (C-3 + C-13). Every G-N and C-N now has a V-* entry inside the Validation section. |
| Review | R-011 | Accepted | New API Surface entry: `make tests` writes `xtest/build/<arch>/tests-rootfs-<arch>.img`; that path is exported as `TESTS_ROOTFS_IMG` and consumed by `run_qemu_tests`. |
| Review | R-012 | Accepted | V-IT-6 split into V-IT-6a (`git diff src/init.sh` empty across the branch) + V-IT-8 (kernel ELF embedded init-script string check). |
| Review | TR-1 | Accepted | T-1 unchanged; vendor decision retained. R-005 raises the provenance bar (now C-13). |
| Review | TR-2 | Accepted | T-2 promoted from "(assumed)" to "Confirmed". |
| Review | TR-3 | Accepted | Old T-3 deleted; new T-3' adopts Option A (build-time switch). |
| Review | TR-4 | Accepted | T-4 unchanged. New small acceptance: `build-suites.sh` warns when both `Makefile` and `BUILD.sh` exist for a suite. |
| Review | TR-5 | Accepted | T-5 unchanged; covered by the C-8a/C-8b split. |
| Review | TR-6 | Accepted | T-6 hardened into C-15; Option B (qemu.mk refactor) chosen. |

---

## Spec

[**Goals**]

- G-1: `xtest/` becomes the in-repo test environment producer; layout is `xtest/{c,testsuites,scripts}` (no other top-level dirs).
- G-2: Every removable piece of the old sdcard pipeline (`xtest/Makefile` (old), `xtest/Makefile.sub`, `xtest/config/`, `xtest/scripts/git_testcode.sh`) is deleted in this task; nothing in the repo references them after the change.
- G-3: First-party C tests live under `xtest/c/` as one `.c` per test; each compiles to one statically-linked ELF using the cross toolchain identified by the Phase 0 spike (`MUSL_CC_RV64` / `MUSL_CC_LA64`). A `xtest/scripts/run-c.sh` driver iterates them and reports pass/fail.
- G-4: A vendored subset of `oscomp/testsuites-for-oskernel @ pre-2025` lives under `xtest/testsuites/<suite>/`, one directory per suite. Sources are committed directly (no submodule, no fetch-on-build). Pinned upstream commit, per-suite license SPDX, and per-suite local-patches summary recorded in `xtest/testsuites/UPSTREAM.md`.
- G-5: `src/test.sh` is a POSIX `sh` script that lives in `src/` next to `src/init.sh`. It is **embedded into the kernel binary** (not into the rootfs) when `make tests`/`make run-tests` build the kernel with `AX_INIT_SCRIPT=test.sh`. Inside the booted kernel it sets `PATH`/`LD_LIBRARY_PATH`/`HOME`, `cd`s into `/root/tests`, runs `./scripts/run-all.sh`, then `exec sh`s for an interactive prompt.
- G-6: `make tests ARCH={riscv64|loongarch64}` produces `tests-rootfs-$ARCH.img` deterministically with respect to the *staged tree* given the pinned toolchain (image bytes may differ due to ext4 timestamps; per-binary determinism rides on the Docker image digest pin in C-14).
- G-7: `make run-tests ARCH=...` (a) builds the kernel with `AX_INIT_SCRIPT=test.sh`, (b) builds the test rootfs image, (c) boots StarryX in QEMU with `tests-rootfs-$ARCH.img` mounted as the root disk via the shared `run_qemu_with_disk` macro.
- G-8: `make run` and `src/init.sh` remain byte-for-byte unchanged after the task lands. The kernel ELF produced by `make run` is verified to embed exactly `src/init.sh` and nothing else (V-IT-6a + V-IT-8).
- G-9: On both `riscv64` and `loongarch64`, end-to-end smoke succeeds: the test rootfs boots under the test-built kernel, `run-all.sh` executes the `basic` suite plus all first-party C tests, and per-test pass/fail lines appear on the serial console. A test failing does **not** abort the run (C-8b).
- G-10: Documentation: `xtest/README.md` describes how to run the pipeline and how to add tests; `AGENTS.md` "Testing" section is updated to reference `make tests` / `make run-tests` and Docker dependency.
- G-11: Build switch — `src/main.rs`'s embedded init script is selected at build time by an `AX_INIT_SCRIPT` env var consumed via `option_env!`; defaults to `init.sh`; only `make tests`/`make run-tests` set it to `test.sh`.

- NG-1: Not building or modifying cross toolchains. The Phase 0 spike *records* what the contest Docker image already provides; it does not add tools.
- NG-2: Not rebuilding the Alpine rootfs from scratch. We bake on top of `Starry-OS/rootfs/rootfs-$ARCH.img` (per TR-2).
- NG-3: Not maintaining glibc-side test variants. Alpine is musl-only; `xtest/c/` and the vendored suites target musl.
- NG-4: Not introducing tier groupings (preliminary / final1 / final2). `run-all.sh` runs everything sequentially in a stable order.
- NG-5: Not changing how the kernel is built beyond the single new `AX_INIT_SCRIPT` env var read by `option_env!` in `src/main.rs`. `scripts/make/build.mk`, target features, axconfig flow are untouched.
- NG-6: Not removing or repurposing `qemu_rootfs` / the upstream rootfs download. `make run` keeps using it.
- NG-7: Not adding host-OS support beyond what the contest Docker image enables (Linux + Docker). macOS hosts go through Docker. `XTEST_NO_DOCKER` is **not** offered (R-006 decision).
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
│   ├─ AX_INIT_SCRIPT=test.sh make build ARCH=riscv64               │
│   │     └─ src/main.rs's option_env!("AX_INIT_SCRIPT") = "test.sh"│
│   │        → include_str! embeds src/test.sh into the kernel ELF  │
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
- **Kernel-embed side** (`src/test.sh` + `src/main.rs`'s `include_str!` + `option_env!`) only knows how to dispatch to the in-image test entry point. It does not know how the rootfs was built.
- **Runtime side** (`xtest/scripts/run-*.sh`) only knows how to discover and execute test binaries and emit a stable `[PASS]/[FAIL]` plain-text format with per-suite group headers. It does not know how anything was built.
- The bridge between build side and runtime side is the **staging contract**: `xtest/build/<arch>/stage/root/tests/{c,testsuites,scripts}` has a fixed shape; `bake-image` copies it verbatim to `/root/tests` in the image. The bridge between build side and kernel-embed side is the **AX_INIT_SCRIPT contract**: `make tests`/`make run-tests` always export `AX_INIT_SCRIPT=test.sh` for the kernel build step; everything else leaves it unset.

[**Build Switch**]

`src/main.rs` line 64 changes from:

```rust
let init = include_str!("init.sh");
```

to either:

```rust
// Option_env form: AX_INIT_SCRIPT picks the include_str! path.
const INIT_SCRIPT: &str = match option_env!("AX_INIT_SCRIPT") {
    Some(s) => s,
    None => "init.sh",
};
let init = include_str!(INIT_SCRIPT);
```

…or, fallback if `include_str!` cannot accept a `const &str` literal cleanly under the pinned toolchain:

```rust
#[cfg(feature = "init-test")]
let init = include_str!("test.sh");
#[cfg(not(feature = "init-test"))]
let init = include_str!("init.sh");
```

with `make tests`/`make run-tests` adding `--features init-test` to the cargo invocation. **Phase 0 spike picks the form**; both produce identical runtime behaviour. Either way, `make run` (no env, no feature) embeds `init.sh` exactly as today.

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
make run-tests       ARCH=riscv64|loongarch64    # build kernel (with switch) + image + boot
```

New top-level Make variables:

```
TESTS_ROOTFS_IMG  := $(ROOT_DIR)/xtest/build/$(ARCH)/tests-rootfs-$(ARCH).img
AX_INIT_SCRIPT    ?= init.sh    # set to test.sh by make tests/make run-tests
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

`xtest/scripts/build/bake-image.sh` contract (revised):

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
- C-11: (reserved — was old C-11; promoted to C-15.)
- C-12: Build switch — `src/main.rs` reads `AX_INIT_SCRIPT` (default `init.sh`) at compile time via `option_env!` (or, fallback, an `init-test` cargo feature; Phase 0 picks the form). `make run` does not set the env / feature; `make tests` and `make run-tests` do (`AX_INIT_SCRIPT=test.sh` or `--features init-test`).
- C-13: Per-suite license preservation — every `xtest/testsuites/<s>/` subtree contains the upstream's license / copyright file(s) verbatim. `UPSTREAM.md` records each suite's SPDX identifier (e.g. `GPL-2.0-only` for LTP, `BSD-3-Clause` for iperf, ...).
- C-14: Docker image pin — `xtest/Makefile`'s `DOCKER_IMAGE` references the contest image by digest (`@sha256:...`), not by mutable tag. Digest captured at Phase 0 and recorded in `xtest/Makefile` and `xtest/README.md`.
- C-15: qemu.mk shared macro — `scripts/make/qemu.mk` factors the QEMU invocation through a `run_qemu_with_disk` macro that takes the disk image path as `$(1)`. `run_qemu` and `run_qemu_tests` are both one-line callers. Every other QEMU flag stays in the macro body so the two paths cannot drift.

---

## Runtime

[**Main Flow**] — `make run-tests ARCH=riscv64`

1. Top-level `Makefile`'s `run-tests` target depends on `tests` and on `build` (the latter invoked with `AX_INIT_SCRIPT=test.sh` exported).
2. `tests` target: ensure upstream `rootfs-$ARCH.img` exists (delegate to existing `qemu_rootfs` logic factored into a shared make function), then `docker run … make -C xtest all ARCH=$(ARCH)`.
3. Inside Docker, `xtest/Makefile`'s `all` runs `build-c → build-suites → stage → bake-image`:
   a. `build-c.sh` finds every `xtest/c/**/*.c`, compiles each with `$(MUSL_CC_<ARCH>) -static -I xtest/c/common -O2 -o xtest/build/<arch>/c/<basename>` (per C-10).
   b. `build-suites.sh` iterates `xtest/testsuites/*/`; for each suite, dispatches per the per-suite recipe contract.
   c. `stage.sh` assembles `xtest/build/<arch>/stage/root/tests/{c,testsuites,scripts}` from the build outputs plus `xtest/scripts/run-*.sh` and `xtest/scripts/lib/`.
   d. `bake-image.sh` copies `rootfs-$ARCH.img` to `tests-rootfs-$ARCH.img`, loop-mounts it, `rsync -a` the staged tree into `/root/tests`, `umount`s, releases the loop device. **No init script is installed.**
4. Back on the host, `run-tests` invokes the kernel build with the build switch: `AX_INIT_SCRIPT=test.sh make build ARCH=...`. `src/main.rs`'s `option_env!("AX_INIT_SCRIPT")` evaluates to `Some("test.sh")`, so `include_str!` embeds `src/test.sh` into the kernel ELF.
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
8. **`AX_INIT_SCRIPT` set to a missing file:** `include_str!` fails at compile time with a clear path error before any image is built. (`include_str!` paths are resolved relative to `src/`, so `AX_INIT_SCRIPT=does_not_exist.sh make build` errors out at `cargo build`.)

[**State Transitions**]

- `xtest/build/<arch>/` empty → populated, by `build-c` + `build-suites`.
- Build outputs → `stage/`, by `stage.sh` (rsync-style copy with stable file modes).
- `stage/` + `rootfs-$ARCH.img` → `tests-rootfs-$ARCH.img`, by `bake-image.sh`.
- Idle disk → mounted under `mnt/` → unmounted, inside `bake-image.sh` (always restored on failure via trap).
- Kernel ELF (default) → kernel ELF (test-build), by adding `AX_INIT_SCRIPT=test.sh` to the cargo env.
- Old `xtest/Makefile`, `Makefile.sub`, `config/`, `scripts/git_testcode.sh` exist → deleted, by Phase 1.

---

## Implementation

[**Phase 0 — Toolchain Spike (single throwaway Docker session, no commits)**]

- Run `docker run --rm -it docker.educg.net/cg/os-contest:20250714 bash`; inside:
  - `find / -name 'gcc' 2>/dev/null` and filter to musl-targeted entries.
  - `find / -name 'libc.a' 2>/dev/null` for both arches.
  - From the host: `docker inspect --format '{{index .RepoDigests 0}}' docker.educg.net/cg/os-contest:20250714` to capture the digest.
- Decide which form of the build switch to use (`option_env!` vs `cfg!`-feature) by trying both in a one-line `src/main.rs` test.
- Record results into the PLAN's `[**API Surface**]` (`MUSL_CC_RV64`, `MUSL_CC_LA64`, `DOCKER_IMAGE` digest) — Phase 0 produces a *PLAN edit*, not a code commit.
- If loongarch64 musl cross is missing: pick one of (a) install during Phase 2, (b) accept libgcc on Alpine via the existing `Makefile.sub` ld-musl symlink trick, (c) drop loongarch C tests from G-3/G-9. Record the choice in the PLAN before Phase 1.

Acceptance for Phase 0: PLAN's API Surface section has concrete strings for `MUSL_CC_RV64`, `MUSL_CC_LA64`, `DOCKER_IMAGE` digest. No code committed yet.

[**Phase 1 — demolition + skeleton (single commit)**]

- Delete `xtest/Makefile` (old), `xtest/Makefile.sub`, `xtest/config/`, `xtest/scripts/git_testcode.sh`.
- Verify with `git grep`: nothing in the repo references `Makefile.sub`, `busybox-config-`, `git_testcode`, `sdcard-rv.img`, or `sdcard-la.img`.
- Create new `xtest/` skeleton: empty `c/`, `testsuites/`, `scripts/{build,lib}/`, plus `xtest/Makefile`, `xtest/README.md`, `xtest/.gitignore` (ignores `build/`), `xtest/testsuites/UPSTREAM.md` (placeholder pending Phase 3).
- Add `src/test.sh` (placeholder that just `exec sh`s — full logic lands in Phase 4).
- Add `src/main.rs`'s build switch (per `[**Build Switch**]`). `make run` still embeds `init.sh` because no env is set.
- Add top-level `make tests` / `make run-tests` targets (no-ops that print "not yet implemented" and exit 0; wired up properly in Phase 5).
- Top-level `Makefile` `.PHONY` line includes `tests run-tests`.

Acceptance for Phase 1: `git grep -E 'Makefile\.sub|busybox-config-|git_testcode|sdcard-rv|sdcard-la'` returns no matches; `make tests` and `make run-tests` resolve as targets (`make -n tests` succeeds); `make build ARCH=riscv64` produces a kernel ELF whose embedded init-script string equals `src/init.sh`'s contents (V-IT-8 dry run).

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

- Implement `xtest/scripts/build/bake-image.sh` (cp rootfs, mount, rsync, umount, with trap-based cleanup; per the revised contract — **no `TEST_SH` input**).
- Wire `xtest/Makefile`'s `bake-image` target.
- Replace the placeholder `src/test.sh` with the real one (env exports, run-all dispatch, exec sh fallback). Lint with `dash -n`.

Acceptance for Phase 4: `make -C xtest bake-image ARCH=riscv64` produces `xtest/build/riscv64/tests-rootfs-riscv64.img`. Loop-mounting it manually shows `/root/tests/{c,testsuites,scripts}` populated and **no** `/test.sh` file at the root (per the revised contract).

[**Phase 5 — top-level wiring + qemu.mk refactor + boot smoke**]

- Refactor `scripts/make/qemu.mk`: extract the QEMU invocation into `run_qemu_with_disk = ... $(1) ...`. `run_qemu` becomes `$(call run_qemu_with_disk,$(DISK_IMG))`. Add `run_qemu_tests = $(call run_qemu_with_disk,$(TESTS_ROOTFS_IMG))`. Confirm `make -n run` is byte-identical (modulo whitespace) before and after the refactor.
- Promote `make tests` from Phase 1 placeholder to a real target that delegates to `xtest/Makefile`.
- Promote `make run-tests` from placeholder to: build-test-image + `AX_INIT_SCRIPT=test.sh make build ARCH=...` + `$(call run_qemu_tests)`.
- Factor the upstream-rootfs-fetch logic out of `qemu_rootfs` into a reusable make function so both `qemu_rootfs` (existing) and `tests` (new) call it.
- Boot smoke on `riscv64` and `loongarch64` (`make run-tests ARCH=...`); confirm the embedded `test.sh` runs, the C tests print PASS/FAIL, the `basic` suite prints its group header/footer with per-test results, and the run lands in an interactive `sh` afterwards.

Acceptance for Phase 5: G-7 + G-9 satisfied; both arches boot the test rootfs under the test-built kernel, run the suites, never abort, and drop to a shell. `make -n run` is byte-identical to its pre-refactor output. `make run` still works and its kernel ELF still embeds `src/init.sh` exactly (V-IT-8).

[**Phase 6 — documentation + verify pass**]

- Write `xtest/README.md` (what xtest is, how to run it, how to add a C test, how to add a suite, Docker requirement, image digest).
- Update `AGENTS.md` "Testing" section to reference `make tests` / `make run-tests` and Docker dependency (R-006 (a) decision).
- Fill `VERIFY.md` against the PRD's Outcome bullets and the Acceptance Mapping below.

Acceptance for Phase 6: documentation merged; VERIFY checklist all non-PENDING.

---

## Trade-offs

- T-1: **Vendor upstream suites vs. submodule vs. fetch-on-build.** Chosen: vendor (per user direction; reviewer TR-1 confirms). Adv.: hermetic clones, offline builds, simple contributor workflow. Disadv.: large repo size; upstream sync is a manual diff. Provenance hardened by C-13.
- T-2: **Bake on top of upstream rootfs vs. build a new rootfs from scratch.** Confirmed: bake on top per reviewer TR-2 guidance. Adv.: minimal new build infrastructure; reuses Alpine, busybox, musl exactly as today. Disadv.: every test rootfs build re-copies a multi-MB image (negligible on modern disks).
- T-3': **Build-time switch (Option A) vs init.sh dispatcher (Option B).** Chosen: Option A (env-driven `option_env!` in `src/main.rs`, fallback to `cfg!` feature flag). Adv.: keeps `make run` byte-identical (G-8 provable), isolates all test-rig changes inside the test build, matches the project's existing `AX_*` env-driven `axconfig` style. Disadv.: two kernel binaries (one with `init.sh`, one with `test.sh`) — acceptable since they're per-target outputs anyway. Option B was rejected because it violates the literal G-8 ("`src/init.sh` byte-for-byte unchanged") and adds a runtime check on every `make run` boot.
- T-4: **Per-suite contract — `Makefile` vs. `BUILD.sh` vs. uniform script.** Chosen: dual (Makefile preferred, BUILD.sh fallback, copy-only as last resort). Per TR-4 acceptance, `build-suites.sh` warns when both are present.
- T-5: **Failure semantics in `run-*.sh` — abort on first failure vs. continue.** Chosen: continue (C-8b). Adv.: full suite report each run; one failure doesn't hide later regressions. Disadv.: a hung test wastes time (mitigated by `lib/timeout.sh`); a kernel panic mid-run aborts everything anyway.
- T-6: **Where `run_qemu_tests` lives.** Chosen: refactor `scripts/make/qemu.mk` to a shared `run_qemu_with_disk` macro per TR-6 / C-15. Adv.: single source of truth for QEMU args; can't drift. Disadv.: a small qemu.mk refactor cost upfront.

---

## Validation

[**Unit Tests**]

- V-UT-1: `xtest/scripts/build/build-c.sh` against a fixture `xtest/c/` containing one passing `.c` and one deliberately-broken `.c`: passing `.c` produces an ELF; broken `.c` causes the script to record the error, continue, and exit non-zero with a clear summary citing the file (per C-8a).
- V-UT-2: `xtest/scripts/build/stage.sh` against a populated `xtest/build/<arch>/` fixture: produces the documented `stage/root/tests/{c,testsuites,scripts}` layout exactly; missing inputs fail with a clear error.
- V-UT-3: `xtest/scripts/build/bake-image.sh` against a tiny ext4 fixture image (created in the test): output image contains `/root/tests/sentinel` (from the staged tree); no `/test.sh` exists in the image (revised contract); failure injection (rsync error) leaves no mounted loop device and no partial output image.
- V-UT-4: `dash -n src/test.sh` and `dash -n` over every `xtest/scripts/*.sh` and `xtest/scripts/**/*.sh` — POSIX shell syntax check (C-6).
- V-UT-5: `git grep -E 'Makefile\.sub|busybox-config-|git_testcode|sdcard-rv|sdcard-la'` over the repo returns no matches after Phase 1 (G-2).
- V-UT-6: `git status --porcelain xtest/build/` is empty after `make -C xtest all ARCH=riscv64` (C-2).
- V-UT-7: For every `xtest/testsuites/<s>/`, the directory contains at least one of `{LICENSE, COPYING, COPYING.LIB, NOTICE}` and `xtest/testsuites/UPSTREAM.md` has a row for that suite with non-empty `License (SPDX)` and `Local patches` cells (C-3 + C-13).
- V-UT-8: Build switch unit — building with no env produces a kernel ELF that contains the first non-blank line of `src/init.sh` (via `strings`); building with `AX_INIT_SCRIPT=test.sh` produces a kernel ELF that contains the first non-blank line of `src/test.sh` and **not** that of `src/init.sh` (G-11, C-12).

[**Integration Tests**]

- V-IT-1: `make -C xtest all ARCH=riscv64` and `ARCH=loongarch64` inside Docker complete successfully on a clean checkout; `xtest/build/<arch>/tests-rootfs-<arch>.img` exists and is non-empty.
- V-IT-2: Loop-mount the produced image (in CI / inside Docker), assert `/root/tests/scripts/run-all.sh` exists and is executable, at least one `/root/tests/c/*` ELF exists, and **no** `/test.sh` exists in the image.
- V-IT-3: `make tests ARCH=...` from the top-level Makefile produces the same staged-tree contents (file paths + sha256 of each file) as `make -C xtest all`. Image bytes are not asserted (per G-6 + R-008).
- V-IT-4: `make run-tests ARCH=riscv64` boots in QEMU with a wall-clock timeout, captures serial output, and asserts:
  - the line `cd /root/tests` (or equivalent first action of the embedded `test.sh`) is observed,
  - at least one `[PASS]` line from `run-c.sh` is observed,
  - the `basic` suite group header `==== basic ====` and matching `==== basic done ====` are observed,
  - the boot reaches the post-test `sh` prompt without panic.
- V-IT-5: Same as V-IT-4 but `ARCH=loongarch64`.
- V-IT-6a: `git diff main..HEAD -- src/init.sh` is empty (G-8).
- V-IT-7: `make -n run ARCH=riscv64` and `make -n run-tests ARCH=riscv64` produce QEMU command lines that differ **only** in the `-drive file=` argument (and any test-only flags); same `BLK`/`NET`/`MEM`/`LOG` plumbing (C-15, C-5).
- V-IT-8: After `make build ARCH=riscv64` (no env), `strings $(OUT_ELF) | grep -F "$(awk 'NF{print;exit}' src/init.sh)"` matches; the same line from `src/test.sh` does **not** match. After `AX_INIT_SCRIPT=test.sh make build ARCH=riscv64`, the `src/test.sh` line matches and the `src/init.sh` line does not (G-8 + G-11).

[**Failure / Robustness Validation**]

- V-F-1: A first-party C test that `exit(1)`s prints `[FAIL] <name> exit=1` and the run continues to subsequent tests (C-8b).
- V-F-2: A first-party C test that segfaults prints `[FAIL] <name> signal=SEGV` (or equivalent) and the run continues (C-8b).
- V-F-3: A suite whose `run.sh` `sleep 9999`s is killed by `lib/timeout.sh` after the configured timeout; `[TIMEOUT] <suite>` is logged; the run continues (C-8b).
- V-F-4: `bake-image.sh` interrupted (SIGTERM) mid-rsync leaves no mounted loop device, no partial output image — verified by re-running `mount` and `losetup -a` after.
- V-F-5: `make tests` with Docker uninstalled prints the documented "docker not found — install Docker and pull <image-url>" error and exits non-zero (C-1, no half-built artifacts).
- V-F-6: `AX_INIT_SCRIPT=does_not_exist.sh make build ARCH=riscv64` fails at `cargo build` with `include_str!` reporting the missing path (Failure Flow item 8).

[**Edge Case Validation**]

- V-E-1: Two `xtest/c/` files with the same basename in different subdirectories cause `build-c.sh` to fail with a "duplicate test name" error (C-9).
- V-E-2: Empty `xtest/c/` (no `.c` files) succeeds: `build-c.sh` produces an empty `c/` dir; `run-c.sh` emits `no first-party tests` and continues.
- V-E-3: `xtest/testsuites/<s>/` with neither `Makefile` nor `BUILD.sh` is copied verbatim by `build-suites.sh`. With both present, a `[WARN] suite <s>: both Makefile and BUILD.sh present; preferring Makefile` is printed (TR-4 acceptance).
- V-E-4: `make tests` re-run with no source changes produces the same staged-tree sha256 set as the previous run (G-6 + R-008).
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
| G-10 (documentation)                  | Phase 6 deliverables; VERIFY checklist |
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
| C-12 (build switch)                   | V-UT-8, V-IT-8, V-F-6 |
| C-13 (per-suite license preservation) | V-UT-7 |
| C-14 (Docker image digest pin)        | inspection of `xtest/Makefile` `DOCKER_IMAGE` post-Phase-0; V-IT-1 reproducibility |
| C-15 (qemu.mk shared macro)           | V-IT-7 |
