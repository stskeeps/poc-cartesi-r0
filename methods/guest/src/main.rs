// If you want to try std support, also update the guest Cargo.toml file
//#![no_std]  // std support is experimental

use std::os::raw::c_char;
use std::ffi::{CStr};
use risc0_zkvm::guest::env;
use risc0_zkvm::sha::{Impl, Sha256, Digest};
use project_core::{SYS_PAGE_IN, PageIn, PageResult, CartesiInput, CartesiResult};

static mut pageIns: Vec<PageIn> = Vec::new();
static mut ttyOut: Vec<u8> = Vec::new();
static mut timeSpentHashing: usize = 0;
static mut timeSpentSendRecv: usize = 0;

extern {
    fn run_uarch(mcycle_begin: u64, mcycle_end: u64) -> u64;
}

risc0_zkvm::guest::entry!(main);

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
pub extern "C" fn print_counter(c_string: *const c_char, cycle: u32) {
  let s = unsafe { CStr::from_ptr(c_string).to_string_lossy().into_owned() };
  println!("<{:?}> count: {}", s, cycle);
}

#[no_mangle]
pub extern "C" fn ucycle() -> u32 {
  env::get_cycle_count() as u32
}

#[no_mangle]
pub extern "C" fn page_dirty(paddr: u64) {
  unsafe {
    let cache = pageIns.iter_mut().find(|x| x.paddr == paddr);
    match cache {
      Some(elm) => {
        //println!("marked paddr 0x{:x} dirty", paddr);
        elm.dirty = true;
      },
      None => {
        panic!("asked to dirty page 0{:x} i don't have", paddr);
      } 
    }  
  } 
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
      //println!("paged in paddr 0x{:x} as 0x{:x} length 0x{:x}", paddr, mem.as_ptr() as usize, length);
      let before_hash = env::get_cycle_count();
      let digest = Box::new(Impl::hash_bytes(mem));
      let initial_hash = Box::leak(digest).as_bytes();
      let after_hash = env::get_cycle_count();
      let s = PageIn {
        page: mem,
        paddr: paddr,
        length: length,
        initial_hash: initial_hash,
        dirty: false,
      };
      pageIns.push(s);
      mem.as_ptr() as usize as u32
    }
    Some(x) => {
      //println!("reused paged in paddr 0x{:x} as 0x{:x} length 0x{:x}", paddr, x.page.as_ptr() as usize, length);
      x.page.as_ptr() as usize as u32
    }
   }
  }
}

#[no_mangle]
pub extern "C" fn _putchar(x: u8) {
 unsafe { 
  ttyOut.push(x);
 }
}

#[no_mangle]
pub extern "C" fn abort() {
 panic!("aborted");
}


pub fn main() {
   unsafe {
    let input: CartesiInput = env::read();
    //let before = env::get_cycle_count();
    let mcycle = run_uarch(input.begin_mcycle, input.end_mcycle);
    //let after = env::get_cycle_count();

    //println!("cycles spent in uarch: {}", after-before);

    let mut commitment: Vec<PageResult> = Vec::new();
    //let before_dirty = env::get_cycle_count();
    for n in pageIns.iter() {
      if n.dirty {
        commitment.push(PageResult {
          paddr: n.paddr,
          length: n.length,
          initial_hash: n.initial_hash.to_vec(),
          after_hash: Impl::hash_bytes(n.page).as_bytes().to_vec(),
          dirty: true
        });
      } else {
        commitment.push(PageResult {
          paddr: n.paddr,
          length: n.length,
          initial_hash: n.initial_hash.to_vec(),
          after_hash: Vec::new(),
          dirty: false
        });
      }
    }
    let result = CartesiResult {
      begin_mcycle: input.begin_mcycle,
      end_mcycle: input.end_mcycle,
      page_results: commitment,
      tty: ttyOut.clone(),
    };

    env::commit(&result);
    // let after_dirty = env::get_cycle_count();
    /* unsafe {
      println!("total cycles spent hashing: {} sendrecv: {} dirty-hashing-commit: {}", timeSpentHashing, timeSpentSendRecv, after_dirty - before_dirty);
    } */
   }
}
