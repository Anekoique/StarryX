# `xtest` PLAN `00`

> Status: Draft
> Feature: `xtest`
> Iteration: `00`
> Owner: Executor
> Depends on:
> - Previous Plan: `none`
> - Review: `none`
> - Master Directive: `none`

---

## Summary

Tear down the old `xtest/` sdcard pipeline and replace it with a test-rootfs pipeline. The new `xtest/` ships first-party C tests (`xtest/c/`), a vendored subset of `oscomp/testsuites-for-oskernel @ pre-2025` (`xtest/testsuites/`), and shell drivers (`xtest/scripts/`). A new `xtest/Makefile` cross-builds everything inside the contest Docker image (`docker.educg.net/cg/os-contest:20250714`), then bakes the build artifacts plus a kernel-side `src/test.sh` boot script into a copy of the upstream Alpine `rootfs-$ARCH.img`, producing `tests-rootfs-$ARCH.img`. Top-level `make tests` builds that image; `make run-tests` builds it and boots StarryX against it. `make run` and `src/init.sh` are unchanged.

## Log

*None in 00_PLAN.*

---

## Spec

[**Goals**]

- G-1: `xtest/` becomes the in-repo test environment producer; layout is `xtest/{c,testsuites,scripts}` (no other top-level dirs).
- G-2: Every removable piece of the old sdcard pipeline (`xtest/Makefile` (old), `xtest/Makefile.sub`, `xtest/config/`, `xtest/scripts/git_testcode.sh`) is deleted in this task; nothing in the repo references them after the change.
- G-3: First-party C tests live under `xtest/c/` as one `.c` per test; each compiles to one statically-linked ELF using the cross toolchains in the contest Docker image. A `xtest/scripts/run-c.sh` driver iterates them and reports pass/fail.
- G-4: A vendored subset of `oscomp/testsuites-for-oskernel @ pre-2025` lives under `xtest/testsuites/<suite>/`, one directory per suite. Sources are committed directly (no submodule, no fetch-on-build). Pinned upstream commit is recorded in `xtest/testsuites/UPSTREAM.md`.
- G-5: `src/test.sh` is a kernel-side boot script symmetric to `src/init.sh`. It sets `PATH=/bin:/sbin:/usr/bin:/usr/sbin`, `LD_LIBRARY_PATH=/lib:/usr/lib`, `HOME=/root`, `cd /root/tests`, runs `./scripts/run-all.sh`, then drops to `sh`.
- G-6: `make tests ARCH={riscv64|loongarch64}` produces `tests-rootfs-$ARCH.img` deterministically (same inputs → same staged tree; image bytes may differ due to ext4 timestamps, which is acceptable).
- G-7: `make run-tests ARCH=...` boots StarryX with `tests-rootfs-$ARCH.img` mounted as the root disk. The kernel ELF / boot path is the existing one; only the disk image and the in-image init script differ from `make run`.
- G-8: `make run` and `src/init.sh` are byte-for-byte unchanged after the task lands.
- G-9: On both `riscv64` and `loongarch64`, end-to-end smoke succeeds: the test rootfs boots, `run-all.sh` executes the `basic` suite plus all first-party C tests, and per-test pass/fail lines appear on the serial console. A test failing does **not** abort the run.

