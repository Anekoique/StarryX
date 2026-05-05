# `xtest` PLAN `03`

> Status: Revised
> Feature: `xtest`
> Iteration: `03`
> Owner: Executor
> Depends on:
> - Previous Plan: `02_PLAN.md`
> - Review: `02_REVIEW.md`
> - Master Directive: `none`

---

## Summary

Iteration 03 fixes the Make-side passthrough for the `init-test` cargo feature and tightens four supporting details.

PLAN 02 assumed `--features init-test` could ride through the existing `FEATURES` plumbing. It cannot: `scripts/make/features.mk` line 27 prepends `axfeat/` to every entry in `FEATURES` and `scripts/make/cargo.mk` line 24 only consumes `$(AX_FEAT)`, so `FEATURES=init-test` would become `axfeat/init-test` (a feature on the wrong crate) and the build would fail. Iteration 03 adopts a new top-level Make variable `ROOT_FEATURES` (separate from `FEATURES`); `cargo.mk`'s `cargo_build` macro is extended to append `--features "$(ROOT_FEATURES)"` to its cargo invocation when non-empty (without the `axfeat/` prefix). `make tests`/`make run-tests` set `ROOT_FEATURES := init-test`; nothing else sets it. This is the cleanest seam (option (a) from REVIEW 02 R-001 — minimal surface, no behavioural change for any existing caller).

Four tightening fixes also land: V-UT-8 / V-IT-8 / Phase 1 acceptance now match against per-script ID-marker comments (`# id: starry-init` / `# id: starry-test`) instead of the brittle "first non-blank line" idiom; Phase 1's acceptance command uses the concrete `ROOT_FEATURES=init-test` form; the `tests-rootfs-<arch>.img` path is annotated in Data Structure as exposed via `TESTS_ROOTFS_IMG`; Phase 1's `.PHONY` instruction explicitly says "extend the existing line."

## Log

[**Added**]
- New top-level Make variable `ROOT_FEATURES` (defaults to empty); documented in `[**API Surface**]`.
- New `cargo.mk` extension: `cargo_build` macro appends `--features "$(ROOT_FEATURES)"` to the cargo invocation when `ROOT_FEATURES` is non-empty. Documented in `[**API Surface**]`.
- New marker convention: `src/init.sh` and `src/test.sh` each carry an `# id: starry-<name>` comment near the top; V-UT-8 / V-IT-8 / Phase 1 acceptance assert the marker is present in the kernel ELF.
- Constraint C-16: the kernel-side ID-marker convention.

