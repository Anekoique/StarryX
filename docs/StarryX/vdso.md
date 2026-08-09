# vDSO

StarryX maps a small ELF (`linux-vdso.so.1`) into every user address space
so libc can serve a handful of frequently-called operations without
trapping into the kernel. The image is built from `xmodules/xvdso/`,
embedded into the kernel via `include_bytes!`, and published to user code
through the auxv `AT_SYSINFO_EHDR` entry.

## Layout

Every user address space contains:

| VA                   | Region        | Flags | Backing                                        |
| -------------------- | ------------- | ----- | ---------------------------------------------- |
| `USER_VDSO_BASE`     | vDSO code     | R-X   | per-process `map_alloc`-backed copy of the ELF |
| `USER_VDSO_DATA`     | vDSO data     | R--   | single shared phys page (`xkernel::vdso::VDSO_DATA`) |

`USER_VDSO_BASE = 0x4001_0000`, `USER_VDSO_DATA = 0x4001_2000`. The data
page is `map_linear`-mapped against the kernel-resident `'static
VDSO_DATA: VdsoData` so all processes observe the same seqlock-protected
snapshot.

## Symbols

The vDSO exports under the `LINUX_2.6` Verdef:

- `__vdso_clock_gettime` (CLOCK_REALTIME / CLOCK_MONOTONIC[_RAW])
- `__vdso_gettimeofday`
- `__vdso_clock_getres`
- `__vdso_time`
- `__vdso_getcpu` (currently `-ENOSYS`; SMP-correct version is a follow-up)
- `__vdso_rt_sigreturn`

Plus `__kernel_*` aliases for glibc's probe path. Unsupported clock IDs
fall through to the kernel via `ecall` / `syscall 0` from inside the vDSO
code — the trap is the same, just paid only when needed.

## Time fast path

`vdso_tick()` runs in the boot CPU's timer ISR (gated by
`xhal::cpu::this_cpu_is_bsp()`) and refreshes `VDSO_DATA` under a
seqlock. The relevant fields are:

```rust
struct VdsoData {
    seq: AtomicU32,             // even = stable, odd = writer
    cpu: u32,
    wall_sec: u64,
    wall_nsec: u32,
    mono_ns: u64,                // captured monotonic ns
    mono_cycles_at_capture: u64, // counter value at capture
    mult: u32, shift: u32,       // (delta * mult) >> shift = ns
}
```

`mult` and `shift` are computed once from
`xhal::time::timer_frequency()`:

```text
shift = 24
mult  = (NANOS_PER_SEC << shift) / timer_frequency()
```

Readers in user space pair two acquire-loads of `seq` around the field
reads and retry if the writer was in flight or the snapshot changed.
After a clean read, the user-side computes
`now_ns = mono_ns + ((rdtime() - mono_cycles_at_capture) * mult) >> shift`.

Maximum representable `delta` before the multiplier overflows u64 is
roughly `2^64 / mult`. At a 10 MHz timer (`mult ≈ 1.7e11`), that's about
10 seconds; the timer ISR refreshes at the kernel's tick rate (≥100 Hz),
so the in-flight delta stays under 10 ms by ~3 orders of magnitude.

## Signal trampoline

`__vdso_rt_sigreturn` replaces the legacy `SIGNAL_TRAMPOLINE` mapping.
On `execve`, `xkernel::vdso::install` parses the embedded ELF's `.dynsym`
to find the symbol's offset, computes the per-process absolute address,
and publishes it via `ProcessSignalManager::set_default_restorer`. When
a signal is delivered, the kernel writes that address into the user
frame's `pretcode`; the handler returns to the vDSO entry, which traps
back via `rt_sigreturn`.

## Building

Pre-built `linux-vdso.so.1` blobs are committed under
`xkernel/src/vdso/blobs/{vdso-riscv64.so, vdso-loongarch64.so}` and
embedded into the kernel via `.incbin` inside a `global_asm!` block in
`xkernel/src/vdso/blob.rs`. The kernel build does not depend on a vDSO
build step — `make build` / `make rv` / `make la` work with whatever
blobs are committed.

To regenerate the blobs after changing the source under
`xmodules/xvdso/`:

```sh
make regenerate-vdso-blobs
```

This runs `cargo build` against `xmodules/xvdso/Cargo.toml` with the
per-arch JSON target spec (under `xmodules/xvdso/targets/`) that enables
`dynamic-linking` and `relocation-model: pic` — needed for a `cdylib`
ELF, but incompatible with the kernel's bare-metal build, so
`xmodules/xvdso/` is workspace-`exclude`d. After regeneration, commit
the updated `.so` files alongside any source changes.

## Tests

Live under `xtest/c/time/`:

- `vdso_clock_monotonic.c` — 100 k `clock_gettime(CLOCK_MONOTONIC)` reads, asserts monotonicity + sane elapsed time.
- `vdso_gettimeofday.c` — gettimeofday agrees with `clock_gettime(CLOCK_REALTIME)` to within 100 ms.
- `vdso_clock_getres.c` — `clock_getres({REAL,MONO})` returns `{0, 1}`.
- `vdso_rt_sigreturn.c` — installs SIGUSR1 handler, raises it, verifies clean return through the vDSO trampoline.

Run via `make tests ARCH=...` then `make run-tests ARCH=...`.
