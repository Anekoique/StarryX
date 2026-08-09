# Agent Guide

Orientation for AI agents contributing to **StarryX** — a component-oriented
macrokernel derived from ArceOS. Read this before touching code.

## Project Snapshot

- `no_std` Rust kernel, edition 2024, toolchain pinned to `nightly-2026-03-15`
  via `rust-toolchain.toml` / `Makefile` (`TOOLCHAIN`).
- Supports `riscv64` and `loongarch64` QEMU targets plus the
  `riscv64-visionfive2` board. `aarch64` / `x86_64` trees in `xcore/` have
  been pruned from the root build.
- Root license trio: GPL-3.0-or-later OR Apache-2.0 OR MulanPSL-2.0.
- Full context lives in `docs/` — `README.md`, `docs/record.md`,
  `docs/StarryX/intro.md`, `structure.md`, `xkernel.md`, `xmodules.md`,
  `xuspace.md`, `xprocess.md`, `xsignal.md`, `xcache.md`, `xvma.md`,
  `fs.md`, `mm.md`, `task.md`, `xmm.md`, `board.md`, `boot.md`.

## Repo Layout

```
starry/               Thin runtime and final image integration crate
xkernel/              Macrokernel services and Linux syscall ABI
  src/syscall/         Syscall translation, implementation, and dispatch
xmodules/             Flat collection of StarryX-owned x* components
  xerrno/             Shared kernel error vocabulary
  xio/                no_std I/O traits and helpers
  xsched/             Reusable scheduling policies
  xvfs/               Filesystem-neutral VFS contracts
  xuspace/            Safe user-space memory access (UserPtr / UserSpaceAccess)
  xprocess/           Process/thread/group/session lifecycle
  xsignal/            UNIX signal machinery (standard + realtime)
  xcache/             Page cache (LRU, Buffered I/O)
  xvma/               File-backed mmap region manager
  xutils/             Kernel shared utilities
  xvdso/              Linux vDSO provider, image, data ABI, and time updates
xcore/                ArceOS-derived low-level modules only
crates/               Lower-level and general-purpose support crates
drivers/              Driver interfaces and implementations
configs/              Build and platform configurations
scripts/make/         Makefile includes: features, platform, config, build, qemu
docs/StarryX/         Design docs, diagrams, images
```

The Cargo workspace (`Cargo.toml`) includes `starry`, `xkernel`,
`xmodules/*`, `xcore/*`, and `crates/*`. It excludes the standalone driver
workspace, display/dma modules, and the `page_table_multiarch`, `smoltcp`,
and `lwext4_rust` subtrees from default workspace-wide commands.

## Build & Run