- NG-1: Not building or modifying cross toolchains. We assume the contest Docker image already provides `riscv64-linux-gnu-*`, `loongarch64-linux-gnu-*`, and the musl variants used today.
- NG-2: Not rebuilding the Alpine rootfs from scratch. We bake on top of `Starry-OS/rootfs/rootfs-$ARCH.img` (same source the existing `make qemu_rootfs` already downloads).
- NG-3: Not maintaining glibc-side test variants. Alpine is musl-only; `xtest/c/` and the vendored suites target musl. (The vendored upstream sources may still contain glibc bits — we just don't build/run them.)
- NG-4: Not introducing tier groupings (preliminary / final1 / final2). `run-all.sh` runs everything sequentially in a stable order; if tiers are ever needed they're a follow-up.
- NG-5: Not changing how the kernel is built (`scripts/make/build.mk`, target features, axconfig flow). `make tests` reuses the existing `build` target unchanged.
- NG-6: Not removing or repurposing `qemu_rootfs` / the upstream rootfs download. `make run` keeps using it.
- NG-7: Not adding host-OS support beyond what the contest Docker image already enables (Linux + Docker, or anything that can run that image). macOS hosts go through Docker — no native macOS path.

[**Architecture**]

```
HOST (any system that can run the contest Docker image)
┌──────────────────────────────────────────────────────────────────┐
│ make tests ARCH=riscv64                                           │
│   └─ docker run … docker.educg.net/cg/os-contest:20250714 \       │
│        make -C xtest build-all ARCH=riscv64                       │
│         ├─ build-c        → xtest/build/<arch>/c/<name>           │
│         ├─ build-suites   → xtest/build/<arch>/testsuites/<s>/…   │
│         └─ stage          → xtest/build/<arch>/stage/root/tests/  │
│                              ├─ c/                                │
│                              ├─ testsuites/                       │
│                              └─ scripts/                          │
│      then bake-image:                                             │
│        cp rootfs-<arch>.img tests-rootfs-<arch>.img               │
│        mount tests-rootfs-<arch>.img → mnt/                       │
│        rsync stage/* into mnt/                                    │
│        install src/test.sh → mnt/test.sh (mode 0755)              │
│        umount, release                                            │
└──────────────────────────────────────────────────────────────────┘
                              │
                              ▼
GUEST (StarryX + tests-rootfs-<arch>.img as virtio-blk root)
┌──────────────────────────────────────────────────────────────────┐
│ kernel boot → user init → /test.sh                                │
│   ├─ export PATH / LD_LIBRARY_PATH / HOME                         │
│   ├─ cd /root/tests                                               │
│   ├─ ./scripts/run-all.sh                                         │
│   │     ├─ ./scripts/run-c.sh           (first-party C tests)     │
│   │     └─ for s in testsuites/*; do                              │
│   │          ./scripts/run-suite.sh "$s"                          │
│   │       done   (per-test pass/fail; no contest markers)         │
│   └─ exec sh   (post-test interactive shell)                      │
└──────────────────────────────────────────────────────────────────┘
```

Module decoupling:
- **Build side** (`xtest/Makefile` + `xtest/scripts/build/*.sh`) only knows about cross-compilation and image baking. It does not embed test logic.
- **Runtime side** (`xtest/scripts/run-*.sh` + `src/test.sh`) only knows how to discover and execute test binaries and emit OS-COMP markers. It does not know how anything was built.
- The bridge between them is the **staging contract**: `xtest/build/<arch>/stage/root/tests/{c,testsuites,scripts}` has a fixed shape, and `bake-image` copies it verbatim to `/root/tests` in the image.

Top-level Makefile integration is additive. `scripts/make/qemu.mk` gains a `run_qemu_tests` macro analogous to `run_qemu`, differing only in the `-drive` pointing at `tests-rootfs-$ARCH.img`. Top-level Makefile gains `tests` and `run-tests` targets that delegate to `xtest/Makefile` for the image build and to `run_qemu_tests` for the boot.

[**Data Structure**]

```
xtest/
├── Makefile                       # build pipeline (top of xtest)
├── README.md                      # what this is, how to run it
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
│   ├── UPSTREAM.md                # upstream URL + pinned commit + import date
│   ├── basic/                     # vendored from oscomp pre-2025
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
│   │   ├── build-c.sh             # in-Docker: compile xtest/c/**/*.c
│   │   ├── build-suites.sh        # in-Docker: build each suite
│   │   ├── stage.sh               # assemble xtest/build/<arch>/stage/
│   │   └── bake-image.sh          # cp rootfs, mount, rsync stage, install test.sh, umount
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
├── init.sh        # unchanged
└── test.sh        # NEW — boot script for the test rootfs
```

`task.toml`-equivalent for the build side is the per-suite recipe: every suite under `xtest/testsuites/<s>/` is expected to expose either a `Makefile` (preferred — `build-suites.sh` calls `make -C testsuites/<s> ARCH=$ARCH OUT=...`) or a `BUILD.sh` script (fallback — invoked the same way). Either must drop its outputs under `xtest/build/<arch>/testsuites/<s>/`. This contract lets us vendor each suite with minimal local patches and lets `build-suites.sh` stay a thin loop.

[**API Surface**]

Top-level `Makefile` — new public targets:

```
make tests           ARCH=riscv64|loongarch64    # build tests-rootfs-$ARCH.img
make run-tests       ARCH=riscv64|loongarch64    # build + boot in QEMU
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

`xtest/scripts/build/bake-image.sh` contract:

```
Inputs:
  ARCH                         riscv64 | loongarch64
  ROOTFS_IMG                   path to upstream rootfs-$ARCH.img
  STAGE_DIR                    path to xtest/build/<arch>/stage/
  TEST_SH                      path to src/test.sh
  OUT_IMG                      path to write tests-rootfs-$ARCH.img
Outputs:
  $OUT_IMG: copy of $ROOTFS_IMG with $STAGE_DIR/root/tests/* copied to /root/tests
            and $TEST_SH installed as /test.sh (mode 0755).
Side effects:
  Mount/umount under $XTEST_BUILD/<arch>/mnt/ inside the Docker container.
```

`src/test.sh` contract (POSIX `sh`, runs as the user-space init replacement under StarryX):

```
- exports PATH, LD_LIBRARY_PATH, HOME
- cds to /root/tests
- runs ./scripts/run-all.sh; never aborts on suite failure
- on completion, exec sh
```

`xtest/scripts/run-all.sh` contract:

```
- runs ./scripts/run-c.sh
- runs ./scripts/run-suite.sh for each subdirectory of testsuites/
- prints a clear group header / footer around each subgroup
  (e.g. "==== c ====" ... "==== c done ====")
- collects per-test pass/fail via exit status, never propagates a non-zero exit
```

[**Constraints**]

- C-1: All cross-compilation runs **inside** `docker.educg.net/cg/os-contest:20250714`. Host requirements are Docker + GNU make + a POSIX shell. No host toolchain assumptions beyond that.
- C-2: `xtest/build/` is gitignored. No build outputs are committed.
- C-3: `xtest/testsuites/` is **vendored**: the upstream sources are committed verbatim. `xtest/testsuites/UPSTREAM.md` records the upstream repo URL, pinned commit, import date, and a one-line note per suite if local patches were applied.
- C-4: No top-level `Makefile` variable rename or removal. New variables introduced (e.g. `TESTS_ROOTFS_IMG`) follow the existing `ROOTFS_IMG` naming style.
- C-5: `make tests` / `make run-tests` must accept the same `ARCH` / `BLK` / `NET` / `MEM` / `LOG` overrides that `make rv` / `make la` accept; their internals reuse `scripts/make/qemu.mk`.
- C-6: `src/test.sh` is POSIX `sh` (Alpine ash-compatible) — no bashisms. Lints clean under `dash -n`.
- C-7: `bake-image.sh` requires `--privileged` for loop-mount, matching what the existing `xtest/Makefile`'s `docker` target already does. The new `xtest/Makefile`'s `docker run` invocation passes `--privileged` and mounts the repo at `/code` exactly as today.
- C-8: A failing test never aborts the run. `run-*.sh` always exits 0 to keep the boot script flowing into the post-test `sh`.
- C-9: `xtest/c/` test names are unique across subdirectories (we flatten to `c/<name>` in the staged tree, so two `mmap.c` in different subdirs are a build error).
- C-10: First-party C tests link statically against musl. `build-c.sh` invokes `${PREFIX}gcc -static` with the musl cross prefix. They must run on Alpine without dynamic-loader gymnastics.

---

## Runtime

[**Main Flow**] — `make run-tests ARCH=riscv64`

1. Top-level `Makefile`'s `run-tests` target depends on `tests` and `build`.
2. `tests` target: ensure upstream `rootfs-$ARCH.img` exists (delegate to existing `qemu_rootfs` logic factored into a shared make function), then `docker run … make -C xtest all ARCH=$(ARCH)`.
3. Inside Docker, `xtest/Makefile`'s `all` runs `build-c → build-suites → stage → bake-image`:
   a. `build-c.sh` finds every `xtest/c/**/*.c`, compiles each with `${PREFIX}gcc -static -I xtest/c/common -O2 -o xtest/build/<arch>/c/<basename>`.
   b. `build-suites.sh` iterates `xtest/testsuites/*/`; for each suite directory, if a `Makefile` exists, runs `make -C` it with `ARCH`, `PREFIX`, `OUT=xtest/build/<arch>/testsuites/<s>`; else if `BUILD.sh` exists, runs it with the same env; else copies the directory verbatim (for script-only suites).
   c. `stage.sh` assembles `xtest/build/<arch>/stage/root/tests/{c,testsuites,scripts}` from the build outputs plus `xtest/scripts/run-*.sh` and `xtest/scripts/lib/`.
   d. `bake-image.sh` copies `rootfs-$ARCH.img` to `tests-rootfs-$ARCH.img`, loop-mounts it, `rsync -a` the staged tree into `/root/tests`, installs `src/test.sh` to `/test.sh` (mode 0755), `umount`s, releases the loop device.
4. Back on the host, `run-tests` invokes the existing kernel build (`make build ARCH=...`) and then a new `run_qemu_tests` macro that runs QEMU with `-drive file=tests-rootfs-$ARCH.img,...` instead of `disk.img`.
5. StarryX boots, mounts the test rootfs, the user-space init dispatches to `/test.sh` (the kernel boot config is unchanged — `/test.sh` is what the rootfs's own init points at; we install it as the script the existing init already invokes).
6. `/test.sh` exports env, `cd /root/tests`, runs `./scripts/run-all.sh`, then `exec sh`.
7. `run-all.sh` runs `run-c.sh` (iterates `c/*` ELFs, prints `[PASS] <name>` / `[FAIL] <name> exit=<n>`), then for each suite dir runs `run-suite.sh <suite>` which prints a plain group header / footer around the suite's invocation entry (suite-specific `run.sh` baked into `testsuites/<s>/run.sh`).
8. After all suites, control returns to `/test.sh`, which `exec sh`s into an interactive shell.

[**Failure Flow**]

1. **Docker not installed / image not pullable on host:** `xtest/Makefile`'s top guard checks `command -v docker`; if missing, prints a clear error pointing to the contest image URL and exits non-zero. `make tests` fails fast.
2. **Cross compile fails for a single C test:** `build-c.sh` collects errors and exits non-zero with a summary; the build aborts so the issue is caught before image baking.
3. **Suite build fails:** `build-suites.sh` records the failing suite, prints its error, and continues. Final exit status is non-zero so the image isn't baked, but every suite is attempted so contributors see all build issues at once.
4. **Image baking fails (mount, rsync, umount):** `bake-image.sh` traps and `umount`s, releases the loop device, deletes the partial output image, and exits non-zero.
5. **A test binary crashes / segfaults at runtime:** `run-c.sh` / `run-suite.sh` capture exit status, print `[FAIL] <name> exit=<n>` (or signal name for signaled exits), and continue to the next test. The full run completes.
6. **A suite hangs:** `lib/timeout.sh` wraps invocations with a per-suite timeout (default 600s, overridable by suite-local `TIMEOUT` env). On timeout, `[TIMEOUT] <name>` is printed and execution continues.
7. **`run-all.sh` itself fails:** `/test.sh` does not `set -e`; it always falls through to `exec sh`, so the user retains an interactive shell to investigate.

[**State Transitions**]

- `xtest/build/<arch>/` empty → populated, by `build-c` + `build-suites`.
- Build outputs → `stage/`, by `stage.sh` (rsync-style copy with stable file modes).
- `stage/` + `rootfs-$ARCH.img` → `tests-rootfs-$ARCH.img`, by `bake-image.sh`.
- Idle disk → mounted under `mnt/` → unmounted, inside `bake-image.sh` (always restored on failure via trap).
- Old `xtest/Makefile`, `Makefile.sub`, `config/`, `scripts/git_testcode.sh` exist → deleted, by Phase 1 of Implementation.

---

## Implementation

[**Phase 1 — demolition + skeleton (single commit)**]

- Delete `xtest/Makefile` (old), `xtest/Makefile.sub`, `xtest/config/`, `xtest/scripts/git_testcode.sh`.
- Create new `xtest/` skeleton: empty `c/`, `testsuites/`, `scripts/{build,lib}/`, plus `xtest/Makefile`, `xtest/README.md`, `xtest/.gitignore` (ignores `build/`), `xtest/testsuites/UPSTREAM.md` (placeholder pending Phase 3).
- Add `src/test.sh` (placeholder that just `exec sh`s — full logic lands in Phase 4).
- Add top-level `make tests` / `make run-tests` targets (no-ops that print "not yet implemented" and exit 0; wired up properly in Phase 5). This keeps the Makefile diff visible from day one even if the image build isn't online yet.
- Top-level `Makefile` `.PHONY` line includes `tests run-tests`.

Acceptance for Phase 1: `git grep -l 'Makefile.sub\|busybox-config-\|git_testcode'` returns nothing; `make tests` and `make run-tests` resolve as targets (`make -n tests` succeeds).

[**Phase 2 — first-party C tests + build pipeline (host + Docker)**]

- Implement `xtest/scripts/build/build-c.sh` (in-Docker; finds all `.c` under `xtest/c/`, compiles each statically with the musl cross prefix, drops ELFs in `xtest/build/<arch>/c/`).
- Implement `xtest/scripts/build/stage.sh` and a partial `xtest/Makefile` (`build-c`, `stage`, `clean` targets only; `docker-shell` for debugging).
- Add 3–5 first-party C tests under `xtest/c/syscall/` (e.g. `getpid.c`, `clone_basic.c`, `mmap_anon.c`, `open_close.c`, `kill_self.c`) plus `common/assert.h`.
- Implement `xtest/scripts/run-c.sh` that iterates `/root/tests/c/*` ELFs and prints PASS/FAIL.
- `make -C xtest build-c stage ARCH=riscv64` and `ARCH=loongarch64` both succeed inside Docker; `xtest/build/<arch>/stage/root/tests/c/` contains the ELFs.

Acceptance for Phase 2: build-only — both ARCH staging dirs populate without errors. No image yet.

[**Phase 3 — vendor upstream test suites**]

- Clone `https://github.com/oscomp/testsuites-for-oskernel` at the `pre-2025` branch tip, identify the suite subdirectories listed under `xtest/testsuites/` in the layout above, and copy each into `xtest/testsuites/<suite>/`. Strip `.git`. Record the source URL, pinned commit, import date, and any per-suite local patches in `xtest/testsuites/UPSTREAM.md`.
- Implement `xtest/scripts/build/build-suites.sh` (per-suite Makefile/BUILD.sh dispatch, copy-only fallback for script-only suites).
- Implement `xtest/scripts/run-suite.sh` and `xtest/scripts/lib/timeout.sh`.
- Wire `xtest/Makefile`'s `build-suites` and `all` targets.

Acceptance for Phase 3: `make -C xtest all ARCH=riscv64` (and la) build inside Docker without errors and stage every suite plus all C tests. `git status` shows no build artifacts (gitignore works).

[**Phase 4 — image baking + `src/test.sh`**]

- Implement `xtest/scripts/build/bake-image.sh` (cp rootfs, mount, rsync, install test.sh, umount, with trap-based cleanup).
- Wire `xtest/Makefile`'s `bake-image` target.
- Replace the placeholder `src/test.sh` with the real one (env exports, run-all dispatch, exec sh fallback). Lint with `dash -n`.

Acceptance for Phase 4: `make -C xtest bake-image ARCH=riscv64` produces `xtest/build/riscv64/tests-rootfs-riscv64.img`. Loop-mounting it manually shows `/root/tests/{c,testsuites,scripts}` and `/test.sh`.

[**Phase 5 — top-level wiring + boot smoke**]

- Promote `make tests` / `make run-tests` from Phase 1 placeholders to real targets that delegate to `xtest/Makefile` and call a new `run_qemu_tests` macro in `scripts/make/qemu.mk` (a parameterised variant of `run_qemu` that swaps the `-drive` to `tests-rootfs-$ARCH.img`).
- Factor the upstream-rootfs-fetch logic out of `qemu_rootfs` into a reusable make function so both `qemu_rootfs` (existing) and `tests` (new) call it.
- Boot smoke on `riscv64` and `loongarch64` (`make run-tests ARCH=riscv64` / `ARCH=loongarch64`); confirm `/test.sh` runs, the C tests print PASS/FAIL, the `basic` suite prints its group header/footer with per-test results, and the run lands in an interactive `sh` afterwards.

Acceptance for Phase 5: G-9 satisfied; both arches boot the test rootfs, run the suites, never abort, and drop to a shell.

[**Phase 6 — documentation + verify pass**]

- Write `xtest/README.md` (what xtest is, how to run it, how to add a C test, how to add a suite).
- Update `AGENTS.md`'s Testing section to reference `make tests` / `make run-tests` and mention `xtest/` as the test environment producer.
- Fill `VERIFY.md` against the PRD's Outcome bullets and the Acceptance Mapping below.

Acceptance for Phase 6: documentation merged; VERIFY checklist all non-PENDING.

---

## Trade-offs

- T-1: **Vendor upstream suites vs. submodule vs. fetch-on-build.** Chosen: vendor (per user direction). Adv.: hermetic clones, offline builds, simple contributor workflow. Disadv.: large repo size; upstream sync is a manual diff; tracking provenance lives only in `UPSTREAM.md`.
- T-2: **Bake on top of upstream rootfs vs. build a new rootfs from scratch.** Chosen: bake on top (assumed; user said "we need a new rootfs" but the simplest reading consistent with everything else is "a new image derived from the existing rootfs"). Adv.: minimal new build infrastructure; reuses Alpine, busybox, musl exactly as today; one extra make step. Disadv.: every test rootfs build re-copies a multi-MB image; if upstream rootfs ever drops something we need we have to add it back via the bake step. **Reviewer: confirm whether (Y) bake-on-top is correct, or whether the user actually meant (X) build-from-scratch — if (X), Phase 4's scope expands significantly.**
- T-3: **In-image `/test.sh` location — `/test.sh` vs. `/root/test.sh` vs. overwriting `/init.sh`.** Chosen: `/test.sh` at root. Adv.: clearly distinct from upstream `/init.sh`; the test rootfs's own init can be configured to dispatch to `/test.sh`; doesn't touch `/root/`. Disadv.: requires the Alpine init plumbing to call `/test.sh` (which we control via the bake step — likely by writing/replacing the boot script the upstream image already runs). **Reviewer: flag if there's a cleaner Alpine-native way (e.g. `/etc/local.d/test.start`).**
- T-4: **Per-suite contract — `Makefile` vs. `BUILD.sh` vs. uniform script.** Chosen: dual (Makefile preferred, BUILD.sh fallback, copy-only as last resort). Adv.: matches what upstream suites already ship; minimum local patches. Disadv.: `build-suites.sh` has more branches; failure mode discovery is per-suite.
- T-5: **Failure semantics in `run-*.sh` — abort on first failure vs. continue.** Chosen: continue (C-8). Adv.: full suite report each run; one failure doesn't hide later regressions. Disadv.: a hung test wastes time (mitigated by `lib/timeout.sh`); a kernel panic mid-run aborts everything anyway.
- T-6: **Where `run_qemu_tests` lives.** Chosen: extend `scripts/make/qemu.mk` with a parameterised macro and a tests-specific entry. Adv.: keeps qemu wiring in one file; reuses existing `BLK`/`NET`/`MEM` plumbing. Disadv.: small duplication if the macro can't fully parameterise the disk arg cleanly — may end up as a sibling macro.

---

## Validation

[**Unit Tests**]

- V-UT-1: `xtest/scripts/build/build-c.sh` against a fixture `xtest/c/` containing one passing `.c` and one deliberately-broken `.c`: passing `.c` produces an ELF; broken `.c` causes the script to exit non-zero with a clear error citing the file. *(Run as a host-side bash test, executed inside the contest Docker image in CI.)*
- V-UT-2: `xtest/scripts/build/stage.sh` against a populated `xtest/build/<arch>/` fixture: produces the documented `stage/root/tests/{c,testsuites,scripts}` layout exactly; missing inputs fail with a clear error.
- V-UT-3: `xtest/scripts/build/bake-image.sh` against a tiny ext4 fixture image (created in the test): output image contains `/root/tests/sentinel` (from the staged tree) and `/test.sh` is mode 0755; failure injection (rsync error) leaves no mounted loop device and no partial output image.
- V-UT-4: `dash -n src/test.sh` and `dash -n` over every `xtest/scripts/*.sh` and `xtest/scripts/**/*.sh` — POSIX shell syntax check.

[**Integration Tests**]

- V-IT-1: `make -C xtest all ARCH=riscv64` and `ARCH=loongarch64` inside Docker complete successfully on a clean checkout; `xtest/build/<arch>/tests-rootfs-<arch>.img` exists and is non-empty.
- V-IT-2: Loop-mount the produced image (in CI / inside Docker), assert `/test.sh` exists with mode 0755, `/root/tests/scripts/run-all.sh` exists, and at least one `/root/tests/c/*` ELF exists.
- V-IT-3: `make tests ARCH=...` from the top-level Makefile produces the same image bytes (modulo timestamps) as `make -C xtest all`.
- V-IT-4: `make run-tests ARCH=riscv64` boots the kernel against the test rootfs in QEMU (with a wall-clock timeout), captures serial output, and asserts:
  - the line `cd /root/tests` (or equivalent first action of `/test.sh`) is observed,
  - at least one `[PASS]` line from `run-c.sh` is observed,
  - the `basic` suite group header/footer (e.g. `==== basic ====` / `==== basic done ====`) is observed,
  - the boot reaches the post-test `sh` prompt without panic.
- V-IT-5: Same as V-IT-4 but `ARCH=loongarch64`.
- V-IT-6: `make run` (existing target) behaves identically before and after the change — same disk image used (`disk.img` from `qemu_rootfs`), same `init.sh`, same boot output up to the user shell.

[**Failure / Robustness Validation**]

- V-F-1: A first-party C test that `exit(1)`s prints `[FAIL] <name> exit=1` and the run continues to subsequent tests.
- V-F-2: A first-party C test that segfaults prints `[FAIL] <name> signal=SEGV` (or equivalent) and the run continues.
- V-F-3: A suite whose `run.sh` `sleep 9999`s is killed by `lib/timeout.sh` after the configured timeout; `[TIMEOUT] <suite>` is logged; the run continues.
- V-F-4: `bake-image.sh` interrupted (SIGTERM) mid-rsync leaves no mounted loop device, no partial output image — verified by re-running `mount` and `losetup -a` after.
- V-F-5: `make tests` with Docker uninstalled prints a clear "docker not found" error and exits non-zero (no half-built artifacts).

[**Edge Case Validation**]

- V-E-1: Two `xtest/c/` files with the same basename in different subdirectories cause `build-c.sh` to fail with a "duplicate test name" error (C-10).
- V-E-2: Empty `xtest/c/` (no `.c` files) succeeds: `build-c.sh` produces an empty `c/` dir; `run-c.sh` emits `no first-party tests` and continues.
- V-E-3: `xtest/testsuites/<s>/` with neither `Makefile` nor `BUILD.sh` is copied verbatim by `build-suites.sh` (script-only suites).
- V-E-4: `make tests` re-run with no source changes is idempotent (rerunning produces a functionally equivalent image; existing `tests-rootfs-<arch>.img` is overwritten cleanly).
- V-E-5: `make tests ARCH=riscv64` followed by `make tests ARCH=loongarch64` both succeed without one wiping the other's `xtest/build/<arch>/` outputs.

[**Acceptance Mapping**]

| Goal / Constraint | Validation |
|-------------------|------------|
| G-1 (layout)                          | V-IT-1, V-E-2 (verified by inspecting staged tree) |
| G-2 (old pipeline deleted)            | Phase 1 acceptance check (`git grep` clean) |
| G-3 (per-test C ELFs + run-c.sh)      | V-UT-1, V-IT-2, V-IT-4, V-F-1, V-F-2, V-E-1 |
| G-4 (vendored suites + UPSTREAM.md)   | Phase 3 acceptance check; V-IT-1 |
| G-5 (`src/test.sh` boot + run-all dispatch) | V-UT-4, V-IT-4, V-IT-5 |
| G-6 (`make tests` deterministic)      | V-IT-3, V-E-4 |
| G-7 (`make run-tests` boots image)    | V-IT-4, V-IT-5 |
| G-8 (`make run` unchanged)            | V-IT-6 |
| G-9 (smoke on rv + la)                | V-IT-4, V-IT-5, V-F-1, V-F-2, V-F-3 |
| C-1 (Docker-only host deps)           | V-F-5; Phase 5 acceptance |
| C-2 (`build/` gitignored)             | Phase 3 acceptance (`git status` clean after build) |
| C-3 (vendored + UPSTREAM.md)          | Phase 3 acceptance |
| C-4 (no Makefile var renames)         | V-IT-6 |
| C-5 (ARCH/BLK/NET/MEM/LOG passthrough)| V-IT-4 with `LOG=info` override; covered by reusing `run_qemu_tests` |
| C-6 (POSIX shell)                     | V-UT-4 |
| C-7 (Docker `--privileged` for mount) | V-IT-1; V-F-4 |
| C-8 (failures don't abort)            | V-F-1, V-F-2, V-F-3 |
| C-9 (unique test names)               | V-E-1 |
| C-10 (static musl link)               | V-IT-2 (file inspection); V-IT-4 (runs on Alpine) |
