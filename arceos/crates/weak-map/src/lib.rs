//! [`WeakMap`] is a wrapper over `BTreeMap` that stores weak references to
//! values.

#![no_std]
#![warn(missing_docs)]

extern crate alloc;

pub mod map;
pub use map::{StrongMap, WeakMap};

mod traits;
pub use traits::{StrongRef, WeakRef};
