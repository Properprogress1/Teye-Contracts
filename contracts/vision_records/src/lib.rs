#![no_std]
use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, symbol_short, Address, Env, String,
    Symbol, Vec,
};

mod prescription;
mod prescription_tests;

pub use crate::prescription::{
    ContactLensData, LensType, OptionalContactLensData, Prescription, PrescriptionData,
use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, symbol_short, Address, Env, String,
    Symbol, Vec,
};
pub mod emergency;
mod test;
pub use emergency::{
    EmergencyAccess, EmergencyAuditEntry, EmergencyCondition, EmergencyStatus, EMRG_CTR,
};

/// Storage keys for the contract
const ADMIN: Symbol = symbol_short!("ADMIN");
const INITIALIZED: Symbol = symbol_short!("INIT");

/// User roles in the vision care system
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Role {
    Patient,
    Optometrist,
    Ophthalmologist,
    Admin,
}

/// Access levels for record sharing
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AccessLevel {
    None,
    Read,
    Write,
    Full,
}

/// Vision record types
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RecordType {
    Examination,
    Prescription,
    Diagnosis,
    Treatment,
    Surgery,
    LabResult,
}

/// User information structure
#[contracttype]
#[derive(Clone, Debug)]
pub struct User {
    pub address: Address,
    pub role: Role,
    pub name: String,
    pub registered_at: u64,
    pub is_active: bool,
}

/// Vision record structure
#[contracttype]
#[derive(Clone, Debug)]
pub struct VisionRecord {
    pub id: u64,
    pub patient: Address,
    pub provider: Address,
    pub record_type: RecordType,
    pub data_hash: String,
    pub created_at: u64,
    pub updated_at: u64,
}

/// Access grant structure
#[contracttype]
#[derive(Clone, Debug)]
pub struct AccessGrant {
    pub patient: Address,
    pub grantee: Address,
    pub level: AccessLevel,
    pub granted_at: u64,
    pub expires_at: u64,
}

/// Contract errors
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
pub enum ContractError {
    NotInitialized = 1,
    AlreadyInitialized = 2,
    Unauthorized = 3,
    UserNotFound = 4,
    RecordNotFound = 5,
    InvalidInput = 6,
    AccessDenied = 7,
    Paused = 8,
}

#[contract]
pub struct VisionRecordsContract;

#[contractimpl]
impl VisionRecordsContract {
    /// Initialize the contract with an admin address
    pub fn initialize(env: Env, admin: Address) -> Result<(), ContractError> {
        if env.storage().instance().has(&INITIALIZED) {
            return Err(ContractError::AlreadyInitialized);
        }

        admin.require_auth();

        env.storage().instance().set(&ADMIN, &admin);
        env.storage().instance().set(&INITIALIZED, &true);

        Ok(())
    }

    /// Get the admin address
    pub fn get_admin(env: Env) -> Result<Address, ContractError> {
        env.storage()
            .instance()
            .get(&ADMIN)
            .ok_or(ContractError::NotInitialized)
    }

    /// Check if the contract is initialized
    pub fn is_initialized(env: Env) -> bool {
        env.storage().instance().has(&INITIALIZED)
    }

    /// Register a new user
    pub fn register_user(
        env: Env,
        user: Address,
        role: Role,
        name: String,
    ) -> Result<(), ContractError> {
        user.require_auth();

        let user_data = User {
            address: user.clone(),
            role,
            name,
            registered_at: env.ledger().timestamp(),
            is_active: true,
        };

        let key = (symbol_short!("USER"), user);
        env.storage().persistent().set(&key, &user_data);

        Ok(())
    }

    /// Get user information
    pub fn get_user(env: Env, user: Address) -> Result<User, ContractError> {
        let key = (symbol_short!("USER"), user);
        env.storage()
            .persistent()
            .get(&key)
            .ok_or(ContractError::UserNotFound)
    }

    /// Add a vision record
    #[allow(clippy::arithmetic_side_effects)]
    pub fn add_record(
        env: Env,
        patient: Address,
        provider: Address,
        record_type: RecordType,
        data_hash: String,
    ) -> Result<u64, ContractError> {
        provider.require_auth();

        // Generate record ID
        let counter_key = symbol_short!("REC_CTR");
        let record_id: u64 = env.storage().instance().get(&counter_key).unwrap_or(0) + 1;
        env.storage().instance().set(&counter_key, &record_id);

        let record = VisionRecord {
            id: record_id,
            patient: patient.clone(),
            provider,
            record_type,
            data_hash,
            created_at: env.ledger().timestamp(),
            updated_at: env.ledger().timestamp(),
        };

        let key = (symbol_short!("RECORD"), record_id);
        env.storage().persistent().set(&key, &record);

        // Add to patient's record list
        let patient_key = (symbol_short!("PAT_REC"), patient);
        let mut patient_records: Vec<u64> = env
            .storage()
            .persistent()
            .get(&patient_key)
            .unwrap_or(Vec::new(&env));
        patient_records.push_back(record_id);
        env.storage()
            .persistent()
            .set(&patient_key, &patient_records);

        Ok(record_id)
    }

