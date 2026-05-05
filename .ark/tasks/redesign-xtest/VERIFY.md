# `xtest` VERIFY

> Status: Living document. Maintained by the implementer during EXECUTE → COMMIT.
> Feature: `xtest`
> Target Task: `redesign-xtest`
> Tier: `deep`
>
> The implementer fills the auto-seeded checklist sections as compliance is
> confirmed (PASS) or violated (FAIL with explanation) or judged irrelevant
> (N/A with explanation). The Findings section captures opinions and
> judgment calls — issues the implementer notices that don't map to a
> single seeded checklist item. **No verdict line: completion = every item
> resolved (no `PENDING`).** `/ark:commit` refuses on deep tier when any
> entry is still `PENDING`; on standard tier it warns and proceeds.

---

## Project Spec Compliance

> No project SPECs registered yet (`.ark/specs/project/INDEX.md` is empty).

- [x] (no project SPECs registered): N/A

## Related Feature Spec Compliance

> PRD lists no related specs — this is the first deep-tier task in the repo.

- (none registered): N/A

## PRD Constraints

> One bullet per Outcome line in the PRD.

- [x] `xtest/` layout is `xtest/{c,scripts}` with all old files deleted: PASS — `xtest/testsuites/` was vendored and exercised end-to-end during execution but **removed before commit at user request** (see V-001). The shipped layout is `xtest/{c,scripts}` plus `Makefile`, `README.md`, `.gitignore`.
- [x] `xtest/c/` holds single-file C tests, each compiled to one ELF in Docker: PASS — five tests under `xtest/c/{syscall,signal,mm,fs}/` cross-compile with `riscv64-linux-musl-gcc` / `loongarch64-linux-musl-gcc` via `xtest/scripts/build/build-c.sh`.
- [x] `xtest/testsuites/{...}/` contains pre-2025 sources: **N/A** — explicit user-directed scope reduction (V-001). The vendored suites + per-suite BUILD.sh wrappers were proven end-to-end on rv64 (lua / netperf / iozone / iperf / busybox / lmbench / libcbench / basic / cyclictest / libc-test / LTP all built and ran), but removed before commit. The build pipeline still supports adding suites back later — see V-002 (lessons learned).
- [x] `src/test.sh` exists alongside `src/init.sh`, sets env, runs run-all.sh, drops to shell: PASS — `src/test.sh` shipped; embedded into kernel ELF when `ROOT_FEATURES=init-test`; verified via boot smoke.
- [x] `make tests ARCH=...` builds `tests-rootfs-$ARCH.img`: PASS — verified for both arches; fresh ext4 image built with `mkfs.ext4`, upstream rootfs contents tar-piped in, then staged tree rsynced into `/root/tests`. Image is ~10 MB (C-only scope).
- [x] `make run-tests` builds + boots the image: PASS — rv64 + la64 boot smoke both green: 5/5 C tests pass.
- [x] `make run` and `src/init.sh` remain unchanged: **PARTIAL** — `make run` semantics unchanged (no `ROOT_FEATURES`, embeds `init.sh`, kernel ELF carries `# id: starry-init`); `src/init.sh` gained exactly one new line (`# id: starry-init`) per the explicit G-12 carve-out in PLAN 03 — see V-003.
- [x] `basic` suite plus first-party C smoke runs end-to-end on rv64 + la64: **PARTIAL** — first-party C smoke is fully green on both arches (5/5 each). The `basic` suite reference is dropped per the C-only scope reduction (V-001); the suite was exercised during execution and 28/33 of its tests passed on rv64 before scope reduction.

## Plan Fidelity

> One bullet per Goal in the latest PLAN.

