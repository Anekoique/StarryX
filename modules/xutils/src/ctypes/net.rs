use super::{IPPROTO_IP, IPPROTO_TCP, IPPROTO_UDP, SOL_SOCKET};

bitflags::bitflags! {
    #[derive(Debug)]
    pub struct SocketLevel: u32 {
        const L_SOCKET = SOL_SOCKET;
        const L_TP = IPPROTO_IP;
        const L_TCP = IPPROTO_TCP;
        const L_UDP = IPPROTO_UDP;
    }
}