Prefer the Makefile — it exports `XCORE_*` env vars that `xconfig` and build
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
make clean        # drop built *.bin/*.elf and xconfig
make docker       # enter contest docker image
```

Useful overrides: `ARCH`, `PLATFORM`, `SMP`, `MODE={release,debug}`,
`LOG={off,error,warn,info,debug,trace}`, `FEATURES`, `BLK`, `NET`, `MEM`,
`DISK_IMG`, `NET_DEV`, `ACCEL`. Never change `TOOLCHAIN` without coordinating.

`xmodules/xvdso/build.rs` obtains the external Linux vDSO provider at a pinned
revision and caches it under Cargo's build output. No Docker setup or committed
blob regeneration is required. `XVDSO_SOURCE_DIR` can select an existing local
checkout for offline builds; see `docs/StarryX/vdso.md`. The default provider
does not yet contain the required LoongArch image.

## Agent Playbook

### Before writing code
- Read the relevant `docs/StarryX/*.md` page(s) for the subsystem.
- `gh search` and `crates.io`/registries first — prefer porting over net-new.
- Use the **planner** agent for multi-file work, **architect** for design
  decisions, **Explore** for broad codebase questions.

### While writing code
- Respect module decoupling: component crates in `xmodules/*` must stay
  reusable — do not pull `xkernel` into them. Exchange behaviour through
  traits (`UserSpaceAccess`, `InodeOps`/`PageOps`, `WaitQueue`, `VmFile`, …).
- XCore modules (`xhal`, `xmm`, `xfs`, `xtask`, `xnet`, …) must stay
  OS-agnostic. If higher-level reusable logic creeps in, move it to `xmodules`.
- `xkernel::syscall` owns Linux ABI translation and depends on the service
  modules in `xkernel`; those service modules must not depend back on
  `xkernel::syscall`. `xmodules` holds reusable contracts and algorithms.
- Kernel code is `no_std` + `alloc`. No `std`, no blocking on host I/O.
- Rust style: `rustfmt` is authoritative; `&str`/`&[T]` in params, return
  owned on transfer; propagate errors with `?`; never `.unwrap()` in prod.
  `LinuxResult<T>` (via `xerrno`) is the default result type.
- Immutability by default — only `let mut` when mutation is required; never
  mutate inputs in place when a new value can be returned.
- `unsafe` needs a `// SAFETY:` comment spelling out every invariant. User
  pointers go through `xuspace::{UserPtr, UserConstPtr}`; do not dereference
  raw user addresses.
- Keep files focused (200–400 lines typical, 800 hard cap); extract helpers
  before they grow.
- Comments only explain non-obvious *why*. No narrated changelogs in source.
- Avoid hardcoded platform constants — use `xconfig` / platform tomls under
  `configs/platforms/` and the `XCORE_*` env vars.

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
- **xtest framework** — `make test ARCH=... [PROFILE=smoke] [CASE=<id>]`
  runs the standalone Rust host runner. It builds `xtest/cases/`, injects one
  `/xtest` bundle into a copied rootfs with non-privileged e2fsprogs, owns the
  Make/QEMU process group, and writes serial/JSON/TAP output below
  `target/xtest/<arch>/<profile>/<run-id>/`. Test selection is a rootfs
  property; ordinary init falls through when `/xtest` is absent.
- **External testsuits** — `xtest/testsuits/<name>/` is only an opt-in Git
  submodule boundary. A selected checkout provides the generic `xtest.toml`
  build-and-cases manifest and writes artifacts below `XTEST_OUT`. Do not add
  suite names, patches, adapters, output parsers, skip policy, implicit fetches,
  or vendored sources to the framework. See `xtest/README.md`.
- **vDSO** — `xmodules/xvdso` owns the pinned external provider, embedded
  image, Linux-compatible data page, and timer updates. `xkernel/src/vdso.rs`
  is only the address-space installation adapter. There is no Docker or local
  blob regeneration flow. See `docs/StarryX/vdso.md`. Tests live under
  `xtest/cases/time/vdso_*.c`.
- Use the **tdd-guide** agent when starting a new feature or bug fix.

## Git & PR Workflow

- Conventional commits: `feat|fix|refactor|docs|test|chore|perf|ci: subject`.
- Never commit generated artifacts (`*.bin`, `*.elf`, `.xconfig.*.toml`,
  `target/`, downloaded rootfs). They are already built locally and should
  stay untracked.
- PR body: summary bullets + test plan + ARCH coverage (at minimum one of
  `rv`/`la` reported). Analyse the whole branch (`git diff main...HEAD`),
  not just the tip commit.
- Ask before destructive ops (force push, hard reset, branch deletion,
  hook skipping, touching ArceOS-vendored crates wholesale).

## Common Pitfalls

- Touching `xcore/xhal/linker.lds.S` or per-arch asm without
  rebuilding all supported targets — always run both `rv` and `la`.
- Adding a dependency from `xmodules` to `xkernel` — breaks the reuse
  contract. Route behaviour through traits.
- Using `VecDeque`/`alloc` types in interrupt context or inside spinlocks;
  see `xsignal` / `xprocess` for the correct mutex + queue layering.
- Forgetting to re-run `make oldconfig` / `make defconfig` after adding a
  new platform config key.
- Assuming x86_64/aarch64 still build from the root — they don't. Keep
  arch-gated code behind `cfg(target_arch = "riscv64"|"loongarch64")` or
  `cfg(feature = "...")` so the root workspace stays green.

<!-- ARK:START -->
Ark is installed in this project. Use `/ark:quick` or `/ark:design` to start tasks.

See `.ark/workflow.md` for the full workflow.

@.ark/specs/INDEX.md
<!-- ARK:END -->
