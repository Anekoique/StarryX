# Agent Guide

Orientation for AI agents contributing to **StarryX** — a monolithic kernel
built on ArceOS (`arceos/starry` lineage). Read this before touching code.

## Project Snapshot

- `no_std` Rust kernel, edition 2024, toolchain pinned to `nightly-2026-03-15`
  via `rust-toolchain.toml` / `Makefile` (`TOOLCHAIN`).
- Supports `riscv64` and `loongarch64` QEMU targets plus the
  `riscv64-visionfive2` board. `aarch64` / `x86_64` trees in `arceos/` have
  been pruned from the root build.
- Root license trio: GPL-3.0-or-later OR Apache-2.0 OR MulanPSL-2.0.
- Full context lives in `docs/` — `README.md`, `docs/record.md`,
  `docs/StarryX/intro.md`, `structure.md`, `xcore.md`, `xmodules.md`,
  `xuspace.md`, `xprocess.md`, `xsignal.md`, `xcache.md`, `xvma.md`,
  `fs.md`, `mm.md`, `task.md`, `axmm.md`, `board.md`, `boot.md`.

## Repo Layout

```
src/                  Entry OS (main.rs, entry.rs, syscall.rs, mm.rs, init.sh)
xcore/                Macrokernel core (fs, ipc, mm, net, sys, task)
xapi/                 POSIX surface (fs, iomux, ipc, mm, net, sys, task)
xmodules/             Reusable macrokernel components
  xuspace/            Safe user-space memory access (UserPtr / UserSpaceAccess)
  xprocess/           Process/thread/group/session lifecycle
  xsignal/            UNIX signal machinery (standard + realtime)
  xcache/             Page cache (LRU, Buffered I/O)
  xvma/               File-backed mmap region manager
  xutils/             Macrokernel shared utilities
arceos/               Vendored ArceOS workspace (modules + crates + configs)
scripts/make/         Makefile includes: features, platform, config, build, qemu
docs/StarryX/         Design docs, diagrams, images
```

The Cargo workspace (`Cargo.toml`) includes `xapi`, `xcore`, `xmodules/*`,
`arceos/modules/*`, `arceos/crates/*` and excludes display/dma/pci/smoltcp
drivers plus the `page_table_multiarch` and `lwext4_rust` subtrees.

## Build & Run

Prefer the Makefile — it exports `AX_*` env vars that `axconfig` and build
scripts rely on.

```sh
make all          # default: riscv64 QEMU competition suite
make rv           # ARCH=riscv64     BLK=y NET=y FEATURES=...virtio-blk
make la           # ARCH=loongarch64 BLK=y NET=y FEATURES=...virtio-blk
make vf2          # PLATFORM=riscv64-visionfive2  BUS=mmio  SMP=2
make build        # build only (no QEMU)
make run
make debug        # runs QEMU + GDB on localhost:1234
make clippy       # oldconfig + workspace clippy
make fmt          # cargo fmt --all
make clean        # drop built *.bin/*.elf and axconfig
make docker       # enter contest docker image
```

Useful overrides: `ARCH`, `PLATFORM`, `SMP`, `MODE={release,debug}`,
`LOG={off,error,warn,info,debug,trace}`, `FEATURES`, `BLK`, `NET`, `MEM`,
`DISK_IMG`, `NET_DEV`, `ACCEL`. Never change `TOOLCHAIN` without coordinating.

## Agent Playbook

### Before writing code
- Read the relevant `docs/StarryX/*.md` page(s) for the subsystem.
- `gh search` and `crates.io`/registries first — prefer porting over net-new.
- Use the **planner** agent for multi-file work, **architect** for design
  decisions, **Explore** for broad codebase questions.

### While writing code
- Respect module decoupling: component crates in `xmodules/*` must stay
  reusable — do not pull `xcore`/`xapi` into them. Exchange behaviour through
  traits (`UserSpaceAccess`, `InodeOps`/`PageOps`, `WaitQueue`, `VmFile`, …).
- ArceOS modules (`axhal`, `axmm`, `axfs-ng`, `axtask`, `axnet`, …) must stay
  OS-agnostic. If macrokernel logic creeps in, move it up to `xmodules`.
- `xapi` wraps POSIX syscalls; `xcore` owns macrokernel state; `xmodules`
  holds reusable algorithms. Pick the smallest layer that fits.
