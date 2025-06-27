pub use linux_raw_sys::ctypes::*;
pub use linux_raw_sys::general::*;
pub use linux_raw_sys::ioctl::RTC_RD_TIME;
pub use linux_raw_sys::net::{
    __kernel_sa_family_t, AF_INET, AF_INET6, AF_UNIX, IPPROTO_TCP, IPPROTO_UDP, SOCK_DGRAM,
    SOCK_STREAM, in_addr, in6_addr, sockaddr, sockaddr_in, sockaddr_in6, socklen_t,
};
pub use linux_raw_sys::system::{new_utsname, sysinfo};

#[repr(C)]
#[allow(non_camel_case_types, dead_code)]
pub struct rtc_time {
    pub tm_sec: c_int,
    pub tm_min: c_int,
    pub tm_hour: c_int,
    pub tm_mday: c_int,
    pub tm_mon: c_int,
    pub tm_year: c_int,
    pub tm_wday: c_int,
    pub tm_yday: c_int,
    pub tm_isdst: c_int,
}