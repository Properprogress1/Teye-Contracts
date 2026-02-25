#![no_std]

pub mod verifier;
pub mod audit;
pub mod helpers;

use verifier::{Proof, Bn254Verifier, PoseidonHasher};
use soroban_sdk::{contract, contractimpl, contracttype, symbol_short, Address, Env, String, Vec, BytesN};

/// Verification result storage
#[contracttype]
#[derive(Clone, Debug)]
pub struct VerificationResult {
    pub proof_id: u64,
    pub submitter: Address,
    public_inputs: Vec<BytesN<32>>,
    verified: bool,
    timestamp: u64,
}

/// Preparation data for verification
#[contracttype]
#[derive(Clone, Debug)]
pub struct PrepareVerification {
    pub proof_id: u64,
    pub submitter: Address,
    pub proof: Proof,
    pub public_inputs: Vec<BytesN<32>>,
    pub timestamp: u64,
}

/// Storage keys
const ADMIN: Symbol = symbol_short!("ADMIN");
const INITIALIZED: Symbol = symbol_short!("INIT");
const PROOF_COUNTER: Symbol = symbol_short!("PROOF_CTR");
const VERIFICATION_RESULTS: Symbol = symbol_short!("VERIFY_RES");

#[contract]
pub struct ZkVerifierContract;

#[contractimpl]
impl ZkVerifierContract {
    /// Initialize the zk verifier contract
    pub fn initialize(env: Env, admin: Address) -> Result<(), ContractError> {
        if env.storage().instance().has(&INITIALIZED) {
            return Err(ContractError::AlreadyInitialized);
        }

        env.storage().instance().set(&ADMIN, &admin);
        env.storage().instance().set(&INITIALIZED, &true);
        env.storage().instance().set(&PROOF_COUNTER, &0u64);

        Ok(())
    }

    /// Verify a zero-knowledge proof
    pub fn verify_proof(
        env: Env,
        submitter: Address,
        proof: Proof,
        public_inputs: Vec<BytesN<32>>,
    ) -> Result<u64, ContractError> {
        Self::require_initialized(&env)?;
        submitter.require_auth();

        // Generate proof ID
        let proof_id: u64 = env
            .storage()
            .instance()
            .get(&PROOF_COUNTER)
            .unwrap_or(0u64)
            .saturating_add(1u64);
        env.storage().instance().set(&PROOF_COUNTER, &proof_id);

        // Verify the proof
        let verified = Bn254Verifier::verify_proof(&env, &proof, &public_inputs);

        // Store verification result
        let result = VerificationResult {
            proof_id,
            submitter: submitter.clone(),
            public_inputs: public_inputs.clone(),
            verified,
            timestamp: env.ledger().timestamp(),
        };

        let key = (VERIFICATION_RESULTS, proof_id);
        env.storage().persistent().set(&key, &result);

        // Log the verification
        audit::log_verification(&env, &submitter, proof_id, verified);

        Ok(proof_id)
    }

    /// Batch verify multiple proofs
    pub fn batch_verify_proofs(
        env: Env,
        submitter: Address,
        proofs: Vec<Proof>,
        public_inputs_batch: Vec<Vec<BytesN<32>>>,
    ) -> Result<Vec<u64>, ContractError> {
        Self::require_initialized(&env)?;
        submitter.require_auth();

        if proofs.len() != public_inputs_batch.len() {
            return Err(ContractError::InvalidInput);
        }

        let mut proof_ids = Vec::new(&env);
        
        for i in 0..proofs.len() {
            let proof = proofs.get(i).unwrap().clone();
            let public_inputs = public_inputs_batch.get(i).unwrap().clone();
            
            let proof_id = Self::verify_proof(env.clone(), submitter.clone(), proof, public_inputs)?;
            proof_ids.push_back(proof_id);
        }

        Ok(proof_ids)
    }