    /// Get a vision record by ID
    pub fn get_record(env: Env, record_id: u64) -> Result<VisionRecord, ContractError> {
        let key = (symbol_short!("RECORD"), record_id);
        env.storage()
            .persistent()
            .get(&key)
            .ok_or(ContractError::RecordNotFound)
    }

    /// Get all records for a patient
    pub fn get_patient_records(env: Env, patient: Address) -> Vec<u64> {
        let key = (symbol_short!("PAT_REC"), patient);
        env.storage()
            .persistent()
            .get(&key)
            .unwrap_or(Vec::new(&env))
    }

    /// Grant access to a user
    #[allow(clippy::arithmetic_side_effects)]
    pub fn grant_access(
        env: Env,
        patient: Address,
        grantee: Address,
        level: AccessLevel,
        duration_seconds: u64,
    ) -> Result<(), ContractError> {
        patient.require_auth();

        let grant = AccessGrant {
            patient: patient.clone(),
            grantee: grantee.clone(),
            level,
            granted_at: env.ledger().timestamp(),
            expires_at: env.ledger().timestamp() + duration_seconds,
        };

        let key = (symbol_short!("ACCESS"), patient, grantee);
        env.storage().persistent().set(&key, &grant);

        Ok(())
    }

    /// Check access level
    pub fn check_access(env: Env, patient: Address, grantee: Address) -> AccessLevel {
        let key = (symbol_short!("ACCESS"), patient, grantee);

        if let Some(grant) = env.storage().persistent().get::<_, AccessGrant>(&key) {
            if grant.expires_at > env.ledger().timestamp() {
                return grant.level;
            }
        }

        AccessLevel::None
    }

    /// Revoke access
    pub fn revoke_access(
        env: Env,
        patient: Address,
        grantee: Address,
    ) -> Result<(), ContractError> {
        patient.require_auth();

        let key = (symbol_short!("ACCESS"), patient, grantee);
        env.storage().persistent().remove(&key);

        Ok(())
    }

    /// Get the total number of records
    pub fn get_record_count(env: Env) -> u64 {
        let counter_key = symbol_short!("REC_CTR");
        env.storage().instance().get(&counter_key).unwrap_or(0)
    }

    /// Add a new prescription
    pub fn add_prescription(
        env: Env,
        patient: Address,
        provider: Address,
        lens_type: LensType,
        left_eye: PrescriptionData,
        right_eye: PrescriptionData,
        contact_data: OptionalContactLensData,
        duration_seconds: u64,
        metadata_hash: String,
    ) -> Result<u64, ContractError> {
        provider.require_auth();

        // Check if provider is authorized (role check)
        let provider_data = VisionRecordsContract::get_user(env.clone(), provider.clone())?;
        if provider_data.role != Role::Optometrist && provider_data.role != Role::Ophthalmologist {
            return Err(ContractError::Unauthorized);
        }

        // Generate ID
        let counter_key = symbol_short!("RX_CTR");
        let rx_id: u64 = env.storage().instance().get(&counter_key).unwrap_or(0) + 1;
        env.storage().instance().set(&counter_key, &rx_id);

        let rx = Prescription {
            id: rx_id,
            patient,
            provider,
            lens_type,
            left_eye,
            right_eye,
            contact_data,
            issued_at: env.ledger().timestamp(),
            expires_at: env.ledger().timestamp() + duration_seconds,
            verified: false,
            metadata_hash,
        };

        prescription::save_prescription(&env, &rx);

        Ok(rx_id)
    }

    /// Get a prescription by ID
    pub fn get_prescription(env: Env, rx_id: u64) -> Result<Prescription, ContractError> {
        prescription::get_prescription(&env, rx_id).ok_or(ContractError::RecordNotFound)
    }

    /// Get all prescription IDs for a patient
    pub fn get_prescription_history(env: Env, patient: Address) -> Vec<u64> {
        prescription::get_patient_history(&env, patient)
    }

    /// Verify a prescription (e.g., by a pharmacy or another provider)
    pub fn verify_prescription(
        env: Env,
        rx_id: u64,
        verifier: Address,
    ) -> Result<bool, ContractError> {
        // Ensure verifier exists
        VisionRecordsContract::get_user(env.clone(), verifier.clone())?;

        Ok(prescription::verify_prescription(&env, rx_id, verifier))
    }

    /// Contract version
    pub fn version() -> u32 {
        1
    }

    // ── Emergency Access ──────────────────────────────────────────