[**Changed**]
- `[**Build Switch**]` final paragraph: replaces "via the existing `scripts/make/cargo.mk` `features` plumbing — see Implementation Phase 1" with "via a new top-level `ROOT_FEATURES` Make variable threaded into `cargo.mk`'s `cargo_build` macro — see API Surface and Phase 1."
- C-12 wording: makes the Make passthrough mechanism explicit (`ROOT_FEATURES := init-test`).
- Phase 1: replaces "we just need the test path to add `init-test` to the existing list (FEATURES)" with the concrete two-file edit (root `Cargo.toml` adds `[features] init-test = []`; `scripts/make/cargo.mk`'s `cargo_build` macro appends `--features "$(ROOT_FEATURES)"` when non-empty; top-level `Makefile` adds `ROOT_FEATURES ?=`). Acceptance line uses `ROOT_FEATURES=init-test`, not `FEATURES=init-test`.
- Phase 5: `make run-tests` invokes the kernel build with `ROOT_FEATURES=init-test`, not `--features init-test`-as-cargo-args. Phase 5 acceptance refers to the same form.
- V-UT-8 / V-IT-8: assert the ID-marker comment from the relevant script is present in the kernel ELF and the *other* marker is absent. No more "first non-blank line" assertion.
- Phase 1 last bullet reworded: "**Extend** the existing top-level `Makefile` `.PHONY:` line at line 99 to include `tests run-tests`."
- Data Structure: `tests-rootfs-<arch>.img` line annotated as "(exposed as `TESTS_ROOTFS_IMG`; see API Surface)".
- Architecture diagram annotation: `cargo build --features init-test` becomes `make build … ROOT_FEATURES=init-test` (the `--features init-test` still happens internally inside `cargo_build`, but the Make-level surface is `ROOT_FEATURES`).

[**Removed**]
- Reference to "we just need the test path to add `init-test` to the existing list (FEATURES)" in Phase 1.
- The "(or whatever the project's existing feature flag passthrough syntax is)" hedge in Phase 1's acceptance line.
- The "first non-blank line" assertion idiom in V-UT-8 and V-IT-8.

[**Unresolved**]
- Resolved by Phase 0 — see `[**Phase 0 Results**]` below. Both musl crosses are present; no fallback needed.

[**Phase 0 Results**]

Spike completed inside `docker.educg.net/cg/os-contest:20250714` (date: 2026-05-03).

Captured values (also propagated to `[**API Surface**]`'s `xtest/Makefile` public-variables block):

```
DOCKER_IMAGE := docker.educg.net/cg/os-contest@sha256:742479b5cd11b24501e2eccf5d409b78b76ba7aabcb87f815bbd5908a288313b
MUSL_CC_RV64 := /opt/riscv64-linux-musl-cross/bin/riscv64-linux-musl-gcc
MUSL_CC_LA64 := /opt/loongarch64-linux-musl-cross/bin/loongarch64-linux-musl-gcc
```

Notes:
- Both musl crosses are present in the image; the loongarch64 fallback decisions in PLAN 03's Unresolved list are not needed.
- `libc.a` confirmed under each cross's sysroot (`/opt/riscv64-linux-musl-cross/riscv64-linux-musl/lib/libc.a`, `/opt/loongarch64-linux-musl-cross/loongarch64-linux-musl/lib/libc.a`), so `-static` linking against musl will work without external libc lookups.
- Image content size: 5.5 GB (25.5 GB on disk after extraction). First-time pull is multi-GB; CI will need a cached layer or a registry-local mirror.
- The contest image is `linux/amd64`; on Apple Silicon hosts Docker emulates with `qemu-x86_64`, which works but is slow. CI runners that are native amd64 will be much faster — flag for documentation in Phase 6.
- `option_env!` form is *not* used; PLAN 03's chosen mechanism is the cargo-feature `init-test` switch threaded via the new `ROOT_FEATURES` Make variable (R-001 from REVIEW 02). End-to-end verification is deferred to Phase 1's acceptance check (where the switch lands in the actual codebase rather than a scratch edit).

[**Response Matrix**]

| Source | ID | Decision | Resolution |
|--------|----|----------|------------|
| Review 02 | R-001 | Accepted | Option (a) from REVIEW 02's recommendation adopted. New top-level `ROOT_FEATURES` Make variable; `scripts/make/cargo.mk`'s `cargo_build` macro is extended to append `--features "$(ROOT_FEATURES)"` when non-empty. `make tests`/`make run-tests` set `ROOT_FEATURES := init-test`. The existing `FEATURES`/`AX_FEAT` plumbing for `axfeat/`-prefixed features is untouched. Phase 1 commit lists the three concrete file edits (root `Cargo.toml` `[features]`, `cargo.mk` macro extension, `Makefile` `ROOT_FEATURES ?=`). Phase 1 acceptance line uses `ROOT_FEATURES=init-test`. NG-5 is honoured: the new feature ships with its own minimal Make seam. |
| Review 02 | R-002 | Accepted | New constraint C-16: `src/init.sh` and `src/test.sh` carry an `# id: starry-init` / `# id: starry-test` marker comment near the top. V-UT-8 / V-IT-8 / Phase 1 acceptance match against the marker. The "first non-blank line" idiom is removed. The marker is unique per script, so the assertion cannot pass on the wrong configuration. |
| Review 02 | R-003 | Accepted | Phase 1's acceptance hedge "(or whatever the project's existing feature flag passthrough syntax is)" is deleted; replaced with the concrete form `make build ARCH=riscv64 ROOT_FEATURES=init-test` (the form chosen by R-001). |
| Review 02 | R-004 | Accepted | Data Structure's `tests-rootfs-<arch>.img` line annotated with "(exposed as `TESTS_ROOTFS_IMG`; see API Surface)". The path remains in both places (each section uses it for a different audience), but cross-referenced. |
| Review 02 | R-005 | Accepted | Phase 1's `.PHONY` bullet reworded to "**Extend** the existing top-level `Makefile` `.PHONY:` line at line 99 to include `tests run-tests`." |
| Review 02 | TR-1 | Accepted | T-1 unchanged. |
| Review 02 | TR-2 | Accepted | T-2 unchanged. |
| Review 02 | TR-3 | Accepted | T-3' unchanged in form (cargo-feature); the Make-side passthrough specifics land via R-001. |
| Review 02 | TR-4 | Accepted | T-4 unchanged. |
| Review 02 | TR-5 | Accepted | T-5 unchanged. |
| Review 02 | TR-6 | Accepted | T-6 unchanged. |

---

## Spec

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

## Runtime

[**Main Flow**] — `make run-tests ARCH=riscv64`

1. Top-level `Makefile`'s `run-tests` target depends on `tests` and on a `build` invocation that sets `ROOT_FEATURES=init-test`.
2. `tests` target: ensure upstream `rootfs-$ARCH.img` exists (delegate to existing `qemu_rootfs` logic factored into a shared make function), then `docker run … make -C xtest all ARCH=$(ARCH)`.
3. Inside Docker, `xtest/Makefile`'s `all` runs `build-c → build-suites → stage → bake-image`:
   a. `build-c.sh` finds every `xtest/c/**/*.c`, compiles each with `$(MUSL_CC_<ARCH>) -static -I xtest/c/common -O2 -o xtest/build/<arch>/c/<basename>` (per C-10).
   b. `build-suites.sh` iterates `xtest/testsuites/*/`; for each suite, dispatches per the per-suite recipe contract.
   c. `stage.sh` assembles `xtest/build/<arch>/stage/root/tests/{c,testsuites,scripts}` from the build outputs plus `xtest/scripts/run-*.sh` and `xtest/scripts/lib/`.
   d. `bake-image.sh` copies `rootfs-$ARCH.img` to `tests-rootfs-$ARCH.img`, loop-mounts it, `rsync -a` the staged tree into `/root/tests`, `umount`s, releases the loop device. **No init script is installed.**
4. Back on the host, `run-tests` invokes `make build ARCH=... ROOT_FEATURES=init-test`. The extended `cargo_build` macro appends `--features init-test` to its cargo invocation. `src/main.rs`'s `#[cfg(feature = "init-test")]` arm activates, so `include_str!("test.sh")` embeds `src/test.sh` (carrying its `# id: starry-test` marker) into the kernel ELF.
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
8. **`src/test.sh` is missing when `ROOT_FEATURES=init-test`:** `cargo build` fails at compile time because `include_str!("test.sh")` cannot resolve. The error names the missing path.
9. **`ROOT_FEATURES` set to a non-existent feature** (e.g. typo `init-tst`): `cargo build` fails with `error: package 'starry' does not have feature 'init-tst'`. Make exits non-zero.

[**State Transitions**]

- `xtest/build/<arch>/` empty → populated, by `build-c` + `build-suites`.
- Build outputs → `stage/`, by `stage.sh` (rsync-style copy with stable file modes).
- `stage/` + `rootfs-$ARCH.img` → `tests-rootfs-$ARCH.img`, by `bake-image.sh`.
- Idle disk → mounted under `mnt/` → unmounted, inside `bake-image.sh` (always restored on failure via trap).
- Kernel ELF (default, `ROOT_FEATURES` unset) → kernel ELF (test-build, `ROOT_FEATURES=init-test`).
- Old `xtest/Makefile`, `Makefile.sub`, `config/`, `scripts/git_testcode.sh` exist → deleted, by Phase 1.

---

## Implementation

[**Phase 0 — Toolchain Spike (single throwaway Docker session, no commits)**]

- Run `docker run --rm -it docker.educg.net/cg/os-contest:20250714 bash`; inside:
  - `find / -name 'gcc' 2>/dev/null` and filter to musl-targeted entries.
  - `find / -name 'libc.a' 2>/dev/null` for both arches.
  - From the host: `docker inspect --format '{{index .RepoDigests 0}}' docker.educg.net/cg/os-contest:20250714` to capture the digest.
- Confirm the Make-side `ROOT_FEATURES` passthrough compiles end-to-end: in a scratch checkout, edit `src/main.rs` to the two-arm `#[cfg(feature = "init-test")]` form; add `[features] init-test = []` to root `Cargo.toml`; extend `scripts/make/cargo.mk`'s `cargo_build` macro per `[**API Surface**]`; add `ROOT_FEATURES ?=` to top-level `Makefile`. Then run `make build ARCH=riscv64` (no `ROOT_FEATURES`) and `make build ARCH=riscv64 ROOT_FEATURES=init-test`. Both must succeed; the embedded `# id:` marker in the resulting ELFs must differ (`starry-init` vs `starry-test`, verified with `strings | grep`).
- Record results into the PLAN's `[**API Surface**]` (`MUSL_CC_RV64`, `MUSL_CC_LA64`, `DOCKER_IMAGE` digest). **Mechanism:** Phase 0 results are appended to this PLAN's `## Log` under a new `[**Phase 0 Results**]` heading; the placeholder strings in `[**API Surface**]` are replaced in place. No new PLAN iteration is required — Phase 0 produces only factual capture, not design change.
- If loongarch64 musl cross is missing: pick one of (a) install during Phase 2, (b) accept libgcc on Alpine via the existing `Makefile.sub` ld-musl symlink trick, (c) drop loongarch C tests from G-3/G-9. Record the choice in the same `[**Phase 0 Results**]` heading.

Acceptance for Phase 0: `[**API Surface**]` has concrete strings for `MUSL_CC_RV64`, `MUSL_CC_LA64`, `DOCKER_IMAGE` digest; `[**Phase 0 Results**]` exists in `## Log`. End-to-end `ROOT_FEATURES=init-test` switch confirmed: `strings $(OUT_ELF) | grep '# id: starry-test'` matches when the feature is set, `# id: starry-init` matches when it isn't, and the wrong marker is absent in each case. No code committed yet.

[**Phase 1 — demolition + skeleton + Make seam (single commit)**]

- Delete `xtest/Makefile` (old), `xtest/Makefile.sub`, `xtest/config/`, `xtest/scripts/git_testcode.sh`.
- Verify with `git grep`: nothing in the repo references `Makefile.sub`, `busybox-config-`, `git_testcode`, `sdcard-rv.img`, or `sdcard-la.img`.
- Create new `xtest/` skeleton: empty `c/`, `testsuites/`, `scripts/{build,lib}/`, plus `xtest/Makefile`, `xtest/README.md`, `xtest/.gitignore` (ignores `build/`), `xtest/testsuites/UPSTREAM.md` (placeholder pending Phase 3).
- Add `src/test.sh` (placeholder that just `exec sh`s — full logic lands in Phase 4) carrying `# id: starry-test` near the top.
- Add `# id: starry-init` near the top of `src/init.sh` (the only edit to this file in the task — see G-12).
- Make-side seam (the three concrete edits per [**Build Switch**] / [**API Surface**]):
  1. Root `Cargo.toml` gains `[features] init-test = []`.
  2. `src/main.rs` line 64 gains the two `#[cfg(feature = "init-test")]`-gated `include_str!` arms.
  3. `scripts/make/cargo.mk`'s `cargo_build` macro is extended to append `--features "$(ROOT_FEATURES)"` when `ROOT_FEATURES` is non-empty.
  4. Top-level `Makefile` gains `ROOT_FEATURES ?=` near the existing `FEATURES ?=` line.
- Add top-level `make tests` / `make run-tests` targets (no-ops that print "not yet implemented" and exit 0; wired up properly in Phase 5).
- **Extend** the existing top-level `Makefile` `.PHONY:` line at line 99 to include `tests run-tests` (do not add a second `.PHONY:` line).

Acceptance for Phase 1: `git grep -E 'Makefile\.sub|busybox-config-|git_testcode|sdcard-rv|sdcard-la' -- ':!src/init.sh' ':!arceos/'` returns no matches; `make tests` and `make run-tests` resolve as targets (`make -n tests` succeeds); `make build ARCH=riscv64` (no `ROOT_FEATURES`) produces a kernel ELF whose `strings` output contains `# id: starry-init` and not `# id: starry-test`; `make build ARCH=riscv64 ROOT_FEATURES=init-test` produces a kernel ELF whose `strings` output contains `# id: starry-test` and not `# id: starry-init`.

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

Acceptance for Phase 3: `make -C xtest all ARCH=riscv64` (and la) build inside Docker without errors and stage every suite plus all C tests. `git status` shows no build artifacts. Every `xtest/testsuites/<s>/` contains at least one of `{LICENSE, COPYING, COPYING.LIB, NOTICE, COPYRIGHT}`.

[**Phase 4 — image baking + `src/test.sh`**]

- Implement `xtest/scripts/build/bake-image.sh` (cp rootfs, mount, rsync, umount, with trap-based cleanup; per the contract — no `TEST_SH` input).
- Wire `xtest/Makefile`'s `bake-image` target.
- Replace the placeholder `src/test.sh` with the real one (env exports, run-all dispatch, exec sh fallback). Keep the `# id: starry-test` marker near the top. Lint with `dash -n`.

Acceptance for Phase 4: `make -C xtest bake-image ARCH=riscv64` produces `xtest/build/riscv64/tests-rootfs-riscv64.img`. Loop-mounting it manually shows `/root/tests/{c,testsuites,scripts}` populated and **no** `/test.sh` file at the root (per the contract).

[**Phase 5 — top-level wiring + qemu.mk refactor + boot smoke**]

- Refactor `scripts/make/qemu.mk`: extract the QEMU invocation into `run_qemu_with_disk = ... $(1) ...`. `run_qemu` becomes `$(call run_qemu_with_disk,$(DISK_IMG))`. Add `run_qemu_tests = $(call run_qemu_with_disk,$(TESTS_ROOTFS_IMG))`. Confirm `make -n run` is byte-identical (modulo whitespace) before and after the refactor.
- Promote `make tests` from Phase 1 placeholder to a real target that delegates to `xtest/Makefile`.
- Promote `make run-tests` from placeholder to: build-test-image + `make build ARCH=... ROOT_FEATURES=init-test` + `$(call run_qemu_tests)`.
- Factor the upstream-rootfs-fetch logic out of `qemu_rootfs` into a reusable make function so both `qemu_rootfs` (existing) and `tests` (new) call it.
- Boot smoke on `riscv64` and `loongarch64` (`make run-tests ARCH=...`); confirm the embedded `test.sh` runs, the C tests print PASS/FAIL, the `basic` suite prints its group header/footer with per-test results, and the run lands in an interactive `sh` afterwards.

Acceptance for Phase 5: G-7 + G-9 satisfied; both arches boot the test rootfs under the test-built kernel, run the suites, never abort, and drop to a shell. `make -n run` is byte-identical to its pre-refactor output. `make run` still works and its kernel ELF still embeds `src/init.sh` (matched via `# id: starry-init`).

[**Phase 6 — documentation + verify pass**]

- Write `xtest/README.md` (what xtest is, how to run it, how to add a C test, how to add a suite, Docker requirement, image digest, `ROOT_FEATURES` usage).
- Update `AGENTS.md` "Testing" section to reference `make tests` / `make run-tests` and Docker dependency.
- Fill `VERIFY.md` against the PRD's Outcome bullets and the Acceptance Mapping below.

Acceptance for Phase 6: documentation merged; VERIFY checklist all non-PENDING.

---

## Trade-offs

- T-1: **Vendor upstream suites vs. submodule vs. fetch-on-build.** Chosen: vendor (per user direction; reviewer TR-1 confirms across all iterations). Adv.: hermetic clones, offline builds, simple contributor workflow. Disadv.: large repo size; upstream sync is a manual diff. Provenance hardened by C-13.
- T-2: **Bake on top of upstream rootfs vs. build a new rootfs from scratch.** Confirmed: bake on top per reviewer TR-2 guidance. Adv.: minimal new build infrastructure; reuses Alpine, busybox, musl exactly as today. Disadv.: every test rootfs build re-copies a multi-MB image (negligible on modern disks).
- T-3': **Build-time switch — cargo-feature form (the only one that compiles given `include_str!`'s literal-only argument grammar).** Chosen: cargo-feature form. Adv.: keeps `make run` byte-identical (G-8 provable post-marker addition); isolates all test-rig changes inside the test build; matches the project's existing cargo-feature idiom; one-line `Cargo.toml` change. Disadv.: two kernel binaries (one with `init.sh`, one with `test.sh`) — acceptable since they're per-target outputs anyway.
- T-3'': **Make-side passthrough for the cargo feature — new `ROOT_FEATURES` variable vs. special-case `init-test` inside `features.mk` vs. generic `EXTRA_CARGO_ARGS`.** Chosen: new `ROOT_FEATURES` variable (option (a) from REVIEW 02 R-001). Adv.: minimal surface; preserves the existing `axfeat/`-prefixing for everything else; easy for the executor to land in one Phase 1 commit; cleanly extensible to other root-crate features without new magic. Disadv.: introduces a second feature variable alongside `FEATURES` — a small documentation cost mitigated by the comment in `[**API Surface**]`. Special-casing inside `features.mk` was rejected as too magical; a generic `EXTRA_CARGO_ARGS` was rejected as too broad (anything could be smuggled in, weakening the contract).
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
- V-UT-5: `git grep -E 'Makefile\.sub|busybox-config-|git_testcode|sdcard-rv|sdcard-la' -- ':!src/init.sh' ':!arceos/'` over the repo returns no matches after Phase 1 (G-2).
- V-UT-6: `git status --porcelain xtest/build/` is empty after `make -C xtest all ARCH=riscv64` (C-2).
- V-UT-7: For every `xtest/testsuites/<s>/`, the directory contains at least one of `{LICENSE, COPYING, COPYING.LIB, NOTICE, COPYRIGHT}` and `xtest/testsuites/UPSTREAM.md` has a row for that suite with non-empty `License (SPDX)` and `Local patches` cells (C-3 + C-13).
- V-UT-8: Build switch unit — `make build ARCH=riscv64` (no `ROOT_FEATURES`) produces a kernel ELF where `strings $(OUT_ELF) | grep -F '# id: starry-init'` returns one match and `strings $(OUT_ELF) | grep -F '# id: starry-test'` returns none. `make build ARCH=riscv64 ROOT_FEATURES=init-test` produces a kernel ELF where the markers are inverted (test present, init absent). G-11, C-12, C-16.
- V-UT-9: `grep -F 'make tests' AGENTS.md` and `grep -F 'make run-tests' AGENTS.md` both return non-empty after Phase 6; `xtest/README.md` exists and is non-empty (G-10).
- V-UT-10: `grep -E 'docker\.educg\.net/cg/os-contest@sha256:[a-f0-9]{64}' xtest/Makefile` returns one match; `grep -E 'docker\.educg\.net/cg/os-contest:[0-9]+' xtest/Makefile` (tag form) returns no matches (C-14).
- V-UT-11: `src/init.sh` and `src/test.sh` each contain exactly one `# id:` line; the values are `starry-init` and `starry-test` respectively; neither marker appears in the other file (C-16).

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
- V-IT-6a: `git diff main..HEAD -- src/init.sh` shows exactly one added line containing `# id: starry-init` and no other changes (G-8 + G-12).
- V-IT-7: `make -n run ARCH=riscv64` and `make -n run-tests ARCH=riscv64` produce QEMU command lines that differ **only** in the `-drive file=` argument; same `BLK`/`NET`/`MEM`/`LOG` plumbing and identical other flags (C-15, C-5).
- V-IT-8: After `make build ARCH=riscv64` (no `ROOT_FEATURES`), `strings $(OUT_ELF) | grep -F '# id: starry-init'` matches and `strings $(OUT_ELF) | grep -F '# id: starry-test'` does not. After `make build ARCH=riscv64 ROOT_FEATURES=init-test`, the matches invert (G-8 + G-11 + C-16).

