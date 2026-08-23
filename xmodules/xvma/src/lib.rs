//! Safe user virtual-memory policy and address-space ownership.
//!
//! [`VmSpace`] is the sole VMA/layout owner. The crate keeps its public
//! surface deliberately small; backing details and fault policy
//! remain internal implementation modules.

#![no_std]
#![forbid(unsafe_code)]

extern crate alloc;

mod area;
mod backend;
mod fault;
mod object;
mod space;

pub use backend::Backend;
pub use fault::FaultResolution;
pub use object::{SharedObject, VmObject, VmPage, VmPageGuard, allocate_object_id};
pub use space::VmSpace;
