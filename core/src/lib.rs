use risc0_zkvm::{
    declare_syscall
};

declare_syscall!(
    /// RISC0 syscall for providing oracle access to a vector committed to by the host.
    pub SYS_PAGE_IN);
