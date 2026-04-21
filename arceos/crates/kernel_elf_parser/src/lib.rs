#![cfg_attr(not(test), no_std)]
#![doc = include_str!("../README.md")]

mod auxv;
pub use auxv::*;
mod info;
pub use info::*;
mod user_stack;
pub use user_stack::app_stack_region;
