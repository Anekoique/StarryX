# iozone 无页缓存基线

本记录是 StarryX 移除 `xkernel` 中旧 `xcache` 接入后的性能对照。它不是
一个综合“总分”：iozone 会报告不同访问模式的吞吐量，未来页缓存实现应逐项
比较相同 workload 的中位数，并同时检查正确性测试。

## 测试边界

- StarryX：本文所在提交；任务基线提交为
  `b76f4d7138e1d9bd02d660cf0bbad1c9c611ded6`。
- xtest：`59faed8281fd17234d682144a7fcd70accb0a6ad`。
- iozone：3.506，RISC-V musl 静态构建。
- guest：RISC-V 64，release，`SMP=1`、`MEM=1G`、`LOG=off`。
- QEMU：11.0.0，`virt` machine，virtio-blk PCI，raw ext4 image。
- host：Apple M4（Mac16,12），24 GiB，macOS 26.5 (25F71)。
- Rust：1.96.0-nightly (`03749d625`, 2026-03-14)。

运行命令：

```sh
make test ARCH=riscv64 CASE=testsuit/iozone/run \
  SMP=1 MEM=1G LOG=off MODE=release
```

命令独立执行三次，每次由 xtest 创建新的 disposable ext4 image 并启动新的
QEMU guest。iozone 使用 `/var/tmp/iozone-scratch`；StarryX 只把 `/tmp` 挂载为
`MemoryFs`，因此本 workload 落在 ext4 根文件系统而不是 tmpfs。

## 原始证据

| Run | xtest evidence | serial.log SHA-256 |
| --- | --- | --- |
| 1 | `target/xtest/riscv64/oscomp/6a78d70c-1ad69ee8-3ab2/` | `9d2bc3c901d98bd9f7450af84b796ab413dad724ca78b82c99d768ac0b6fa667` |
| 2 | `target/xtest/riscv64/oscomp/6a78d7a9-1d8e5090-5160/` | `86d43b7927b90389a66c0d519c345b531b1224c0a0c55f020ade3462787af201` |
| 3 | `target/xtest/riscv64/oscomp/6a78d823-38ee3800-6779/` | `37518c8c63d9ab8d74b000cffd8d2cb28920496fd1b90012bbe1d01e8fbcf183` |

三个 `report.json` 均为 `passed`：每次 1 passed、0 failed、0 timed out，
且 guest/QEMU 退出码为 0。目录位于忽略提交的 `target/` 下；上表 checksum
用于确认本次本地原始记录，长期结果以本文表格为准。

## 吞吐量结果

单位均为 kB/s。`auto.*` 是 `-a -r 1k -s 4m` 的完整结果；其余是
`-t 4 -r 1k -s 1m` 各模式的 `Children see throughput` 聚合值。parent、
单进程 min/max/avg 是同一次并发运行的派生视图，不作为跨版本主指标。

| Metric | Run 1 | Run 2 | Run 3 | Median |
| --- | ---: | ---: | ---: | ---: |
| auto.write | 19883.00 | 20745.00 | 17019.00 | 19883.00 |
| auto.rewrite | 17512.00 | 19574.00 | 16995.00 | 17512.00 |
| auto.read | 15852.00 | 18123.00 | 17395.00 | 17395.00 |
| auto.reread | 15924.00 | 18946.00 | 17359.00 | 17359.00 |
| auto.random_read | 13619.00 | 15565.00 | 15368.00 | 15368.00 |
| auto.random_write | 13535.00 | 14165.00 | 15870.00 | 14165.00 |
| auto.backward_read | 13549.00 | 9176.00 | 15109.00 | 13549.00 |
| auto.record_rewrite | 11671.00 | 8311.00 | 13612.00 | 11671.00 |
| auto.stride_read | 9899.00 | 8194.00 | 14929.00 | 9899.00 |
| auto.fwrite | 3974.00 | 7736.00 | 16848.00 | 7736.00 |
| auto.frewrite | 3655.00 | 9601.00 | 17210.00 | 9601.00 |
| auto.fread | 3087.00 | 5813.00 | 9353.00 | 5813.00 |
| auto.freread | 6595.00 | 6253.00 | 9697.00 | 6595.00 |
| write_read.initial_writers | 13666.62 | 14932.09 | 22363.42 | 14932.09 |
| write_read.rewriters | 12122.92 | 14409.15 | 16908.57 | 14409.15 |
| write_read.readers | 18597.85 | 15909.74 | 8909.53 | 15909.74 |
| write_read.re_readers | 18254.72 | 14011.29 | 12533.82 | 14011.29 |
| random.initial_writers | 17404.32 | 16475.96 | 9361.09 | 16475.96 |
| random.rewriters | 17756.47 | 15605.93 | 19154.51 | 17756.47 |
| random.random_readers | 14813.51 | 12590.05 | 20220.41 | 14813.51 |
| random.random_writers | 14321.67 | 11240.25 | 12720.95 | 12720.95 |
| backward.initial_writers | 16234.92 | 16898.54 | 14528.29 | 16234.92 |
| backward.rewriters | 9955.30 | 13813.78 | 12420.24 | 12420.24 |
| backward.reverse_readers | 15534.67 | 14022.23 | 14715.40 | 14715.40 |
| stride.initial_writers | 18203.79 | 16874.84 | 11437.13 | 16874.84 |
| stride.rewriters | 18445.13 | 15491.26 | 12544.87 | 15491.26 |
| stride.stride_readers | 15961.37 | 12764.59 | 11399.31 | 12764.59 |
| stdio.fwriters | 34065.80 | 35258.54 | 26117.75 | 34065.80 |
| stdio.freaders | 12541.02 | 16968.13 | 15157.02 | 15157.02 |
| positional.pwrite_writers | 12099.74 | 18034.79 | 9435.19 | 12099.74 |
| positional.pread_readers | 7222.52 | 16494.04 | 3668.20 | 7222.52 |
| vector_fallback.initial_writers | 15807.32 | 11733.43 | 13022.66 | 13022.66 |
| vector_fallback.rewriters | 13651.70 | 13076.74 | 11120.43 | 13076.74 |

iozone 3.506 对 `-i 11 -i 12` 输出 `Selected test not available on the
version`，随后只产生 initial writer/rewriter 数据。因此最后两行是该 invocation
的 fallback 结果，不得解释为 pwritev/preadv 性能；未来比较必须保留相同行为，
或在两侧同时升级 workload 后建立新的基线。

## 后续比较规则

1. 保持架构、QEMU、CPU/内存、rootfs、iozone 参数和 scratch 路径一致。
2. 每个实现执行三次 fresh-boot run，并以逐项中位数作为主比较值。
3. 同时报告三个原始值，避免用波动较大的单次结果宣称优化比例。
4. 页缓存版本必须先通过 `cases`、iozone 八阶段和存储正确性测试。
5. 分别解释 read/reread、write/rewrite、随机、stdio 和 positional 的变化，
   不把不同访问模式相加为缺乏含义的总分。
