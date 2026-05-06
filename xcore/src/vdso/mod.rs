//! Kernel-side vDSO management.
//!
//! - [`image`] embeds the per-arch ELF blob and indexes its symbols.
//! - [`data`] owns the shared data page and the timer-tick refresher.
//! - [`install`] maps both into a fresh user address space on `execve`.

mod data;
mod image;
mod install;

pub use install::{VdsoBinding, install};
