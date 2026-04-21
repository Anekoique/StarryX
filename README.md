#  StarryX

A macrokernel based on arceos unikernel.

First prize in CSCC OS Competition 2025.

## Architecture

<div style="text-align: center;">
  <img src="./docs/StarryX/images/structure.png" alt="structure" width="60%">
</div>

```shell
.
├── rust-toolchain.toml      // Toolchain configuration
├── docs                     // Documentations
├── Makefile                 // Build scripts
├── Cargo.toml               // Rust workspace configuration and dependencies
├── scripts                  // Build and QEMU helper scripts
├── arceos                   // Vendored ArceOS component crates/workspace
├── src                      // Entry OS
├── xapi                     // Posix API
├── xcore                    // OS Core
├── xmodules                 // Macrokernel modules 
└── xtest                    // Linux APP for tets
```

## Build and run

Run Alpine-Linux:

```shell
make rv
make la
```

Port to Vision-Five2:

```shell
make vf2
```

## Description

StarryX操作系统是基于组件化操作系统ArceOS的宏内核扩展实现，完整实现了进程管理，内存管理，文件系统，信号系统等模块。[学习/开发日志](./docs/record.md);[决赛文档](./docs/StarryX.pdf);[决赛PPT](./docs/StarryX.pptx)

截至目前，StarryX共实现约200项系统调用，包括进程管理、文件系统、内存管理、网络等各个模块的系统调用，能够运行大量LTP测例，并能够运行Redis、Git、Gcc等Linux应用。

<div style="text-align: center;">
  <img src="./docs/StarryX/images/rank.png" alt="structure" width="100%">
</div>

StarryX的各个子模块实现均较为完善，完成情况如下：

| 子模块   | 完成情况                                                     |
| -------- | ------------------------------------------------------------ |
| 文件系统 | 类Linux 的 VFS设计 <br>丰富的抽象文件支持和完善的伪文件系统实现<br>支持EXT4与FAT文件系统<br>模块解耦的页缓存设计 |
| 内存管理 | 懒分配、写时复制<br>模块解耦的用户空间访问<br>模块解耦的文件映射管理 |
| 进程管理 | 多核场景下的负载均衡<br>多种调度方式支持<br>完整实现的System V进程通信<br>模块解耦的进程管理 |
| 网络模块 | 支持TCP和UDP套接字                                |
| 信号系统 | 模块解耦的信号系统<br>可被信号中断的系统调用                 |

## License

Tri-licensed, matching the ArceOS upstream. Pick whichever fits:

[GPL-3.0-or-later](./LICENSE.GPLv3)

[Apache-2.0](./LICENSE.Apache2)

[MulanPSL-2.0](./LICENSE.MulanPSL2) / [MulanPubL-2.0](./LICENSE.MulanPubL2)
