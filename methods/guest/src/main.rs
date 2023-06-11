#![no_main]
// If you want to try std support, also update the guest Cargo.toml file
//#![no_std]  // std support is experimental

use std::os::raw::c_char;
use std::ffi::{CStr};

extern {
    fn run_uarch() -> u64;
}

use risc0_zkvm::guest::env;

risc0_zkvm::guest::entry!(main);

use project_core::{SYS_PAGE_IN};


static mut pageIns: Vec<&[u8]> = Vec::new();

#[no_mangle]
pub extern "C" fn printout(c_string: *const c_char) {
 let s = unsafe { CStr::from_ptr(c_string).to_string_lossy().into_owned() };
 println!("printout: <{:?}>", s);
}



#[no_mangle]
pub extern "C" fn page_in(paddr: u64) -> u32 {
  unsafe {
   let mem = env::send_recv_slice::<u8, u8>(SYS_PAGE_IN, &u64::to_le_bytes(paddr));
   pageIns.push(mem);
   println!("paged in {} as {}", paddr, mem.as_ptr() as usize);
   mem.as_ptr() as usize as u32
  }
}

#[no_mangle]
pub extern "C" fn page_out(paddr: u64) {
}


#[no_mangle]
pub extern "C" fn _putchar(x: u8) {
 print!("{}", x as char);
}

#[no_mangle]
pub extern "C" fn abort() {
 panic!("aborted");
}


pub fn main() {
   println!("entering uarch");
   unsafe {
     let mcycle = run_uarch();
     println!("new mcycle = {}", mcycle);
   }
   println!("leaving uarch");
}
