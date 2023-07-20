use bincode;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::fs::File;
use std::io::Write;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};
use tokio::runtime::Runtime;

// TODO: Update the name of the method loaded by the prover. E.g., if the method
// is `multiply`, replace `METHOD_NAME_ELF` with `MULTIPLY_ELF` and replace
// `METHOD_NAME_ID` with `MULTIPLY_ID`
use methods::{METHOD_NAME_ELF, METHOD_NAME_ID};
use risc0_zkvm::{ExecutorEnv, Session};
use risc0_zkvm::default_executor_from_elf;


use project_core::{CartesiInput, CartesiResult, SYS_PAGE_IN};
use risc0_zkvm::serde::from_slice;
use risc0_zkvm::serde::to_vec;
use sha2::{Digest, Sha256};

extern crate cartesi_grpc_interfaces;
extern crate grpc_cartesi_machine;
use cartesi_grpc_interfaces::grpc_stubs::cartesi_machine::Csr;

use grpc_cartesi_machine::{ConcurrencyConfig, GrpcCartesiMachineClient, MachineRuntimeConfig};
struct SessionOfCycle {
    session: Session,
    from_mcycle: u64,
    to_mcycle: u64,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();

    let grpc_machine = Arc::new(Mutex::new(
        GrpcCartesiMachineClient::new(args[1].clone()).await?,
    ));

    let grpc_machine_producer = Arc::clone(&grpc_machine);
    let min_cycle: u64 = args[2].parse()?;
    let max_cycle: u64 = args[3].parse()?;
    let step: u64 = args[4].parse()?;
    let bound: usize = args[5].parse()?;
    let (tx, rx) = mpsc::sync_channel(bound);

    let handle = thread::spawn(move || {
        let rt = Runtime::new().unwrap(); // Create a new runtime

        let data = rt.block_on(async {
            grpc_machine
                .lock()
                .unwrap()
                .load_machine(
                    "/images/test3",
                    &MachineRuntimeConfig {
                        concurrency: ConcurrencyConfig {
                            update_merkle_tree: 1,
                        },
                    },
                )
                .await
                .unwrap();

            grpc_machine_producer
                .lock()
                .unwrap()
                .run(min_cycle)
                .await
                .unwrap();

            let mut current_mcycle: u64 = grpc_machine_producer
                .lock()
                .unwrap()
                .read_csr(Csr::Mcycle)
                .await
                .unwrap();

            if current_mcycle != min_cycle {
                panic!("should start at {} but was {}", min_cycle, current_mcycle);
            }
            while current_mcycle < max_cycle {
                let end_mcycle = if current_mcycle + step > max_cycle {
                    max_cycle
                } else {
                    current_mcycle + step
                };

                let input = CartesiInput {
                    begin_mcycle: current_mcycle,
                    end_mcycle: end_mcycle,
                };

                let grpc_machine_clone = Arc::clone(&grpc_machine_producer); // Clone here instead

                let env = ExecutorEnv::builder()
                    .add_input(&to_vec(&input).unwrap())
                    .io_callback(SYS_PAGE_IN, move |buf: &[u8]| -> Vec<u8> {
                        // use move keyword to capture the environment
                        let paddr =
                            u64::from_le_bytes(buf[0..8].try_into().expect("incorrect length"));
                        let length =
                            u64::from_le_bytes(buf[8..16].try_into().expect("incorrect length"));
                        // println!("{} got asked to page in 0x{:x} length 0x{:x}", begin_mcycle, paddr, length);
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
                    .build()
                    .unwrap();
                let mut exec = default_executor_from_elf(env, METHOD_NAME_ELF).unwrap();
                println!("executing mcycle {} to {}", current_mcycle, end_mcycle);

                let start = Instant::now();
                let session = exec.run().unwrap();
                let duration = start.elapsed();
                println!("done executing, elapsed {:?}", duration);
                let result: CartesiResult = from_slice(&session.journal).unwrap();
                let real_end_mcycle = if result.end_mcycle != end_mcycle {
                    result.end_mcycle
                } else {
                    end_mcycle
                };

                if result.tty.len() > 0 {
                    println!("tty: {}", String::from_utf8(result.tty).unwrap());
                }

                let ses = SessionOfCycle {
                    session: session,
                    from_mcycle: current_mcycle,
                    to_mcycle: real_end_mcycle,
                };
                tx.send(ses).unwrap();

                grpc_machine_producer
                    .lock()
                    .unwrap()
                    .run(real_end_mcycle)
                    .await
                    .unwrap();

                current_mcycle = grpc_machine_producer
                    .lock()
                    .unwrap()
                    .read_csr(Csr::Mcycle)
                    .await
                    .unwrap();
            }
        });
    });

    for received in rx {
        println!(
            "Proving session {}-{} {}",
            received.from_mcycle, received.to_mcycle,
            received.session.segments.len()
        );
        let start = Instant::now();
        let receipt = received.session.prove().unwrap();
        let duration = start.elapsed();
        println!("done proving, elapsed {:?}", duration);

        println!("Writing out proof");
        let filename = format!(
            "proofs/proofs_{}_{}.bin",
            received.from_mcycle, received.to_mcycle
        );
        let encoded: Vec<u8> = bincode::serialize(&receipt).unwrap();;
        let mut file = File::create(&filename).unwrap();
        file.write_all(&encoded).unwrap();
        println!("done writing proof, size was {}", encoded.len());
    }

    Ok(())
}
