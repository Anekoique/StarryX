# XCore：底层组件集

`xcore/` 只保存 StarryX 从 ArceOS modules 裁剪并演进出的底层组件，例如 `xhal`、`xalloc`、`xtask`、`xfs` 和 `xnet`。StarryX 自有的可复用组件统一位于 `xmodules/`，其中也包括从 `crates/` 迁入的 `xerrno`、`xio`、`xsched` 与 `xvfs`。

它目前提供启动与硬件抽象、内存与任务机制、设备驱动、文件系统及网络等基础能力。`xmodules` 提供可复用的项目基础契约和较高层内核机制，`xkernel` 则拥有宏内核状态与 Linux ABI。

```text
xkernel -> xmodules / xcore
xmodules -> xcore / crates
xcore -> xmodules / drivers / crates
```

当前本地 crate、Rust API、宏和构建环境已经统一使用 `x*`、`X*` 与 `XCORE_*` 命名。仍可见的 `axconfig-gen`、`axconfig-macros` 和 ArceOS URL 属于外部工具、发布包及上游来源；其中宏包的发布名称兼容被限制在 `xconfig` 内部。

这次目录整理进一步限定了 XCore 的准入边界：只有原 ArceOS module 系列可以放入 `xcore/`。`xmodules/` 是 StarryX 自有 `x*` 组件的归属边界，不强制表达单向层级，因此 XCore 可以依赖其中的基础契约；通用 crate、第三方 crate、驱动 crate 和平台配置仍位于仓库根部对应目录。`xcore/` 目前仍不等同于已经完成收缩与审计的最小可信核心。

`xfs` 是 XCore 的通用文件系统服务组件，名称不指 Linux 的 XFS 磁盘文件系统；文件系统中立接口位于 `xvfs`。
