//! # Axfs-ng VFS (Virtual File System)
//!
//! This crate provides a virtual file system abstraction layer for the Axfs-ng filesystem.
//! It offers a unified interface for working with different filesystem implementations
//! through mountpoints, locations, and file/directory operations.
//!
//! ## Main Components
//!
//! - **Filesystem**: Core filesystem abstraction
//! - **Mountpoint**: Mount points for filesystem hierarchies
//! - **Location**: Represents a location within the filesystem tree
//! - **DirEntry**: Directory entries (files and directories)
//! - **Path utilities**: Path manipulation and normalization
//! - **Types**: Common data types for metadata, permissions, etc.
//!
//! ## Features
//!
//! - `no_std` compatible
//! - Mount/unmount operations
//! - File and directory operations
//! - Path resolution and normalization
//! - Metadata management
//! - Cross-platform filesystem abstraction

#![no_std]
#![feature(trait_upcasting)]

extern crate alloc;

/// Filesystem operations and abstractions
mod fs;
/// Mount point management and location handling
mod mount;
/// File system nodes (files and directories)
mod node;
/// Path manipulation utilities
pub mod path;
/// Common types and data structures
mod types;

pub use fs::*;
pub use mount::*;
pub use node::*;
pub use types::*;

/// Type alias for VFS-specific error types based on Linux errno values
pub type VfsError = axerrno::LinuxError;
/// Type alias for VFS operation results
pub type VfsResult<T> = Result<T, VfsError>;
