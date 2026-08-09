# Feature Specs

Feature specifications extracted from deep-tier tasks at commit. Layout: `<feature>/SPEC.md`.

The table below is managed by `ark agent spec register` — new rows appear when a deep-tier task is committed with a promoted SPEC. **Do not hand-edit rows between the markers.** Edit outside the block, or let the CLI do it.

## Index

<!-- ARK:FEATURES:START -->
| Feature | Scope | Promoted |
| ------- | ----- | -------- |
| `redesign-xtest` | Redesign xtest as test-rootfs pipeline | 2026-05-05 from task `redesign-xtest` |
| `vdso-support` | Add vDSO support | 2026-05-06 from task `vdso-support` |
| `xtest/INDEX.md` | redesign xtest framework | 2026-08-09 from task `redesign-xtest-framework` |

<!-- ARK:FEATURES:END -->

---

## How to use

- **Read:** scan the table; open the SPEC for any feature you'll touch.
- **Modify a feature SPEC:** append a `[**CHANGELOG**]` entry. Ark re-writes the `Promoted` column with the latest touch date.
