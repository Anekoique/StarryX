//! StarryX filesystem service component.
//!
//! The crate name `xfs` follows the XCore `x*` naming convention. It is not an
//! implementation of the Linux XFS disk filesystem.

#![no_std]

extern crate alloc;

mod disk;
pub mod fs;
mod highlevel;

pub use highlevel::*;
