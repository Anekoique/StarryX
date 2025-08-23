//! Virtual filesystems implementation
//!
//! This module provides various virtual filesystems including:
//! - `/dev` - Device filesystem (devfs)
//! - `/tmp` - Temporary filesystem (tmpfs)
//! - `/proc` - Process information filesystem (procfs)
#![allow(dead_code)]
#![allow(clippy::len_without_is_empty)]

pub mod api;
pub mod fanotify;
pub mod fd;
pub mod file;
pub mod vfs;

pub use api::*;
pub use fanotify::*;
