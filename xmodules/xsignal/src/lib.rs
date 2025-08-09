//! # Axsignal - Signal Management for StarryX
//!
//! This crate provides signal management functionality for the StarryX operating system.
//! It implements UNIX-style signal handling with support for both standard and real-time signals.
//!
//! ## Features
//!
//! - **Signal Types**: Support for all standard UNIX signals (1-31) and real-time signals (32-64)
//! - **Signal Actions**: Configurable signal handlers, default actions, and signal masking
//! - **Pending Signals**: Efficient management of pending signals with proper queuing for real-time signals
//! - **Architecture Support**: Cross-platform signal handling for multiple architectures
//! - **Process/Thread APIs**: High-level APIs for both process-wide and thread-specific signal operations
//!
//! ## Architecture
//!
//! The crate is organized into several modules:
//! - `types`: Core signal types and enumerations
//! - `action`: Signal action definitions and default behaviors
//! - `pending`: Management of pending signals
//! - `api`: High-level APIs for process and thread signal operations
//! - `arch`: Architecture-specific signal handling implementations
//!
//! ## Usage
//!
//! This crate is designed to be used within the StarryX kernel and provides
//! the foundational signal management capabilities that higher-level system
//! calls and process management can build upon.

#![no_std]

#[macro_use]
extern crate log;
extern crate alloc;

/// Signal action definitions and default behaviors
mod action;
/// High-level APIs for signal management
pub mod api;
/// Architecture-specific signal handling
pub mod arch;
/// Management of pending signals
mod pending;
/// Core signal types and data structures
mod types;

pub use action::*;
pub use pending::*;
pub use types::*;
