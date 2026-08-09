# Crates

`crates/` 保存更底层、命名中立且不以 StarryX `x*` 命名空间为归属的通用库。它不是第三方代码的 `vendor/` 目录；部分代码来自上游项目，但在仓库中作为普通依赖维护。

| Crate | 整体作用 |
| --- | --- |
| `allocator` | 提供 bitmap、buddy、slab 和 TLSF 等通用内存分配算法。 |
| `kernel_elf_parser` | 解析 ELF、辅助向量和用户栈布局，为程序装载提供支持。 |
| `lwext4_rust` | 封装 lwext4，为 EXT4 文件系统实现提供 Rust 接口。 |
| `page_table_multiarch` | 提供多架构页表项和页表遍历、映射能力。 |
| `smoltcp` | 提供 `no_std` TCP/IP 协议栈实现。 |
| `weak-map` | 提供基于弱引用的通用映射容器。 |

若一个包采用 StarryX 自有的 `x*` 命名，并承担跨系统形态可复用的接口或机制，应放入 `xmodules/`；若它属于原 ArceOS modules 底座，则放入 `xcore/`，不因名称以 `x` 开头而迁移。
