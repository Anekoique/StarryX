//! [ArceOS](https://github.com/rcore-os/arceos) network module.
//!
//! It provides unified networking primitives for TCP/UDP communication
//! using various underlying network stacks. Currently, only [smoltcp] is
//! supported.
//!
//! # Organization
//!
//! - [`TcpSocket`]: A TCP socket that provides POSIX-like APIs.
//! - [`UdpSocket`]: A UDP socket that provides POSIX-like APIs.
//! - [`dns_query`]: Function for DNS query.
//!
//! # Cargo Features
//!
//! - `smoltcp`: Use [smoltcp] as the underlying network stack. This is enabled
//!   by default.
//!
//! [smoltcp]: https://github.com/smoltcp-rs/smoltcp

#![no_std]

#[macro_use]
extern crate log;
extern crate alloc;

cfg_if::cfg_if! {
    if #[cfg(feature = "smoltcp")] {
        mod smoltcp_impl;
        use smoltcp_impl as net_impl;
    }
}

pub use self::net_impl::TcpSocket;
pub use self::net_impl::UdpSocket;
pub use self::net_impl::{UnixAddr, UnixSocket};
pub use self::net_impl::{add_membership, dns_query, poll_interfaces};
pub use self::net_impl::{bench_receive, bench_transmit};
pub use smoltcp::time::Duration;
pub use smoltcp::wire::{
    IpAddress as IpAddr, IpEndpoint as SocketAddr, Ipv4Address as Ipv4Addr, Ipv6Address as Ipv6Addr,
};

pub type NetError = axerrno::LinuxError;
pub type NetResult<T = ()> = Result<T, NetError>;

use axdriver::{AxDeviceContainer, prelude::*};

/// Initializes the network subsystem by NIC devices.
pub fn init_network(mut net_devs: AxDeviceContainer<AxNetDevice>) {
    info!("Initialize network subsystem...");

    if let Some(dev) = net_devs.take_one() {
        info!("  use NIC 1: {:?}", dev.device_name());
        net_impl::init(dev);
    } else {
        warn!("No NIC device found!");
    }
}

pub(crate) fn net_error_to_axio(_err: NetError) -> axio::Error {
    axio::Error::Io
}