    /// Get verification result
    pub fn get_verification_result(env: Env, proof_id: u64) -> Result<VerificationResult, ContractError> {
        let key = (VERIFICATION_RESULTS, proof_id);
        env.storage()
            .persistent()
            .get(&key)
            .ok_or(ContractError::VerificationNotFound)
    }

    /// Check if a proof was verified
    pub fn is_verified(env: Env, proof_id: u64) -> bool {
        let key = (VERIFICATION_RESULTS, proof_id);
        if let Some(result) = env.storage().persistent().get::<VerificationResult>(&key) {
            result.verified
        } else {
            false
        }
    }

    /// Hash data using Poseidon
    pub fn hash_data(env: Env, inputs: Vec<BytesN<32>>) -> BytesN<32> {
        PoseidonHasher::hash(&env, &inputs)
    }

    /// Get admin address
    pub fn get_admin(env: Env) -> Result<Address, ContractError> {
        env.storage()
            .instance()
            .get(&ADMIN)
            .ok_or(ContractError::NotInitialized)
    }

    /// Check if contract is initialized
    pub fn is_initialized(env: Env) -> bool {
        env.storage().instance().has(&INITIALIZED)
    }

    // ======================== Two-Phase Commit Hooks ========================

    /// Prepare phase for proof verification
    pub fn prepare_verify_proof(
        env: Env,
        submitter: Address,
        proof: Proof,
        public_inputs: Vec<BytesN<32>>,
    ) -> Result<u64, ContractError> {
        Self::require_initialized(&env)?;

        // Validate inputs without making state changes
        if public_inputs.is_empty() {
            return Err(ContractError::InvalidInput);
        }

        // Generate proof ID
        let proof_id: u64 = env
            .storage()
            .instance()
            .get(&PROOF_COUNTER)
            .unwrap_or(0u64)
            .saturating_add(1u64);

        // Store preparation data
        let prep_key = (symbol_short!("PREP_VERIFY"), proof_id);
        let prep_data = PrepareVerification {
            proof_id,
            submitter: submitter.clone(),
            proof: proof.clone(),
            public_inputs: public_inputs.clone(),
            timestamp: env.ledger().timestamp(),
        };
        env.storage().temporary().set(&prep_key, &prep_data);

        Ok(proof_id)
    }

    /// Commit phase for proof verification
    pub fn commit_verify_proof(
        env: Env,
        proof_id: u64,
    ) -> Result<(), ContractError> {
        Self::require_initialized(&env)?;

        // Retrieve preparation data
        let prep_key = (symbol_short!("PREP_VERIFY"), proof_id);
        let prep_data: PrepareVerification = env.storage().temporary()
            .get(&prep_key)
            .ok_or(ContractError::InvalidInput)?;

        // Update the counter
        env.storage().instance().set(&PROOF_COUNTER, &proof_id);

        // Verify the proof
        let verified = Bn254Verifier::verify_proof(&env, &prep_data.proof, &prep_data.public_inputs);

        // Store verification result
        let result = VerificationResult {
            proof_id,
            submitter: prep_data.submitter.clone(),
            public_inputs: prep_data.public_inputs.clone(),
            verified,
            timestamp: prep_data.timestamp,
        };

        let key = (VERIFICATION_RESULTS, proof_id);
        env.storage().persistent().set(&key, &result);

        // Log the verification
        audit::log_verification(&env, &prep_data.submitter, proof_id, verified);

        // Clean up preparation data
        env.storage().temporary().remove(&prep_key);

        Ok(())
    }

    /// Rollback phase for proof verification
    pub fn rollback_verify_proof(
        env: Env,
        proof_id: u64,
    ) -> Result<(), ContractError> {
        // Clean up preparation data
        let prep_key = (symbol_short!("PREP_VERIFY"), proof_id);
        env.storage().temporary().remove(&prep_key);

        Ok(())
    }