[**Failure / Robustness Validation**]

- V-F-1: A first-party C test that `exit(1)`s prints `[FAIL] <name> exit=1` and the run continues to subsequent tests (C-8b).
- V-F-2: A first-party C test that segfaults prints `[FAIL] <name> signal=SEGV` (or equivalent) and the run continues (C-8b).
- V-F-3: A suite whose `run.sh` `sleep 9999`s is killed by `lib/timeout.sh` after the configured timeout; `[TIMEOUT] <suite>` is logged; the run continues (C-8b).
- V-F-4: `bake-image.sh` interrupted (SIGTERM) mid-rsync leaves no mounted loop device, no partial output image — verified by re-running `mount` and `losetup -a` after.
- V-F-5: `make tests` with Docker uninstalled prints the documented "docker not found — install Docker and pull <image-url>" error and exits non-zero (C-1, no half-built artifacts).
- V-F-6: After temporarily removing `src/test.sh`, `make build ARCH=riscv64 ROOT_FEATURES=init-test` fails at compile time with `include_str!` reporting the missing path (Failure Flow item 8).
- V-F-7: `make build ARCH=riscv64 ROOT_FEATURES=init-tst` (typo) fails with cargo's "package does not have feature" error and Make exits non-zero (Failure Flow item 9).

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
| G-8 (`make run` unchanged post-marker) | V-IT-6a, V-IT-8 |
| G-9 (smoke on rv + la)                | V-IT-4, V-IT-5, V-F-1, V-F-2, V-F-3 |
| G-10 (documentation)                  | V-UT-9 |
| G-11 (build switch)                   | V-UT-8, V-IT-8, V-F-6, V-F-7 |
| G-12 (init-script ID markers)         | V-UT-11, V-IT-6a, V-IT-8 |
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
| C-12 (build switch — cargo feature + ROOT_FEATURES seam) | V-UT-8, V-IT-8, V-F-6, V-F-7 |
| C-13 (per-suite license preservation) | V-UT-7 |
| C-14 (Docker image digest pin)        | V-UT-10, V-IT-1 |
| C-15 (qemu.mk shared macro)           | V-IT-7 |
| C-16 (init-script ID markers)         | V-UT-8, V-UT-11, V-IT-8 |
