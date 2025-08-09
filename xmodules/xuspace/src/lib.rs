//! User space memory access utilities for kernel-level operations.
//! Provides safe abstractions for reading/writing user memory with proper validation.

#![no_std]
extern crate alloc;

mod ptr;
mod uspace;

pub use ptr::*;
pub use uspace::*;
