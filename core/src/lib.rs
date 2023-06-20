use risc0_zkvm::{
    declare_syscall
};

use risc0_zkvm::sha::{Impl, Sha256, Digest};
use serde::{Serialize, Deserialize};

declare_syscall!(
    /// RISC0 syscall for providing oracle access to a vector committed to by the host.
    pub SYS_PAGE_IN);

pub struct PageIn<'a> {
    pub page: &'a [u8],
    pub paddr: u64,
    pub length: u64,
    pub initial_hash: &'a [u8],
    pub after_hash: &'a [u8],
    pub dirty: bool
}

#[derive(Serialize, Deserialize, Debug)]
pub struct PageResult {
    pub paddr: u64,
    pub length: u64,
    pub initial_hash: Vec<u8>,
    pub after_hash: Vec<u8>,
    pub dirty: bool
}