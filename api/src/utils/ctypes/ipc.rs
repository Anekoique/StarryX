use bitflags::bitflags;

bitflags! {
    /// Shared memory get flags
    pub struct ShmGetFlags: u32 {
        /// Read permission
        const SHM_R = 0o400;
        /// Write permission
        const SHM_W = 0o200;
    }
    /// Shared memory attach flags
    pub struct ShmAtFlags: u32 {
        /// Attach for read-only access
        const SHM_RDONLY = 0o10000;
        /// Round address to SHMLBA boundary
        const SHM_RND = 0o20000;
        /// Remap existing mapping
        const SHM_REMAP = 0o40000;
        /// Allow execution of shared memory
        const SHM_EXEC = 0o100000;
    }
}

bitflags! {
    // MsgGet
    pub struct MsgGetFlags: u32 {
        const MSG_R = 0o400;
        const MSG_W = 0o200;
    }
    // MsgRcv
    pub struct MsgRcvFlags: u32 {
        const IPC_NOWAIT = 0o4000;
        const MSG_EXCEPT = 0o20000;
        const MSG_NOERROR = 0o10000;
    }
    // MsgSnd
    pub struct MsgSndFlags: u32 {
        const IPC_NOWAIT = 0o4000;
    }
}

bitflags! {
    // SemGet
    pub struct SemGetFlags: u32 {
        const SEM_R = 0o400;
        const SEM_A = 0o200;
    }
    // SemOp
    pub struct SemOpFlags: u16 {
        const IPC_NOWAIT = 0o4000;
        const SEM_UNDO = 0o10000;
    }
}
