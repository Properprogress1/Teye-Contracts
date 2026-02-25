use soroban_sdk::{contracttype, Address, BytesN, Env};

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuditRecord {
    pub user: Address,
    pub resource_id: BytesN<32>,
    pub proof_hash: BytesN<32>,
    pub timestamp: u64,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerificationRecord {
    pub submitter: Address,
    pub proof_id: u64,
    pub verified: bool,
    pub timestamp: u64,
}

pub struct AuditTrail;

impl AuditTrail {
    pub fn log_access(env: &Env, user: Address, resource_id: BytesN<32>, proof_hash: BytesN<32>) {
        let record = AuditRecord {
            user: user.clone(),
            resource_id: resource_id.clone(),
            proof_hash,
            timestamp: env.ledger().timestamp(),
        };
        env.storage()
            .persistent()
            .set(&(&user, &resource_id), &record);
        env.events().publish((user, resource_id), record);
    }

    pub fn get_record(env: &Env, user: Address, resource_id: BytesN<32>) -> Option<AuditRecord> {
        env.storage().persistent().get(&(&user, &resource_id))
    }

    pub fn log_verification(env: &Env, submitter: &Address, proof_id: u64, verified: bool) {
        let record = VerificationRecord {
            submitter: submitter.clone(),
            proof_id,
            verified,
            timestamp: env.ledger().timestamp(),
        };
        env.storage()
            .persistent()
            .set(&("verification", proof_id), &record);
        env.events().publish(("verification", proof_id), (submitter, verified));
    }
}
