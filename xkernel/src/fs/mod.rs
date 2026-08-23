//! Filesystem and pseudo-filesystem implementation.
//!
//! The [`pseudofs`] module provides in-memory kernel filesystems including:
//! - `/dev` - Device filesystem (devfs)
//! - `/tmp` - Temporary filesystem (tmpfs)
//! - `/proc` - Process information filesystem (procfs)
//! - `/etc` - Kernel-provided system configuration files
#![allow(dead_code)]
#![allow(clippy::len_without_is_empty)]

pub mod api;
pub mod cache;
pub mod fanotify;
pub mod fd;
pub mod file;
pub mod pseudofs;

pub use api::*;
pub use fanotify::*;