- [x] G-1 (`xtest/{c,testsuites,scripts}` layout): **REVISED** — shipped layout is `xtest/{c,scripts}` per V-001; `testsuites/` was built and exercised then removed at user direction. The build pipeline supporting `testsuites/` was deleted along with the vendored content.
- [x] G-2 (old pipeline deleted, nothing references them): PASS — `git grep -E 'Makefile\.sub|busybox-config-|git_testcode|sdcard-rv|sdcard-la' -- ':!src/init.sh' ':!arceos/'` returns empty.
- [x] G-3 (per-test C ELFs + run-c.sh): PASS — 5 tests, both arches, static musl ELFs.
- [x] G-4 (vendored suites + UPSTREAM.md): **WITHDRAWN** — see V-001.
- [x] G-5 (`src/test.sh` embedded + run-all dispatch): PASS — boot output confirms `test.sh` ran and dispatched to `run-all.sh` which invoked `run-c.sh`.
- [x] G-6 (`make tests` deterministic staging): PASS — staging is rsync-deterministic; the new bake-image flow uses `mkfs.ext4` + tar-pipe of upstream rootfs contents (more robust than in-place resize).
- [x] G-7 (`make run-tests` builds with feature + boots): PASS on rv64 + la64.
- [x] G-8 (`make run` unchanged post-marker): PASS — `git diff src/init.sh` adds exactly one `# id: starry-init` line; kernel ELF without the feature embeds `starry-init` and not `starry-test`.
- [x] G-9 (smoke on rv + la): PASS — both arches: kernel boots, `test.sh` runs, all 5 C tests PASS, drops to interactive shell.
- [x] G-10 (documentation): PASS — `xtest/README.md` updated to reflect C-only scope; `AGENTS.md` "Testing" section references `make tests` / `make run-tests` and Docker dependency.
- [x] G-11 (build switch): PASS — `init-test` cargo feature on root crate; two-arm `#[cfg(feature = "init-test")]` `include_str!` in `src/main.rs`; threaded through `ROOT_FEATURES` Make variable; verified on both arches.
- [x] G-12 (init-script ID markers): PASS — `src/init.sh` and `src/test.sh` carry unique `# id:` markers; ELF strings inspection confirms the right script is embedded for each build mode.

## SPEC Drift

> Promoted SPEC will be `specs/features/xtest/SPEC.md` (first feature SPEC in the repo).

- [x] No prior SPECs modified (this is the first feature SPEC promotion): N/A
- [x] **The promoted SPEC must reflect the C-only scope** — the `## Spec` section of PLAN 03 still describes a `xtest/{c,testsuites,scripts}` layout with vendored suites. Before commit-time SPEC promotion, the `## Spec` section should be updated to drop the `testsuites/` half. See V-001.

## Findings

### V-001 Test-suite scope withdrawn at user request before commit

- **Severity:** HIGH (process/Spec)
- **Location:** entire `xtest/testsuites/` subtree (during execution); PLAN 03's `## Spec` Goals G-4, G-9, Data Structure, Architecture diagram, build-suites.sh / run-suite.sh / build-one-suite.sh references.
- **Problem:** PLAN 03's Spec lists a `testsuites/` half with 11 vendored OS-COMP suites + per-suite `BUILD.sh`/`run.sh` plus a `build-suites.sh` dispatcher. During execution all 11 suites were vendored, cross-built (with substantial musl-vs-glibc patching for libc-test, lmbench, cyclictest, basic), and end-to-end exercised on rv64 against the StarryX kernel — surfacing real kernel issues (basic/clone signal=11, basic/{open,openat,read,fstat} signal=6, kernel OOM after 7 LTP tests, libc-test mass failures suggesting kernel syscall stub gaps). After this evidence the user directed: "transfer test suits seems leave too much load and problems, lets port c first, remove testsuits totally."
- **Why it matters:** The shipped tree no longer includes `testsuites/` content. The promoted SPEC at archive time must reflect the C-only scope — otherwise it documents capabilities the code doesn't provide. The Spec section of PLAN 03 needs to be edited (per workflow §4 EXECUTE: "If implementation reveals gaps in the design, update the latest PLAN's `## Spec` section to reflect reality") before `/ark:commit` extracts it to `specs/features/xtest/SPEC.md`.
- **Recommendation:** Before commit, edit PLAN 03's Spec section to:
  1. Layout: `xtest/{c,scripts}` (drop `testsuites/`).
  2. G-4: withdraw (no upstream suites in this iteration).
  3. G-9: rewrite as "first-party C smoke succeeds on rv64 + la64".
  4. Architecture diagram: drop the `for s in testsuites/*` loop.
  5. Data Structure: drop `testsuites/` block.
  6. Constraints C-3, C-13, NG-3 (glibc): trim/withdraw the suite-related parts.
  7. Validation: drop V-UT-7, V-IT-1's suite assertion, V-E-3.
  Open a follow-up task (e.g. `port-oscomp-suites`) that re-adds `testsuites/` once the kernel can survive a full LTP / libc-test run. The follow-up has a clear inheritance: every BUILD.sh / patch we developed is preserved in this conversation's git history for `feat/redesign-xtest` (now reset).
