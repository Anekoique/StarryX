bitflags::bitflags! {
    #[derive(Debug)]
    pub struct SocketLevel: u32 {
        const SOL_SOCKET
    }
}