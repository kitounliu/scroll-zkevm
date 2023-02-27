use clap::Parser;
use log::info;
use rand::SeedableRng;
use rand_xorshift::XorShiftRng;
use std::fs;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use std::time::Instant;
use zkevm::{
    circuit::{EvmCircuit, StateCircuit, AGG_DEGREE, DEGREE},
    prover::Prover,
    utils::{get_block_trace_from_file, load_or_create_params, load_or_create_seed},
};

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    /// Get params and write into file.
    #[clap(short, long = "params")]
    params_path: Option<String>,
    /// Get seed and write into file.
    #[clap(long = "seed")]
    seed_path: Option<String>,
    /// Get BlockTrace from file or dir.
    #[clap(short, long = "trace")]
    trace_path: Option<String>,
    /// Option means if generates evm proof.
    /// Boolean means if output evm proof.
    #[clap(long = "evm")]
    evm_proof: Option<bool>,
    /// Option means if generates state proof.
    /// Boolean means if output state proof.
    #[clap(long = "state")]
    state_proof: Option<bool>,
    /// Option means if generates agg proof.
    /// Boolean means if output agg proof.
    #[clap(long = "agg")]
    agg_proof: Option<bool>,
}

fn main() {
    dotenv::dotenv().ok();
    env_logger::init();

    let args = Args::parse();
    let params = load_or_create_params(&args.params_path.clone().unwrap(), *DEGREE)
        .expect("failed to load or create params");
    let agg_params = load_or_create_params(&args.params_path.unwrap(), *AGG_DEGREE)
        .expect("failed to load or create params");
    let seed =
        load_or_create_seed(&args.seed_path.unwrap()).expect("failed to load or create seed");
    let rng = XorShiftRng::from_seed(seed);

    let mut prover = Prover::from_params_and_rng(params, agg_params, rng);

    let mut traces = Vec::new();
    let trace_path = PathBuf::from(&args.trace_path.unwrap());
    if trace_path.is_dir() {
        for entry in fs::read_dir(&trace_path).unwrap() {
            let path = entry.unwrap().path();
            if path.is_file() && path.to_str().unwrap().ends_with(".json") {
                let block_trace = get_block_trace_from_file(path.to_str().unwrap());
                traces.push(block_trace);
            }
        }
    } else {
        let block_trace = get_block_trace_from_file(trace_path.to_str().unwrap());
        traces.push(block_trace);
    }

    let outer_now = Instant::now();
    if args.evm_proof.is_some() {
        let proof_path = trace_path.join("evm.proof");

        let now = Instant::now();
        let evm_proof = prover
            .create_target_circuit_proof_batch::<EvmCircuit>(&traces)
            .expect("cannot generate evm_proof");
        info!(
            "finish generating evm proof of {}, elapsed: {:?}",
            trace_path.to_str().unwrap(),
            now.elapsed()
        );

        if args.evm_proof.unwrap() {
            let mut f = File::create(&proof_path).unwrap();
            f.write_all(evm_proof.proof.as_slice()).unwrap();
        }
    }

    if args.state_proof.is_some() {
        let proof_path = trace_path.join("state.proof");

        let now = Instant::now();
        let state_proof = prover
            .create_target_circuit_proof_batch::<StateCircuit>(&traces)
            .expect("cannot generate state_proof");
        info!(
            "finish generating state proof of {}, elapsed: {:?}",
            trace_path.to_str().unwrap(),
            now.elapsed()
        );

        if args.state_proof.unwrap() {
            let mut f = File::create(&proof_path).unwrap();
            f.write_all(state_proof.proof.as_slice()).unwrap();
        }
    }

    if args.agg_proof.is_some() {
        let mut proof_path = trace_path.join("agg.proof");

        let now = Instant::now();
        let agg_proof = prover
            .create_agg_circuit_proof_batch(&traces)
            .expect("cannot generate agg_proof");
        info!(
            "finish generating agg proof of {}, elapsed: {:?}",
            trace_path.to_str().unwrap(),
            now.elapsed()
        );

        if args.agg_proof.unwrap() {
            fs::create_dir_all(&proof_path).unwrap();
            agg_proof.write_to_dir(&mut proof_path);
        }
    }

    info!("finish generating all, elapsed: {:?}", outer_now.elapsed());
}
