//! Core functionality of the StarryX monolithic kernel
//!
//! This module provides the core components for managing user processes,
//! memory, filesystem operations, and system resources in a monolithic kernel.
//!
//! ## Modules
//!
//! - [`fs`] - Virtual filesystem implementations including devfs, tmpfs, and procfs
//! - [`mm`] - Memory management including user address space and mmap regions
//! - [`resources`] - Resource limit management (rlimits) for processes
//! - [`task`] - User task and process management with signal handling
//! - [`time`] - Timer and time statistics for process scheduling
//!
//! ## Key Features
//!
//! - **Process Management**: Complete lifecycle management of user processes and threads
//! - **Memory Management**: Virtual memory areas, mmap regions, and user address spaces
//! - **Virtual Filesystems**: Device files, temporary storage, and process information
//! - **Signal Handling**: POSIX-compliant signal delivery and handling
//! - **Resource Control**: Process resource limits and usage tracking
//! - **Synchronization**: Futex-based userspace synchronization primitives

#![no_std]

#[macro_use]
extern crate axlog;
extern crate alloc;

/// Architecture-specific configurations
pub mod config;
/// Virtual filesystem implementations and operations
pub mod fs;
/// Memory management and address space operations
pub mod mm;
/// Process resource limits and management
pub mod resources;
/// Task and process management with signal handling
pub mod task;
/// Timer and time tracking for processes
pub mod time;
