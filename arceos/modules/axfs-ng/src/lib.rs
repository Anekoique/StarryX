#![no_std]

extern crate alloc;

#[macro_use]
extern crate log;

mod disk;
pub mod fs;
mod highlevel;

pub use highlevel::*;
