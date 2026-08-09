# vDSO

StarryX maps an external Linux `linux-vdso.so.1` image into every user
address space. `xmodules/xvdso` is the reusable trusted component: it obtains
and embeds the external image, owns the Linux-compatible vvar pages, updates
time data, and resolves vDSO symbols. `xkernel/src/vdso.rs` is a small profile
adapter that maps those shared pages into each process.

There are no committed vDSO blobs, local vDSO source, regeneration target, or
Docker-specific setup.

## External image

`xmodules/xvdso/build.rs` clones the default provider into Cargo's `OUT_DIR`,
checks out the pinned revision, and reuses that checkout on later builds:

| Setting | Default |
| --- | --- |
| Repository | `https://github.com/asterinas/linux_vdso.git` |
| Revision | `74898350d406d6cd8988531ad737380a8e2cdbf4` |

The component selects the image by target architecture:

| Architecture | Required file |
| --- | --- |
| RISC-V 64 | `vdso_riscv64.so` |
| LoongArch 64 | `vdso_loongarch64.so` |

The ordinary RISC-V build therefore needs no preparation:

```sh
make build ARCH=riscv64
```

For an offline build or a previously prepared checkout, bypass cloning with
`XVDSO_SOURCE_DIR`:

```sh
XVDSO_SOURCE_DIR=/path/to/linux_vdso make build ARCH=riscv64
```

`XVDSO_REPOSITORY` and `XVDSO_REVISION` select another provider and immutable
revision. For a managed checkout, the build script verifies the remote URL,
checked-out commit, and architecture-specific file before passing its absolute
path to the component. `XVDSO_SOURCE_DIR` is an explicit offline escape hatch:
it verifies the required file, while the caller owns checkout provenance. The
managed checkout stays in build output and never modifies `xmodules/xvdso/`.

The Asterinas repository currently has no LoongArch image. A LoongArch build
must set `XVDSO_SOURCE_DIR`, or select a compatible provider containing
`vdso_loongarch64.so`; it must use the Linux 6.8 `vdso_data` ABI and the
documented three-page vvar layout. StarryX never substitutes a RISC-V image
for LoongArch.

## Address-space layout

The vvar area precedes the ELF image, matching Linux's PC-relative layout:

```text
low address
  USER_VDSO_DATA       Linux vdso_data page       R--, shared
  + 0x1000             time-namespace slot        unmapped
  + 0x2000             LoongArch data page        R--, shared (LoongArch only)
  USER_VDSO_BASE       linux-vdso.so.1             R-X, shared
high address
```

RISC-V therefore places `USER_VDSO_BASE` two pages after
`USER_VDSO_DATA`; LoongArch places it three pages after the data base.
`AT_SYSINFO_EHDR` points to `USER_VDSO_BASE`.

The code mapping is backed directly by the page-aligned image embedded by
`xvdso`. `execve` does not allocate a private copy, write an absolute data
pointer, or perform an RW-to-RX permission transition.

## Linux data ABI

The shared page matches Linux 6.8 `include/vdso/datapage.h` and contains the
two clocksource records used by Linux:

- `CS_HRES_COARSE` for realtime, monotonic, boottime, and coarse clocks;
- `CS_RAW` for `CLOCK_MONOTONIC_RAW`.

The boot CPU publishes `cycle_last`, `mult`, `shift`, and base timestamps
through `xvdso` under Linux sequence counters. Other CPUs do not write the
page. Component writes are serialized with a no-IRQ spin lock, while
userspace sees only a read-only mapping.

## Signal return

The external ELF exports `__vdso_rt_sigreturn`. `xvdso` parses its dynamic
symbol offset once; the kernel adapter adds the process's vDSO base and
installs the resulting address as the default signal restorer. This avoids
Asterinas's image-specific hard-coded `0x5b0` offset.

## Tests

Tests live under `xtest/c/time/`:

- `vdso_clock_monotonic.c`
- `vdso_gettimeofday.c`
- `vdso_clock_getres.c`
- `vdso_rt_sigreturn.c`

Run them through the normal build. Set `XVDSO_SOURCE_DIR` only when an offline
checkout or a non-default provider is required:

```sh
make tests ARCH=riscv64
make run-tests ARCH=riscv64
```
