# xtest framework redesign PRD

---

[**What**]

Move the verified xtest redesign out of the StarryX source tree into the
standalone `Anekoique/xtest` repository, load that repository at `xtest/` as a
StarryX Git submodule, and preserve the redesigned Rust host runner, POSIX guest
runner, target-compiled supervisor, immutable TestPlan/TestBundle/TestEvent
contracts, and all eleven previously supported system testsuits.

[**Why**]

The redesigned framework already fixes the important runtime problems in the
old suite-aware shell pipeline: it owns QEMU lifetime, enforces host and guest
timeouts, reaps descendants, injects a copied ext4 image without privileged
mounts, validates a versioned serial protocol, and returns reliable JSON/TAP
and process status. Keeping that implementation as ordinary files inside
StarryX would still couple framework releases, first-party user tests, and
large third-party suite data to kernel history.

The standalone repository is the correct ownership boundary. StarryX should
only provide the kernel/QEMU seam, the normal-init bundle dispatch, and Linux
process semantics required by the guest supervisor. The xtest repository
should own framework code, test data, profiles, and package-local integration
for `basic`, `busybox`, `cyclictest`, `iozone`, `iperf`, `libcbench`,
`libctest`, `lmbench`, `lua`, `netperf`, and `unixbench`.

[**Outcome**]

- StarryX tracks `https://github.com/Anekoique/xtest.git` at `xtest/` through a
  root `.gitmodules` entry and a Git gitlink; no framework or testsuit source is
  copied into the StarryX repository.
- The standalone xtest repository has explicit `src/`, `guest/`, `cases/`,
  `testsuits/`, and `profiles/` ownership boundaries. Generated host Cargo and
  run artifacts remain outside committed source.
- A single safe Rust host crate resolves profiles, builds cases and testsuit
  packages, creates an immutable TestPlan/TestBundle, injects it into a copied
  rootfs image, controls QEMU through the selected kernel repository, validates
  serial events, and writes JSON/TAP reports.
- A single POSIX guest runner reads only the generated plan and emits versioned
  `XTEST/1` events. A target-compiled supervisor owns each case process group,
  monotonic timeout, child-subreaper role, and descendant reaping.
- First-party C cases build conventionally from `cases/`; package-specific
  build and result normalization for external suites is confined to
  `testsuits/<name>/` and never appears in generic `src/` or `guest/` code.
- All eleven prior testsuits are represented by versioned manifests and build
  packages. Default profiles may exclude architecture-incompatible or known
  hanging suites, but those suites remain explicitly buildable/selectable and
  their policy is declarative rather than hard-coded in the framework.
- Testsuit outputs are treated as sealed package trees: declared entrypoints
  and working directories remain beneath `XTEST_OUT`, symlinks and special
  files are rejected, and required scripts/data are copied with the executable
  into the guest bundle.
- Test-image construction never requires privileged Docker, loop devices, or
  host mounts. A testsuit may use a pinned non-privileged toolchain container
  through its package-local build contract when native host tooling is absent;
  image injection remains host-side e2fsprogs.
- Batch runs enforce a host wall-clock timeout, preserve serial evidence,
  terminate and reap QEMU, and return non-zero for test failure, timeout,
  protocol error, or unexpected QEMU exit.
- `make test ARCH={riscv64|loongarch64}` remains the StarryX one-command entry.
  The host runner also accepts an explicit kernel-root argument so the xtest
  repository can be developed and tested independently.
- A fresh recursive StarryX checkout resolves the published xtest gitlink, and
  the xtest revision used by StarryX exists on the public xtest remote before
  the parent gitlink is published.

[**Related Specs**]

- `specs/features/redesign-xtest/SPEC.md` — preserves isolated copied-rootfs
  testing, dual-architecture intent, first-party cases, and normal-boot
  isolation while superseding its Make/shell runner, privileged image builder,
  `init-test` feature, staged-tree layout, and unconditional-success semantics.
- `specs/features/xtest/port-oscomp-suites/SPEC.md` — preserves the complete
  eleven-suite coverage and recorded compatibility knowledge while replacing
  StarryX-side vendoring, central suite adapters, and hard-coded quarantine with
  standalone package-local manifests and declarative profiles.

[**SPEC Path**]

xtest/redesign-xtest-framework
