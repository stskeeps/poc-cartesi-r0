#![no_main]
// If you want to try std support, also update the guest Cargo.toml file
//#![no_std]  // std support is experimental

use std::os::raw::c_char;
use std::ffi::{CStr};

extern {
    fn run_uarch(mcycle_end: u64) -> u64;
}

use risc0_zkvm::guest::env;

risc0_zkvm::guest::entry!(main);

use project_core::{SYS_PAGE_IN};

struct PageIn<'a> {
  page: &'a [u8],
  paddr: u64,
  length: u64
}

static mut pageIns: Vec<PageIn> = Vec::new();

#[no_mangle]
pub extern "C" fn printout(c_string: *const c_char) {
 let s = unsafe { CStr::from_ptr(c_string).to_string_lossy().into_owned() };
 println!("printout: <{:?}>", s);
}



#[no_mangle]
pub extern "C" fn page_in(paddr: u64) -> u32 {
  page_in_with_length(paddr, 4096)
}

#[no_mangle]
pub extern "C" fn page_in_with_length(paddr: u64, length: u64) -> u32 {
  unsafe {
   let cache = pageIns.iter().find(|&x| x.paddr == paddr && x.length == length);
   match cache {
    None => {
      let mut v: Vec<u8> = Vec::new();

      v.extend_from_slice(&paddr.to_le_bytes());
      v.extend_from_slice(&length.to_le_bytes());
      let mem = env::send_recv_slice::<u8, u8>(SYS_PAGE_IN, v.as_slice());   
      println!("paged in paddr 0x{:x} as 0x{:x} length 0x{:x}", paddr, mem.as_ptr() as usize, length);

      let s = PageIn {
        page: mem,
        paddr: paddr,
        length: length
      };
      pageIns.push(s);
      mem.as_ptr() as usize as u32
    }
    Some(x) => {
      println!("reused paged in paddr 0x{:x} as 0x{:x} length 0x{:x}", paddr, x.page.as_ptr() as usize, length);
      x.page.as_ptr() as usize as u32
    }
   }
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
     let mcycle = run_uarch(3000);
     println!("new mcycle = {}", mcycle);
   }
   println!("leaving uarch");
}
