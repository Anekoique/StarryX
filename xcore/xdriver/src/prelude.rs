//! Device driver prelude that includes some traits and types.

pub use xdriver_base::{BaseDriverOps, DevError, DevResult, DeviceType};

#[cfg(feature = "block")]
pub use {crate::structs::XBlockDevice, xdriver_block::BlockDriverOps};
#[cfg(feature = "display")]
pub use {crate::structs::XDisplayDevice, xdriver_display::DisplayDriverOps};
#[cfg(feature = "net")]
pub use {crate::structs::XNetDevice, xdriver_net::NetDriverOps};
