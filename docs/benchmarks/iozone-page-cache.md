# Page-cache iozone result

This result compares the redesigned page cache with the recorded no-page-cache
baseline. It is a strict reproducibility gate, not a shortened smoke benchmark.

## Environment and command

Each candidate sample was produced by a distinct fresh QEMU boot with:

```text
make test PROFILE=oscomp ARCH=riscv64 CASE=testsuit/iozone \
  SMP=1 MEM=1G LOG=off MODE=release
```

The comparator verifies the StarryX baseline commit, xtest gitlink, rootfs,
iozone manifest and driver hashes, rustc, QEMU, macOS, host architecture, three
distinct passed run IDs, report/serial hashes, and all expected metrics. The
complete provenance and raw samples are stored in
[iozone-page-cache.json](iozone-page-cache.json).

Candidate runs:

- `6a8ada76-16b60e70-127fb`
- `6a8adae9-36fe5a98-12add`
- `6a8adb5d-242fcb40-12db9`

All use iozone 3.506 and the unchanged package workloads (`-a`, write/read,
random, backward, stride, stdio, positional, and vector-fallback groups).

## Result

All 33 three-run medians are strictly greater than their recorded historical
values. The smallest improvement is `random.random_readers` at `+234.93%`; the
largest is `positional.pread_readers` at `+1332.39%`.

| Group | Metrics | Candidate median range | Minimum improvement | Gate |
|---|---:|---:|---:|---:|
| auto | 13 | 51,516–99,615 KiB/s | +273.83% | PASS |
| write/read | 4 | 72,810.31–91,771.93 KiB/s | +387.61% | PASS |
| random | 4 | 49,614.80–89,668.20 KiB/s | +234.93% | PASS |
| backward | 3 | 57,557.51–88,542.44 KiB/s | +291.14% | PASS |
| stride | 3 | 54,174.36–77,593.09 KiB/s | +324.41% | PASS |
| stdio | 2 | 202,029.30–238,992.04 KiB/s | +601.56% | PASS |
| positional | 2 | 79,299.35–103,454.69 KiB/s | +555.38% | PASS |
| vector fallback | 2 | 78,982.90–92,871.30 KiB/s | +506.50% | PASS |

The authoritative metric-by-metric baseline, three raw values, median,
percentage delta, artifact hashes, and pass result remain in the JSON evidence;
this summary intentionally does not replace that machine-readable record.

## Reproduction

After producing three completed run directories, run:

```text
scripts/bench/compare-page-cache-iozone \
  <run-1> <run-2> <run-3> \
  --output docs/benchmarks/iozone-page-cache.json
```

The script exits non-zero for a provenance mismatch, failed/duplicate run,
missing metric, or any median that is not strictly above its corresponding
baseline.
