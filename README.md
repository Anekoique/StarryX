# StarryX

A monolithic kernel based on arceos/starry.

## 📦 项目结构

<div style="text-align: center;">
  <img src="./docs/starry/images/structure.png" alt="structure" width="70%">
</div>

```shell
.
├── apps
├── arceos
├── api
├── core
├── src
├── modules
├── configs
├── docs
├── Makefile
├── Cargo.toml
├── README.md
└── rust-toolchain.toml
```

## 🛠️ 构建方式

构建全国大学生操作系统内核赛测例 (riscv / loongarch)

```shell
make all
```

构建自定义程序 (riscv / loonarch / aarch / x86)

```shell
make AX_TESTCASE=libc user_apps ARCH={ARCH}
```

## 🚀 运行方式

运行全国大学生操作系统内核赛测例 (riscv / loongarch)

```shell
make rv ARCH=riscv64 run
make la ARCH=loongarch64 run
```

运行全国大学生操作系统内核赛测例  (riscv / loongarch / aarch / x86)

```shell
make oscomp_run ARCH={ARCH}
```

运行自定义APP

```shell
make AX_TESTCASE=libc run_apps ARCH={ARCH}
```

## ✨ 项目说明

StarryX是基于组件化操作系统ArceOS的宏内核扩展实现，完整实现了进程管理，文件系统，信号系统等功能，并通过硬件抽象层axhal能够运行在四个架构上（riscv64 / loongarch64 / x86_64 / aarch64）,截止目前我们实现了约140条系统调用，完整通过了初赛的basic，busybox，libtest，lmbench，libcbench，lua，iozone相关测例，我们完整撰写了初赛文档和PPT，在开发过程中我们始终践行组件化内核的开发理念与原则，并详细记录了我们参加开源操作系统训练营、学习和开发ArceOS/Srarry的过程。

[初赛文档](./docs/StarryX.pdf)

[初赛PPT](./docs/StarryX.pptx)

[初赛视频](https://pan.baidu.com/s/1x0gMF_K7H1GuciTz_kKf5g?pwd=ftns) 提取码：ftns

[学习/开发日志](./docs/record.md)

[Github代码仓库](https://github.com/Anekoique/StarryX)

我们主要完成了以下工作

- 修复基座已有代码的bug
- 完善和扩展实现系统调用
- 组织和重构Starry-API

- 时钟管理
- 虚拟文件系统
- 文件页懒分配
- 信号机制完善
- 负载均衡
- 动态调度器
- System V IPC通信机制

我们后续希望完成的工作

- 适配板卡
- 网络栈完善
- Copy-On-Write与页缓存
- 文件系统重构与改进
- 系统调用完善与添加

### Reference

StarryX的基座代码为ArceOS和Starry-next，并在后续开发过程中合并了大部分主线进展，其中ArceOS的文件系统修改使用了清华大学开发的axfs-ng

基座仓库：

[oscomp/starry-next](https://github.com/oscomp/starry-next/tree/0c84473be8a7e3876c62d69f63c2853f404df3a9) commit: 0c8447c

[oscomp/arceos](https://github.com/oscomp/arceos/tree/dec6f341785079143dcd55f48e0b8764ead3029f) commit: d1f0c64

[axfs_ng](https://github.com/Mivik/arceos/tree/fs/modules/axfs-ng) branch: fs

系统调用实现参考：

[learningos](https://learningos.cn/oscomptest-grading/)

[asterinas](https://github.com/asterinas/asterinas/tree/main)

[byteos](https://github.com/oscomp/ByteOS)
