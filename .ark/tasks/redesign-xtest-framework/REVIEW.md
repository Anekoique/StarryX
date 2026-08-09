# xtest framework redesign REVIEW

> Status: Delta review complete
> Feature: `redesign-xtest-framework`
> Owner: Reviewer
> Target Plan: `PLAN.md`
> Scope: Plan correctness · Spec alignment · Design soundness · Validation adequacy · Trade-off advice

---

## Verdict

- Decision: Approved
- Blocking: 0
- Non-blocking: 2

## Summary

The original runtime findings were folded into the Plan and implementation.
The later standalone-repository amendment has now received a focused delta
review: framework/kernel roots are independent, testsuits use one sealed
package contract, the published xtest revision precedes the StarryX gitlink,
and generic host/guest code remains free of suite-specific policy. No blocking
finding remains.

---

## Post-review Scope Amendment

After the first implementation, the deployment boundary changed: the framework
is published from `Anekoique/xtest`, consumed by StarryX as a gitlink, and owns
package-local integration for all eleven previous testsuits. The focused delta
review checked cross-repository root separation, gitlink release ordering,
sealed multi-file package trees, and the eleven package manifests. Findings
R-001 through R-007 remain the historical design record and are resolved by the
final Plan and implementation.

## Standalone-repository Delta Review

- **Repository boundary:** PASS — `framework_root` is anchored to the xtest
  crate while an explicit/fallback `kernel_root` owns rootfs, Make, QEMU, and
  evidence paths. No nested-checkout assumption remains.
- **Package boundary:** PASS — manifests declare relative entrypoints and
  optional working directories beneath a recursively validated `XTEST_OUT`;
  symlinks, special files, traversal, root replacement, and non-executable
  entrypoints fail before image creation.
- **Genericity:** PASS — all eleven names, build workarounds, architecture
  restrictions, and quarantine choices are confined to `testsuits/` and
  profiles. `src/` has no suite dispatch or output parser; BusyBox in the POSIX
  guest runner is a guest primitive, not the BusyBox testsuit integration.
- **Release order:** PASS — xtest commit `9435be9` is reachable from the public
  remote's `main` and feature refs before StarryX records the gitlink.
- **Integration:** PASS — direct invocation, fresh-clone unit tests, StarryX
  submodule invocation, real ext4/QEMU smoke, and descendant-reaping evidence
  all exercise the same published framework sources.
- **Accepted limitations:** LoongArch QEMU remains blocked by the pre-existing
  external vDSO blob set; post-poweroff ext4 inconsistency remains a separate
  StarryX storage issue. Neither changes the xtest ownership or execution
  contracts.

---

## Findings

### R-001 Guest timeout contract is not implementable as written

- **Severity:** HIGH
- **Section:** Architecture / Runtime
- **Problem:** The POSIX runner must continue after a timed-out case, but the Plan
  names no dependable guest timeout or process-group primitive.
- **Why it matters:** A hanging case or surviving child can force the host global
  timeout and prevent a reliable per-case timeout result.
- **Recommendation:** Require BusyBox `setsid`, kill, and sleep primitives; define
  watchdog marking, TERM/KILL of the case process group, reaping, timeout
  classification, and fail-fast behavior when those primitives are absent.

### R-002 Kernel/QEMU Make contract is incomplete

- **Severity:** HIGH
- **Section:** API Surface / Main Flow / Phase 5
- **Problem:** `make build + make justrun` does not specify required `BLK=y`,
  virtio-blk features, `DISK_IMG`, `ARCH`, and network configuration.
- **Why it matters:** The kernel may build successfully while the guest cannot see
  the disposable test rootfs.
- **Recommendation:** Define one internal Make target invoked by the host. It must
  recursively build and run with one resolved variable set and the disposable
  image. Validate the final QEMU argv.

### R-003 `requires` is declared but has no semantics

- **Severity:** HIGH
- **Section:** Architecture / Data Structure
- **Problem:** `CaseMetadata.requires` has no closed values or mapping and is lost
  during resolution.
- **Why it matters:** An executor cannot know whether it changes kernel features,
  devices, filtering, or validation.
- **Recommendation:** Remove `requires` from this iteration instead of reserving an
  undefined extension point.

### R-004 Immutable run artifacts contradict the proposed layout

- **Severity:** HIGH
- **Section:** Architecture / Failure Flow
- **Problem:** `target/xtest/<arch>/<profile>/` would overwrite repeated runs even
  though the Plan promises immutable evidence and run-ID correlation.
- **Why it matters:** A retry can destroy the previous bundle, image, log, or report.
- **Recommendation:** Store each run under `<profile>/<run-id>/`, build under a
  partial directory, publish by rename, and keep host Cargo artifacts separate.

### R-005 Plan and testsuit artifact containment is underspecified

- **Severity:** HIGH
- **Section:** Architecture / Testsuit Manifest
- **Problem:** The Plan constrains IDs but not manifest program paths, symlinks,
  file type, host containment, or argument characters POSIX shell cannot preserve.
- **Why it matters:** A manifest can escape `XTEST_OUT`, create guest traversal, or
  violate the promised argv round trip.
- **Recommendation:** Require canonical relative regular-file programs inside
  `XTEST_OUT`, reject absolute/parent/symlink escapes, generate fixed guest paths,
  and define a control-character-free argument contract.

### R-006 `debugfs` compatibility needs an explicit preflight

- **Severity:** MEDIUM
- **Section:** Image Construction
- **Problem:** Host e2fsprogs compatibility with both pinned base images is assumed.
- **Recommendation:** Preflight both image features and required operations; fail
  fast and verify injected inode type, mode, and content hash plus e2fsck status.

### R-007 Serial protocol trust boundary is overstated

- **Severity:** MEDIUM
- **Section:** TestEvent
- **Problem:** Cases share the serial channel and can read the run ID, so a trusted
  case can print protocol-looking lines.
- **Recommendation:** State that cases are trusted and `XTEST/1` is reserved;
  duplicate or early events are protocol errors rather than overrides.

---

## Trade-off Advice

### TR-1 Keep one host runner, but minimize external contract

- **Related Plan Item:** `T-6`
- **Topic:** Flexibility vs Safety
- **Reviewer Position:** Prefer fixed schema
- **Advice:** Keep one crate and one fixed testsuit manifest. Remove fields without
  current runtime semantics instead of building a provider/plugin layer.
- **Rationale:** The current task has one framework and no concrete external suite.
- **Required Action:** Adopt

### TR-2 Reuse Make through one explicit seam

- **Related Plan Item:** `T-7`
- **Topic:** Compatibility vs Clean Design
- **Reviewer Position:** Prefer one dedicated Make target
- **Advice:** Centralize the resolved test image, arch, feature, block, and network
  variables in one internal target invoked by the host runner.
- **Rationale:** It reuses platform configuration without duplicating or drifting
  build and launch variables.
- **Required Action:** Adopt
