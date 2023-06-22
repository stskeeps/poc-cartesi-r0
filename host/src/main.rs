use std::sync::{Arc, Mutex};
use std::thread;
use tokio::runtime::Runtime;
use std::sync::mpsc;
use std::cmp::Ordering;

// TODO: Update the name of the method loaded by the prover. E.g., if the method
// is `multiply`, replace `METHOD_NAME_ELF` with `MULTIPLY_ELF` and replace
// `METHOD_NAME_ID` with `MULTIPLY_ID`
use methods::{METHOD_NAME_ELF, METHOD_NAME_ID};
use risc0_zkvm::{
    Executor, ExecutorEnv,
};

use risc0_zkvm::serde::from_slice;
use risc0_zkvm::serde::to_vec;
use futures::FutureExt; // for `.boxed()`
use sha2::{Sha256, Digest};
use project_core::{SYS_PAGE_IN, CartesiInput, CartesiResult};

extern crate grpc_cartesi_machine;

use grpc_cartesi_machine::{GrpcCartesiMachineClient, MachineRuntimeConfig, ConcurrencyConfig};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();

    env_logger::init();

    // Initialize the grpc_machine outside the callback
    let grpc_machine = Arc::new(Mutex::new(GrpcCartesiMachineClient::new(args[1].clone()).await?));
    grpc_machine.lock().unwrap().load_machine("/images/test3", &MachineRuntimeConfig { concurrency: ConcurrencyConfig { update_merkle_tree: 1 }}).await?;
    let grpc_machine_op = Arc::clone(&grpc_machine);
    let mut total_segments = 0;
    let start_at = 7000000;
    let step = 50000;
    grpc_machine_op.lock().unwrap().run(start_at).await?;

    for i in 0..10000 { 
        let begin_mcycle = start_at + (i * step);
        let end_mcycle = start_at + ((i + 1) * step); 
        let input = CartesiInput {
            begin_mcycle: begin_mcycle,
            end_mcycle: end_mcycle
        };
        let grpc_machine_clone = Arc::clone(&grpc_machine_op); // Clone here instead
        if begin_mcycle % 100000 == 0 {
            println!(". {}", begin_mcycle);
        }
        // First, we construct an executor environment

        let env = ExecutorEnv::builder()
            .add_input(&to_vec(&input)?)
            .io_callback(SYS_PAGE_IN, move |buf: &[u8]| -> Vec<u8> { // use move keyword to capture the environment
                let paddr = u64::from_le_bytes(buf[0..8].try_into().expect("incorrect length"));
                let length = u64::from_le_bytes(buf[8..16].try_into().expect("incorrect length"));
                println!("{} got asked to page in 0x{:x} length 0x{:x}", begin_mcycle, paddr, length);          
                let (tx, rx) = mpsc::channel();
    
                let grpc_machine_inner_clone = Arc::clone(&grpc_machine_clone);
                let handle = thread::spawn(move || {
                    let rt = Runtime::new().unwrap(); // Create a new runtime
            
                    let data = rt.block_on(async {
                        let mut grpc_machine = grpc_machine_inner_clone.lock().unwrap();
                        grpc_machine.read_memory(paddr, length).await.unwrap()
                    }); // Use the runtime to run the async function
                    tx.send(data).unwrap();
                });
            
                handle.join().expect("Thread panicked");
            
                let mem = rx.recv().expect("Failed to receive data");
                mem
            })
            .build();
    
        // TODO: add guest input to the executor environment using
        // ExecutorEnvBuilder::add_input().
        // To access this method, you'll need to use the alternate construction
        // ExecutorEnv::builder(), which creates an ExecutorEnvBuilder. When you're
        // done adding input, call ExecutorEnvBuilder::build().
    
        // For example: let env = ExecutorEnv::builder().add_input(&vec).build();
    
        // Next, we make an executor, loading the (renamed) ELF binary.
        let mut exec = Executor::from_elf(env, METHOD_NAME_ELF).unwrap();
    
        // Run the executor to produce a session.
        let session = exec.run().unwrap();
        total_segments = total_segments + session.segments.len();
        println!("session segments {:?} {:?}", session.segments.len(), total_segments);
    //    println!("proving ..");
        // Prove the session to produce a receipt.
    //    let receipt = session.prove()?;
        
        let result: CartesiResult = from_slice(&session.journal)?;
        if end_mcycle != result.end_mcycle {
            panic!("end_mcycle != result.end_mcycle");
        }

        if result.tty.len() > 0 {
            println!("tty: {}", String::from_utf8(result.tty).unwrap());
        }
        grpc_machine_op.lock().unwrap().run(result.end_mcycle).await?;
        
        for page in result.page_results.iter() {
            if !page.dirty {
                continue;
            }
            //println!("checking dirty page {:x}", page.paddr);
            let mem = grpc_machine_op.lock().unwrap().read_memory(page.paddr, page.length).await.unwrap();
            let mem_clone = mem.clone();
            let mut hasher = Sha256::new();
            hasher.update(mem);
            let hash = hasher.finalize();
            if page.dirty && hash.to_vec().cmp(&page.after_hash.to_vec()) != Ordering::Equal {
                panic!("address 0x{:x} does not match SHA256 guest: {:?} host: {:?}", page.paddr, hash, page.after_hash);
            }
        }
    }

    //println!("got result: {:?}", result);
    // TODO: Implement code for transmitting or serializing the receipt for
    // other parties to verify here

    // Optional: Verify receipt to confirm that recipients will also be able to
    // verify your receipt
  //  receipt.verify(METHOD_NAME_ID).unwrap();
    Ok(())
}
