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

iozone is a suite like the rest under `xtest/testsuites/`: its `BUILD.sh`
(dispatched by `xtest/scripts/build/build-suites.sh` inside the contest Docker
image) cross-compiles `src/` into one static musl ELF, mirroring upstream's
`linux-AMD64` recipe (threads + largefiles + async I/O + SysV shared memory).
It links plain `-static` — the contest musl toolchain turns that into the
loadable static-PIE the StarryX user-ELF loader runs (never `-static-pie`/
`-fPIE`, which would fault the loader).

## Run

In the booted guest, the vendored `iozone_testcode.sh` runs the standard
OS-COMP iozone sequence (automatic mode plus seven `-t 4` throughput modes);
`run-suite.sh` drives it and `adapt_iozone` maps each stage to
`[PASS]/[FAIL] iozone/<stage>`.
