# `vdso-support` PRD

---

[**What**]
Add a Linux-compatible vDSO to StarryX: a kernel-built shared object mapped read-execute into every user process, exposing `__vdso_clock_gettime`, `__vdso_gettimeofday`, `__vdso_clock_getres`, `__vdso_time`, `__vdso_getcpu`, and `__vdso_rt_sigreturn` (plus `__vdso_sigreturn` where the per-arch ABI requires it). Published to userspace via the `AT_SYSINFO_EHDR` auxv entry. The signal-return trampoline migrates from the current fixed `SIGNAL_TRAMPOLINE` linear mapping into the vDSO image.

[**Why**]
- **Performance.** Time syscalls are the most frequent user→kernel transition in typical workloads; serving them from a userspace-readable counter (`rdtime` on RV, `rdtime.d` on LA) plus a kernel-published shared data page eliminates the trap cost.
- **ABI parity.** glibc and musl probe `AT_SYSINFO_EHDR` and prefer vDSO entry points when present. Without one, dynamic loaders fall back to syscalls and miss optimizations; some newer glibc paths assume the vDSO is mapped.
- **Cleanup.** The current ad-hoc `SIGNAL_TRAMPOLINE` (single page mapped linearly to a kernel symbol at a fixed VA) is a Linux-incompatible workaround. Folding `rt_sigreturn` into the vDSO consolidates user-visible kernel-provided code into one well-defined image.

[**Outcome**]
- `xmodules/xvdso/` builds a per-arch vDSO `cdylib` (riscv64, loongarch64) with the pinned toolchain; the artifact is `include_bytes!`-ed into the kernel.
- On `execve`, the kernel maps two pages into every user address space: one R-X for the vDSO code, one R-only for the kernel-published time data page (seqlock-protected). `AT_SYSINFO_EHDR` in auxv points at the code page's ELF header.
- glibc/musl in the test rootfs resolve and call `__vdso_clock_gettime` / `__vdso_gettimeofday` / `__vdso_rt_sigreturn` without trapping into the kernel for the supported clocks.
- `SIGNAL_TRAMPOLINE` and `xsignal::arch::signal_trampoline_address()` are removed; `sigaction` writes the vDSO `rt_sigreturn` symbol address into the user signal frame's `pretcode`.
- All existing LTP cases in `src/init.sh` (notably `clock_gettime0[1-3]`, `gettimeofday*`, `kill*`, `sigaction*`, `rt_sigreturn*`) stay green on both `make rv` and `make la`.
- A new xtest C program loops `clock_gettime(CLOCK_MONOTONIC)` and verifies correctness + monotonicity under the vDSO path.
- `make rv` and `make la` boot cleanly; `make build` succeeds for both arches.

[**Related Specs**]

- `specs/features/redesign-xtest/SPEC.md` — the new vDSO micro-benchmark and signal/time LTP cases run through the xtest pipeline; no contract change, but the new C test must integrate into the `xtest/` build per that spec.
