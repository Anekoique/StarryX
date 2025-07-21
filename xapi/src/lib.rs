#![no_std]
#![allow(missing_docs)]
#![allow(clippy::unit_arg)]
#![feature(let_chains)]

#[macro_use]
extern crate axlog;
extern crate alloc;

pub mod backend;
mod syscall;
pub mod utils;

pub use {backend::*, syscall::*, utils::*};