    /// Request emergency access. Requester must attest to the condition.
    /// `emergency_contacts` are notified by recording them in the grant.
    /// Default window: 4 hours (14_400 seconds); caller may pass shorter.
    #[allow(clippy::arithmetic_side_effects)]
    pub fn request_emergency_access(
        env: Env,
        requester: Address,
        patient: Address,
        condition: EmergencyCondition,
        attestation: String,
        emergency_contacts: Vec<Address>,
        duration_seconds: u64, // recommend ≤ 14_400 (4 h)
    ) -> Result<u64, ContractError> {
        requester.require_auth();

        // Attestation must not be empty
        if attestation.is_empty() {
            return Err(ContractError::InvalidInput);
        }

        // Assign ID
        let id: u64 = env.storage().instance().get(&EMRG_CTR).unwrap_or(0) + 1;
        env.storage().instance().set(&EMRG_CTR, &id);

        let now = env.ledger().timestamp();

        let grant = EmergencyAccess {
            id,
            patient: patient.clone(),
            requester: requester.clone(),
            condition,
            attestation,
            granted_at: now,
            expires_at: now + duration_seconds,
            status: EmergencyStatus::Active,
            notified_contacts: emergency_contacts,
        };

        let key = (symbol_short!("EMRG"), id);
        env.storage().persistent().set(&key, &grant);

        // Write audit entry
        Self::write_emergency_audit(&env, id, requester, String::from_str(&env, "GRANTED"), now);

        Ok(id)
    }

    #[test]
    fn test_register_user() {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register(VisionRecordsContract, ());
        let client = VisionRecordsContractClient::new(&env, &contract_id);

        let admin = Address::generate(&env);
        client.initialize(&admin);

        let user = Address::generate(&env);
        let name = String::from_str(&env, "Dr. Smith");

        client.register_user(&user, &Role::Optometrist, &name);

        let user_data = client.get_user(&user);
        assert_eq!(user_data.role, Role::Optometrist);
        assert!(user_data.is_active);
  }
  
    /// Retrieve an emergency access grant.
    pub fn get_emergency_access(
        env: Env,
        access_id: u64,
    ) -> Result<EmergencyAccess, ContractError> {
        let key = (symbol_short!("EMRG"), access_id);
        env.storage()
            .persistent()
            .get(&key)
            .ok_or(ContractError::RecordNotFound)

    }

    /// Check whether an emergency grant is currently valid.
    pub fn is_emergency_access_valid(env: Env, access_id: u64) -> bool {
        let key = (symbol_short!("EMRG"), access_id);
        if let Some(grant) = env.storage().persistent().get::<_, EmergencyAccess>(&key) {
            return grant.status == EmergencyStatus::Active
                && grant.expires_at > env.ledger().timestamp();
        }
        false
    }

    /// Revoke an active emergency grant. Only the original patient or admin may do this.
    pub fn revoke_emergency_access(
        env: Env,
        caller: Address,
        access_id: u64,
    ) -> Result<(), ContractError> {
        caller.require_auth();

        let admin: Address = env
            .storage()
            .instance()
            .get(&ADMIN)
            .ok_or(ContractError::NotInitialized)?;

        let key = (symbol_short!("EMRG"), access_id);
        let mut grant: EmergencyAccess = env
            .storage()
            .persistent()
            .get(&key)
            .ok_or(ContractError::RecordNotFound)?;

        let record_id =
            client.add_record(&patient, &provider, &RecordType::Examination, &data_hash);

        assert_eq!(record_id, 1);

        let record = client.get_record(&record_id);
        assert_eq!(record.patient, patient);
        assert_eq!(record.provider, provider);

        if caller != grant.patient && caller != admin {
            return Err(ContractError::Unauthorized);
        }

        grant.status = EmergencyStatus::Revoked;
        env.storage().persistent().set(&key, &grant);

        Self::write_emergency_audit(
            &env,
            access_id,
            caller,
            String::from_str(&env, "REVOKED"),
            env.ledger().timestamp(),
        );

        Ok(())
    }

    /// Record that a requester actually accessed a record under emergency authority.
    /// Call this every time a record is read under an emergency grant.
    pub fn log_emergency_record_access(
        env: Env,
        requester: Address,
        access_id: u64,
    ) -> Result<(), ContractError> {
        requester.require_auth();

        if !Self::is_emergency_access_valid(env.clone(), access_id) {
            return Err(ContractError::AccessDenied);
        }

        Self::write_emergency_audit(
            &env,
            access_id,
            requester,
            String::from_str(&env, "ACCESSED"),
            env.ledger().timestamp(),
        );

        Ok(())
    }

    fn write_emergency_audit(
        env: &Env,
        access_id: u64,
        actor: Address,
        action: String,
        timestamp: u64,
    ) {
        let audit_key = (symbol_short!("EMRG_LOG"), access_id);
        let mut log: Vec<EmergencyAuditEntry> = env
            .storage()
            .persistent()
            .get(&audit_key)
            .unwrap_or(Vec::new(env));

        // Grant access
        client.grant_access(&patient, &doctor, &AccessLevel::Read, &86400);

        assert_eq!(client.check_access(&patient, &doctor), AccessLevel::Read);
        log.push_back(EmergencyAuditEntry {
            access_id,
            actor,
            action,
            timestamp,
        });

        env.storage().persistent().set(&audit_key, &log);
    }
}