    /// Prepare phase for batch verification
    pub fn prepare_batch_verify_proofs(
        env: Env,
        submitter: Address,
        proofs: Vec<Proof>,
        public_inputs_batch: Vec<Vec<BytesN<32>>>,
    ) -> Result<Vec<u64>, ContractError> {
        Self::require_initialized(&env)?;

        if proofs.len() != public_inputs_batch.len() {
            return Err(ContractError::InvalidInput);
        }

        let mut proof_ids = Vec::new(&env);
        let mut start_proof_id: u64 = env
            .storage()
            .instance()
            .get(&PROOF_COUNTER)
            .unwrap_or(0u64);

        // Validate all inputs and generate IDs
        for i in 0..proofs.len() {
            let public_inputs = public_inputs_batch.get(i).unwrap().clone();
            
            if public_inputs.is_empty() {
                return Err(ContractError::InvalidInput);
            }

            start_proof_id = start_proof_id.saturating_add(1);
            proof_ids.push_back(start_proof_id);

            // Store preparation data for each proof
            let prep_key = (symbol_short!("PREP_BATCH_VERIFY"), start_proof_id);
            let prep_data = PrepareVerification {
                proof_id: start_proof_id,
                submitter: submitter.clone(),
                proof: proofs.get(i).unwrap().clone(),
                public_inputs,
                timestamp: env.ledger().timestamp(),
            };
            env.storage().temporary().set(&prep_key, &prep_data);
        }

        Ok(proof_ids)
    }

    /// Commit phase for batch verification
    pub fn commit_batch_verify_proofs(
        env: Env,
        proof_ids: Vec<u64>,
    ) -> Result<(), ContractError> {
        Self::require_initialized(&env)?;

        let mut max_proof_id = 0u64;

        // Commit each verification
        for proof_id in proof_ids {
            max_proof_id = max_proof_id.max(proof_id);
            
            // Retrieve preparation data
            let prep_key = (symbol_short!("PREP_BATCH_VERIFY"), proof_id);
            let prep_data: PrepareVerification = env.storage().temporary()
                .get(&prep_key)
                .ok_or(ContractError::InvalidInput)?;

            // Verify the proof
            let verified = Bn254Verifier::verify_proof(&env, &prep_data.proof, &prep_data.public_inputs);

            // Store verification result
            let result = VerificationResult {
                proof_id,
                submitter: prep_data.submitter.clone(),
                public_inputs: prep_data.public_inputs.clone(),
                verified,
                timestamp: prep_data.timestamp,
            };

            let key = (VERIFICATION_RESULTS, proof_id);
            env.storage().persistent().set(&key, &result);

            // Log the verification
            audit::log_verification(&env, &prep_data.submitter, proof_id, verified);

            // Clean up preparation data
            env.storage().temporary().remove(&prep_key);
        }

        // Update the counter to the highest proof ID
        env.storage().instance().set(&PROOF_COUNTER, &max_proof_id);

        Ok(())
    }

    /// Rollback phase for batch verification
    pub fn rollback_batch_verify_proofs(
        env: Env,
        proof_ids: Vec<u64>,
    ) -> Result<(), ContractError> {
        // Clean up preparation data for all proofs
        for proof_id in proof_ids {
            let prep_key = (symbol_short!("PREP_BATCH_VERIFY"), proof_id);
            env.storage().temporary().remove(&prep_key);
        }

        Ok(())
    }

    // Helper functions
    fn require_initialized(env: &Env) -> Result<(), ContractError> {
        if !env.storage().instance().has(&INITIALIZED) {
            Err(ContractError::NotInitialized)
        } else {
            Ok(())
        }
    }
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ContractError {
    AlreadyInitialized,
    NotInitialized,
    InvalidInput,
    VerificationNotFound,
    Unauthorized,
}

impl From<ContractError> for u32 {
    fn from(error: ContractError) -> u32 {
        match error {
            ContractError::AlreadyInitialized => 1001,
            ContractError::NotInitialized => 1002,
            ContractError::InvalidInput => 1003,
            ContractError::VerificationNotFound => 1004,
            ContractError::Unauthorized => 1005,
        }
    }
}