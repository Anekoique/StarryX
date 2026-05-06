#![cfg_attr(not(test), no_std)]
#![doc = include_str!("../README.md")]

mod auxv;
pub use auxv::*;
mod info;
pub use info::*;
mod user_stack;
pub use user_stack::app_stack_region;

/// Number of `AuxvEntry` slots filled by [`ELFParser::auxv_vector`].
///
/// 18 entries: 16 standard + `SYSINFO_EHDR` + `NULL` terminator. Bumped
/// from 17 by the vDSO migration; future additions grow this constant in
/// lockstep with `auxv_vector`'s body.
pub const AUXV_LEN: usize = 18;