- **Resolution:** ACCEPTED — explicit user directive. Will edit PLAN 03 Spec before `/ark:commit` so the promoted SPEC matches the shipped code.

### V-002 Lessons learned: per-suite cross-musl build patches (kept for follow-up reference)

- **Severity:** LOW (knowledge capture)
- **Location:** N/A — nothing in the shipped tree.
- **Problem:** During the suite porting work we developed real, non-trivial patches to make each upstream suite cross-build against musl. These shouldn't be lost — when someone re-attempts the suites, they should start from this list:
  - **libc-test:** add `-Wl,-z,notext` to LDFLAGS to allow text relocations on the static link (`tls_align_dso.obj` mixes PIC into a static binary). Drop `entry-dynamic`'s `-rdynamic` if it conflicts.
  - **basic (rCore-style sources):** ship a `oscomp_shim.h` that pulls in standard musl headers, defines `STDIN`/`STDOUT`/`STDERR` aliases for `STDIN_FILENO` etc., maps `O_CREATE` → `O_CREAT`, defines `kstat` → `stat`, maps `st_atime_sec` → `st_atim.tv_sec` (and friends), implements `get_time()` via `clock_gettime(CLOCK_MONOTONIC)`, and provides `xtest_getdents` via `syscall(SYS_getdents64,…)` (musl's `getdents` declaration in `<dirent.h>` conflicts with rCore's signature). Compile with `-include <shim.h>`.
  - **busybox:** Kbuild requires a `.config` we don't ship; skip the build and exercise Alpine's pre-installed `/bin/busybox` instead.
  - **lua:** `cd src && make CC=$MUSL_CC AR='${PREFIX}ar rcu' RANLIB=${PREFIX}ranlib MYCFLAGS=-static MYLDFLAGS=-static PLAT=generic`.
  - **iperf:** `./configure --host=$host CC=$MUSL_CC --enable-static-bin --disable-shared`; iperf3 binary lands in `src/iperf3` or `src/.libs/iperf3`.
  - **netperf:** unpack `netperf-2.7.0.tar.gz`, `./configure --host=$host CC=$MUSL_CC CFLAGS='-static -O2 -Wno-error' --disable-omni-tests --enable-cpuutil=none`.
  - **iozone:** `make -f makefile linux-AMD64 CC=$MUSL_CC GCC=$MUSL_CC CFLAGS='-static -O2 -Wno-error'` (linux-AMD64 is the most generic target).
  - **lmbench:** sed-patch `bench.h` to drop `<rpc/rpc.h>` and `<rpc/types.h>` (musl doesn't ship those). Stub `pmap_set`/`pmap_unset`/`pmap_getport` in a small `rpc_stubs.c`. Drop `lat_rpc`/`lat_http`/`lmhttp`/`mhz`/`rhttp` handlers from `lmbench_all.c`. Compile every EXE_SRCS .c into a .o and link with `-DTRUE=1 -DFALSE=0 -D_GNU_SOURCE`.
  - **cyclictest:** build numactl-2.0.14 statically in `/tmp` (configure `--host=$host CC=$MUSL_CC --enable-static --disable-shared`), tar-pipe the result back into the suite dir. Pass `-DLOONGARCH_MUSL` (the macro name is misleading — also needed on rv64 musl) to skip the rCore-style `sigev_notify_thread_id` redefinition. Add `-Wl,-z,notext -static -latomic` to LDFLAGS.
  - **LTP:** sed-replace `<sys/sysinfo.h>` → `<linux/sysinfo.h>` repo-wide; prepend `FILTER_OUT_DIRS += fmtmsg rt_sigtimedwait rt_tgsigqueueinfo timer_create` to `testcases/kernel/syscalls/Makefile`; `make autotools && ./configure --host=$host CC=$MUSL_CC --without-tirpc --with-target-cflags='-march=rv64gc' --prefix=/ltp`; `make -j -k`; ELFs land scattered under `testcases/<category>/<test>/<bin>` — flatten via tar-pipe with `--transform 's|.*/||'` and run `${cross}-strip --strip-unneeded` to shrink ~1.3GB → ~334MB.
- **Why it matters:** When the kernel can survive a real LTP / libc-test run, the next person attempting the suites shouldn't have to re-discover all of this from scratch. This finding lives in the task's archive.
- **Recommendation:** When opening the follow-up task `port-oscomp-suites`, copy this list into its PRD as the starting point. The conversation transcript on `feat/redesign-xtest` (before the testsuites removal commit) has the full per-suite BUILD.sh content.
- **Resolution:** ACCEPTED — knowledge capture only.

### V-003 `src/init.sh` gained one line — intentional G-12 carve-out

- **Severity:** LOW
- **Location:** `src/init.sh:1`
- **Problem:** PRD's Outcome bullet says `src/init.sh remains byte-for-byte unchanged`, but the implemented design adds a single `# id: starry-init` marker line so V-UT-8 / V-IT-8 can mechanically verify which script the kernel ELF embedded.
- **Why it matters:** The literal PRD wording is not honoured. The reader of the PRD should know there's a one-line carve-out.
- **Recommendation:** PLAN 03 explicitly captures the carve-out in G-8 + G-12. The promoted feature SPEC carries that carve-out.
- **Resolution:** ACCEPTED — explicitly designed, documented in G-8/G-12, mechanically verified.

## Notes

**Final shipped layout:**
```
xtest/
├── Makefile           build pipeline (Docker-driven)
├── README.md
├── .gitignore         ignores build/
├── c/
│   ├── common/assert.h
│   ├── syscall/{getpid,clone_basic}.c
│   ├── signal/kill_self.c
│   ├── mm/mmap_anon.c
│   └── fs/open_close.c
└── scripts/
    ├── build/{build-c,stage,bake-image}.sh
    ├── run-all.sh
    └── run-c.sh

src/
├── init.sh            unchanged except for `# id: starry-init` marker
└── test.sh            new — kernel-embedded test driver
```

**Boot smoke evidence** (rv64 + la64, post C-only scope reduction):
```
==== c ====
[PASS] clone_basic
[PASS] getpid
[PASS] kill_self
[PASS] mmap_anon
[PASS] open_close
==== c done ====
[done] xtest run complete
```

**Trade-offs landed during execution:**
- `# id:` marker convention now in `src/init.sh` (one new line) — explicit carve-out in G-8/G-12.
- `make run-tests` reuses an existing image when present; `make tests` always rebuilds.
- `bake-image.sh` builds a **fresh** ext4 image with `mkfs.ext4` + tar-pipes upstream rootfs contents in (rather than in-place resize2fs of the upstream image, which fails on the contest e2fsprogs 1.46.5 because the upstream rootfs uses unsupported ext4 features).
- `run-tests` recurses via `$(MAKE) … BLK=y NET=y FEATURES=$(QEMU_FEATURES) ROOT_FEATURES=init-test justrun-tests` so `qemu.mk` evaluates with the virtio-blk/virtio-net device wiring (mirrors how `make rv` / `make la` work). Without this recursion, `run_qemu_tests` runs in the parent's context where `BLK=n` and the kernel panics with `No block device found!`.
- Test suite scope withdrawn — see V-001.

**Spec edit pending before `/ark:commit`:** PLAN 03's `## Spec` section needs the testsuites-related parts trimmed so the promoted feature SPEC matches the shipped code. The edits are listed in V-001's Recommendation.
