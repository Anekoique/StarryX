# iozone (vendored)

Filesystem benchmark used as a StarryX I/O smoke test.

## Provenance

- Upstream: <https://www.iozone.org/>
- Source tarball: `iozone3_506.tar` (`src/current/`)
- Version: 3.506 (`$Revision: 3.506 $`, dated 2023-05-01)
- Imported: 2026-06-23
- SPDX: see `LICENSE` — free use/distribute with copyright notice intact
  (William D. Norcott / Don Capps). Not an OSI-listed identifier; treated as a
  permissive custom license, recorded here verbatim.

## Vendored files

Only the files needed to build the benchmark are kept (`src/`):

- `iozone.c`   — the benchmark
- `libasync.c` — POSIX AIO helpers (linked under `-DASYNC_IO`)
- `libbif.c`   — block-interface helpers

The upstream `makefile`, docs, gnuplot tooling, and platform variants we do not
build are intentionally omitted.

## Build

`xtest/scripts/build/build-iozone.sh` cross-compiles these into one static-PIE
musl ELF, mirroring upstream's `linux-AMD64` recipe (threads + largefiles +
async I/O + SysV shared memory). The static-PIE format matches the first-party C
tests and is the only executable type the StarryX user-ELF loader accepts.

It runs inside the contest Docker image (whose musl `libc.a` is PIC-capable);
a stock host musl toolchain that ships a non-PIC `libc.a` cannot link
`-static-pie` and is not supported for this suite.

## Run

In the booted guest, `xtest/scripts/run-iozone.sh` runs a bounded smoke pass
(4 MB file, 64 KB record, write/read tests) and reports `[PASS]/[FAIL] iozone`.