- Kernel code is `no_std` + `alloc`. No `std`, no blocking on host I/O.
- Rust style: `rustfmt` is authoritative; `&str`/`&[T]` in params, return
  owned on transfer; propagate errors with `?`; never `.unwrap()` in prod.
  `LinuxResult<T>` (via `axerrno`) is the default result type.
- Immutability by default — only `let mut` when mutation is required; never
  mutate inputs in place when a new value can be returned.
- `unsafe` needs a `// SAFETY:` comment spelling out every invariant. User
  pointers go through `xuspace::{UserPtr, UserConstPtr}`; do not dereference
  raw user addresses.
- Keep files focused (200–400 lines typical, 800 hard cap); extract helpers
  before they grow.
- Comments only explain non-obvious *why*. No narrated changelogs in source.
- Avoid hardcoded platform constants — use `axconfig` / platform tomls under
  `arceos/configs/platforms/` and the `AX_*` env vars.

### After writing code
- `make fmt` and `make clippy` both targets you touched (`ARCH=riscv64`
  *and* `ARCH=loongarch64` when behaviour is arch-sensitive).
- `make rv` / `make la` (or `make build` when you cannot boot) to confirm
  the kernel still links and boots.
- Run the **code-reviewer** agent on the diff; address CRITICAL/HIGH, fix
  MEDIUM when cheap.
- Run the **security-reviewer** agent for anything touching user pointers,
  syscall dispatch, signal delivery, filesystem paths, mmap, or networking.

### Testing
- 80 %+ coverage target where host tests are feasible. Unit tests inside
  crates via `#[cfg(test)]`, doc-tests for public APIs, integration tests
  under each crate's `tests/` dir.
- For kernel-only paths, prefer component-level tests in `xmodules/*/tests/`
  against trait fakes rather than booting the whole kernel.
- End-to-end validation uses the upstream Alpine rootfs: `make qemu_rootfs`
  fetches `rootfs-$(ARCH).img`; `make rv` / `make la` runs it.
- **xtest pipeline** — `make tests ARCH=...` builds a separate
  `tests-rootfs-$ARCH.img` (under `xtest/build/`) by baking first-party C
  tests and vendored OS-COMP suites into a copy of the upstream rootfs.
  `make run-tests ARCH=...` builds the kernel with `ROOT_FEATURES=init-test`
  (which embeds `src/test.sh` instead of `src/init.sh` via the `init-test`
  cargo feature on the root crate) and boots it against that image. Both
  targets require Docker — the cross-build runs inside
  `docker.educg.net/cg/os-contest@sha256:742479b…`. See `xtest/README.md`.
- **vDSO** — the kernel maps `linux-vdso.so.1` into every user address
  space. Pre-built blobs live under `xcore/src/vdso/blobs/`, embedded
  via `.incbin`. Source under `xmodules/xvdso/` (workspace-excluded).
  Run `make regenerate-vdso-blobs` after touching the source and commit
  the updated `.so`. See `docs/StarryX/vdso.md`. Tests under
  `xtest/c/time/vdso_*.c`.
- Use the **tdd-guide** agent when starting a new feature or bug fix.

## Git & PR Workflow

- Conventional commits: `feat|fix|refactor|docs|test|chore|perf|ci: subject`.
- Never commit generated artifacts (`*.bin`, `*.elf`, `.axconfig.*.toml`,
  `target/`, downloaded rootfs). They are already built locally and should
  stay untracked.
- PR body: summary bullets + test plan + ARCH coverage (at minimum one of
  `rv`/`la` reported). Analyse the whole branch (`git diff main...HEAD`),
  not just the tip commit.
- Ask before destructive ops (force push, hard reset, branch deletion,
  hook skipping, touching ArceOS-vendored crates wholesale).

## Common Pitfalls

- Touching `arceos/modules/axhal/linker.lds.S` or per-arch asm without
  rebuilding all supported targets — always run both `rv` and `la`.
- Adding direct dependencies between `xcore` and `xmodules` crates — breaks
  the reuse contract. Route through traits.
- Using `VecDeque`/`alloc` types in interrupt context or inside spinlocks;
  see `xsignal` / `xprocess` for the correct mutex + queue layering.
- Forgetting to re-run `make oldconfig` / `make defconfig` after adding a
  new platform config key.
- Assuming x86_64/aarch64 still build from the root — they don't. Keep
  arch-gated code behind `cfg(target_arch = "riscv64"|"loongarch64")` or
  `cfg(feature = "...")` so the root workspace stays green.
