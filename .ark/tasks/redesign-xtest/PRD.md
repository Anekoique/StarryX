# `redesign-xtest` PRD

---

[**What**]
Replace the dead `xtest/` sdcard-build pipeline with a test-rootfs pipeline that bakes our own C tests and the OS-COMP `testsuites-for-oskernel/pre-2025` suites into a copy of the upstream Alpine `rootfs-$ARCH.img`, boots it under a dedicated `make tests` / `make run-tests` flow, and runs everything via a kernel-side `src/test.sh` script that mirrors `src/init.sh`.

[**Why**]
`xtest/` today builds an `sdcard-{rv,la}.img` for an old contest setup that nothing references — `src/init.sh` boots straight into Alpine `sh`, the LTP/contest blocks are commented out, and the dual glibc/musl staging predates the Alpine rootfs. We have no in-repo way to run regression suites or our own user-space tests against the kernel. The redesign turns `xtest/` into the actual test environment: vendored upstream suites + first-party C tests, a Docker-driven build, and a separate boot path so the normal `make run` user experience stays untouched.

[**Outcome**]
- `xtest/` layout is `xtest/{c,testsuites,scripts}` with all old files (`Makefile`, `Makefile.sub`, `config/`, `scripts/git_testcode.sh`) deleted.
- `xtest/c/` holds single-file C tests (one `.c` per test, one ELF per test) cross-compiled inside the contest Docker image.
- `xtest/testsuites/{basic,busybox,libctest,libcbench,lua,iozone,iperf,netperf,cyclictest,lmbench,ltp,splice,copy-file-range,interrupts}/` contains the relevant subset of `oscomp/testsuites-for-oskernel @ pre-2025` vendored directly into the repo (no submodule).
- `src/test.sh` exists alongside `src/init.sh` and is the kernel-side boot script for the test rootfs; it sets up `PATH`/`LD_LIBRARY_PATH`/`HOME`, `cd`s into `/root/tests`, runs `xtest/scripts/run-all.sh` (staged into the image), and on completion drops to an interactive shell.
- `make tests ARCH={riscv64|loongarch64}` builds `tests-rootfs-$ARCH.img` by copying `rootfs-$ARCH.img`, mounting it inside the contest Docker image, copying `xtest/build/<arch>/{c,testsuites,scripts}` to `/root/tests`, and installing `src/test.sh` as the boot entry; `make run-tests` builds and boots that image in QEMU using the existing kernel build path.
- `make run` and `src/init.sh` remain bit-for-bit unchanged; the normal rootfs path is unaffected.
- At least the `basic` suite plus a handful of first-party C smoke tests run end-to-end on `riscv64` and `loongarch64` and produce visible pass/fail output (binary-exit-status; non-zero is recorded, not fatal).

[**Related Specs**]
None — no project or feature specs exist yet. This task will be the first feature SPEC promoted (`specs/features/xtest/SPEC.md`).
