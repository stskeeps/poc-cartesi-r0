use std::sync::{Arc, Mutex};
use std::thread;
use tokio::runtime::Runtime;
use std::sync::mpsc;

// TODO: Update the name of the method loaded by the prover. E.g., if the method
// is `multiply`, replace `METHOD_NAME_ELF` with `MULTIPLY_ELF` and replace
// `METHOD_NAME_ID` with `MULTIPLY_ID`
use methods::{METHOD_NAME_ELF, METHOD_NAME_ID};
use risc0_zkvm::{
    serde::{from_slice, to_vec},
    Executor, ExecutorEnv,
};
use futures::FutureExt; // for `.boxed()`

use project_core::{SYS_PAGE_IN};

extern crate grpc_cartesi_machine;

use grpc_cartesi_machine::{GrpcCartesiMachineClient, MachineRuntimeConfig, ConcurrencyConfig};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();

    env_logger::init();

    // Initialize the grpc_machine outside the callback
    let grpc_machine = Arc::new(Mutex::new(GrpcCartesiMachineClient::new(args[1].clone()).await?));
    grpc_machine.lock().unwrap().load_machine("/images/test", &MachineRuntimeConfig { concurrency: ConcurrencyConfig { update_merkle_tree: 1 }}).await?;

    // First, we construct an executor environment
    let env = ExecutorEnv::builder()
        .io_callback(SYS_PAGE_IN, move |buf: &[u8]| -> Vec<u8> { // use move keyword to capture the environment
            let paddr = u64::from_le_bytes(buf.try_into().expect("incorrect length"));
            println!("got asked to page in {}", paddr);          
            let (tx, rx) = mpsc::channel();

            let grpc_machine_clone = Arc::clone(&grpc_machine);

            let handle = thread::spawn(move || {
                let rt = Runtime::new().unwrap(); // Create a new runtime
        
                let data = rt.block_on(async {
                    let mut grpc_machine = grpc_machine_clone.lock().unwrap();
                    grpc_machine.read_memory(paddr, 4096).await.unwrap()
                }); // Use the runtime to run the async function
                tx.send(data).unwrap();
            });
        
            handle.join().expect("Thread panicked");
        
            rx.recv().expect("Failed to receive data")
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

    println!("session segments {:?}", session.segments.len());
    println!("proving ..");
    // Prove the session to produce a receipt.
    let receipt = session.prove().unwrap();

    println!("made receipt");
    // TODO: Implement code for transmitting or serializing the receipt for
    // other parties to verify here

    // Optional: Verify receipt to confirm that recipients will also be able to
    // verify your receipt
    receipt.verify(METHOD_NAME_ID).unwrap();
    Ok(())
}
