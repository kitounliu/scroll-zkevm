//! This module implements outer circuit related APIs for Prover.

use super::{AggCircuitProof, Prover};
use crate::circuit::{SuperCircuit, TargetCircuit};
use crate::io::{serialize_fr_tensor, serialize_vk};
use crate::prover::TargetCircuitProof;
use rand::{Rng, SeedableRng};
use rand_xorshift::XorShiftRng;
use snark_verifier_sdk::evm::gen_evm_proof_shplonk;
use snark_verifier_sdk::halo2::aggregation::AggregationCircuit;
use snark_verifier_sdk::CircuitExt;
use types::eth::BlockTrace;

impl Prover {
    /// Load a proved aggregation circuit instance from the disc.
    /// Used for Debugging only.
    pub fn load_aggregation_circuit_instance<C: TargetCircuit>(
        &self,
    ) -> anyhow::Result<TargetCircuitProof> {
        assert!(!self.debug_dir.is_empty());
        log::debug!("load aggregation circuit instance: {}", C::name());
        let file_name = format!("{}/{}_proof.json", self.debug_dir, C::name());
        let file = std::fs::File::open(file_name)?;
        let proof: TargetCircuitProof = serde_json::from_reader(file)?;
        Ok(proof)
    }

    /// Input a block trace, generate a proof for the aggregation circuit.
    /// This proof is verifiable by the evm.
    pub fn create_agg_circuit_proof(
        &mut self,
        block_trace: &BlockTrace,
        rng: &mut (impl Rng + Send),
    ) -> anyhow::Result<AggCircuitProof> {
        self.create_agg_circuit_proof_batch(&[block_trace.clone()], rng)
    }

    /// Input a list of block traces, generate a proof for the aggregation circuit.
    /// This proof is verifiable by the evm.
    pub fn create_agg_circuit_proof_batch(
        &mut self,
        block_traces: &[BlockTrace],
        rng: &mut (impl Rng + Send),
    ) -> anyhow::Result<AggCircuitProof> {
        let circuit_results: Vec<TargetCircuitProof> =
            vec![self.prove_inner_circuit::<SuperCircuit>(block_traces, rng)?];
        self.create_agg_circuit_proof_impl(circuit_results.as_ref(), rng)
    }

    /// Input an instance of the aggregation circuit, output its proof.
    ///
    /// The actual work for the outer circuit prover.
    ///
    pub fn create_agg_circuit_proof_impl(
        &mut self,
        inner_circuit_results: &[TargetCircuitProof],
        rng: &mut (impl Rng + Send),
    ) -> anyhow::Result<AggCircuitProof> {
        let mut seed1 = [0u8; 16];
        rng.fill_bytes(&mut seed1);
        let mut seed2 = [0u8; 16];
        rng.fill_bytes(&mut seed2);
        let rng1 = XorShiftRng::from_seed(seed1);
        let mut rng2 = XorShiftRng::from_seed(seed2);

        // build the aggregation circuit inputs from the inner circuit outputs
        let agg_circuit = AggregationCircuit::new(
            &self.agg_params,
            inner_circuit_results.iter().map(|p| p.snark.clone()),
            rng1,
        );
        let pk = match self.agg_pk.clone() {
            Some(pk) => pk,
            None => panic!("aggregation proving key is not found"),
        };

        let agg_proof = gen_evm_proof_shplonk(
            &self.agg_params,
            &pk,
            agg_circuit.clone(),
            agg_circuit.instances(),
            &mut rng2,
        );

        // total number of blocks proved
        let total_proved_block_count = inner_circuit_results
            .iter()
            .map(|x| x.num_of_proved_blocks)
            .sum();
        let total_block_count: usize = inner_circuit_results
            .iter()
            .map(|x| x.total_num_of_blocks)
            .sum();
        // serialize instances
        let instances_for_serde = serialize_fr_tensor(&[agg_circuit.instances()]);
        let instance_bytes = serde_json::to_vec(&instances_for_serde)?;
        // serialize vk
        let vk_bytes = serialize_vk(pk.get_vk());

        log::info!(
            "create agg proof done, block proved {}/{}",
            total_proved_block_count,
            total_block_count
        );
        Ok(AggCircuitProof {
            proof: agg_proof,
            instance: instance_bytes,
            vk: vk_bytes,
            total_proved_block_count,
        })
    }
}
