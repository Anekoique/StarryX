# XCore

XCore is StarryX's collection of low-level modules derived from ArceOS. It is
not maintained as a standalone upstream checkout.

The rename establishes a project-owned namespace and component boundary. It
does not yet mean that every crate in this directory belongs to a minimal
trusted computing base: reducing and enforcing that boundary is follow-up
architecture work.

## Scope

This directory contains only the former ArceOS module family, including
`xhal`, `xalloc`, `xtask`, `xmm`, `xdriver`, `xfs`, `xnet`, and `xfeat`.
Reusable `x*` components outside the former ArceOS module family live in
`xmodules/`; lower-level and general-purpose support crates live in `crates/`;
driver crates live in `drivers/`.

Upstream auxiliary content that does not serve the current StarryX tree may be removed here, such as:

- local CI files
- standalone examples
- standalone docs
- board-specific packaging helpers that StarryX does not use

Some platform implementations may still remain in source under `xhal` even if
their top-level configs or helper tools are trimmed. That is intentional.

## Build

Use the StarryX root make targets when working in this repository:

```bash
make rv
make la
make vf2
```

## Retained Top-Level Platforms

The build configs, helper scripts, and workspace manifest now live at the repository root. The retained platform implementations under this vendored tree still primarily target:

- `riscv64-qemu-virt`
- `riscv64-visionfive2`
- `loongarch64-qemu-virt`
