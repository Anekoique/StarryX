# 1 概述

## 1.1 简介

## 1.2 系统架构

## 1.3 完成情况

# 2 启动与初始化

## 1.1 硬件初始化

axhal部分 （arch + platform）

## 1.2 组件初始化

axhal -> axruntime (axruntime do crate init ...) -> starry main

## 1.3 宏内核初始化

starry main spawn task and mm init ...

# 3 进程管理

## 1.1 整体架构

axtask(task runqueue waitqueue)

## 1.2 任务结构

task structure

## 1.3 任务调度

axsched 

multicore sched(runqueue)

## 1.4 任务扩展

task ext  -> thread(xprocess) -> process(xprocess) -> xthread,xprocess

## 1.5 进程通信

System V ipc

# 4 内存管理

## 1.1 整体架构

axmm + axalloc + pagetable_multiarch

## 1.2 内存分配器

axalloc + allocator

## 1.3 地址空间管理

axmm (backend(leaner + alloc + shared) design)

## 1.4 延迟分配技术

copy on write 

xvma lazy alloc file page

## 1.5 用户地址访问

xuspace

# 5 文件系统

## 1.1 整体架构

axfs-vfs -> axfs -> xcore fs

## 1.2 虚拟文件系统

unix-like vfs design: disk -> inode -> direntry -> location -> fs_context

## 1.3 文件系统实例

fat + ext4

## 1.4 缓存设计

block cache 

direntry cache

page cache(xcache)

## 1.5 抽象文件

xcore XFile -> FileLike design

virt_file virt_fs -> /proc /dev /tmp

# 6 网络

## 1.1 整体架构

smoltcp -> axnet -> xcore net

## 1.2 TCP套接字

tcp operations

## 1.3 UDP套接字

udp operations

# 7 组件化与抽象解耦

## 1.1 设计理念

## 1.2 xuspace

## 1.3 xprocess

## 1.4 xcache

## 1.5 xvma

## 1.6 xsignal

## 1.7 xutils

# 8 应用支持