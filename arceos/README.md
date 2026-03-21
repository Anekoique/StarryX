# ArceOS

This directory is a vendored and trimmed ArceOS tree used by StarryX.
It is not maintained here as a standalone upstream checkout.

## Scope

This copy keeps the ArceOS core code that StarryX builds on:

- `modules/` and `crates/` for the kernel substrate
- `modules/axfeat` as the retained feature-selection surface
- `configs/` and `scripts/` needed by the local build flow

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

You can also invoke the vendored ArceOS makefile directly:

```bash
make -C arceos ARCH=riscv64 BUS=mmio A=$(pwd) build
```

## Retained Top-Level Platforms

The trimmed top-level configs in this vendored copy currently keep these build targets:

- `riscv64-qemu-virt`
- `riscv64-visionfive2`
- `loongarch64-qemu-virt`
- `x86_64-qemu-q35`
- `aarch64-qemu-virt`
