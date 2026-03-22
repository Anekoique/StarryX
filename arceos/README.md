# ArceOS

This directory is a vendored and trimmed ArceOS tree used by StarryX.
It is not maintained here as a standalone upstream checkout.

## Scope

This copy keeps the ArceOS component code that StarryX builds on:

- `modules/` and `crates/` for the kernel substrate
- `modules/axfeat` as the retained feature-selection surface

Cargo workspace ownership now lives at the repository root. This `arceos/`
directory only keeps vendored component sources and support crates.

Upstream auxiliary content that does not serve the current StarryX tree may be removed here, such as:

- local CI files
- standalone examples
- standalone docs
- board-specific packaging helpers that StarryX does not use

Some platform implementations may still remain in source under `modules/axhal` even if their top-level configs or helper tools are trimmed. That is intentional.

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
