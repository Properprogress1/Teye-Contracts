#![no_std]

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, symbol_short, Address, Env, String,
    Symbol, Vec,
};
pub mod emergency;
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

// Contract errors
#[contracterror]
#[derive(Clone, Debug, Eq, PartialEq)]
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

    /// Contract version
    pub fn version() -> u32 {
        1
    }

    // ── Emergency Access ──────────────────────────────────────────

    /// Request emergency access. Requester must attest to the condition.
    /// `emergency_contacts` are notified by recording them in the grant.
    /// Default window: 4 hours (14_400 seconds); caller may pass shorter.
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
        if attestation.len() == 0 {
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

        log.push_back(EmergencyAuditEntry {
            access_id,
            actor,
            action,
            timestamp,
        });

        env.storage().persistent().set(&audit_key, &log);
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use soroban_sdk::testutils::Address as _;
    use soroban_sdk::Env;

    #[test]
    fn test_initialize() {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register(VisionRecordsContract, ());
        let client = VisionRecordsContractClient::new(&env, &contract_id);

        let admin = Address::generate(&env);
        client.initialize(&admin);

        assert!(client.is_initialized());
        assert_eq!(client.get_admin(), admin);
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

    #[test]
    fn test_add_and_get_record() {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register(VisionRecordsContract, ());
        let client = VisionRecordsContractClient::new(&env, &contract_id);

        let admin = Address::generate(&env);
        client.initialize(&admin);

        let patient = Address::generate(&env);
        let provider = Address::generate(&env);
        let data_hash = String::from_str(&env, "QmHash123");

        let record_id =
            client.add_record(&patient, &provider, &RecordType::Examination, &data_hash);

        assert_eq!(record_id, 1);

        let record = client.get_record(&record_id);
        assert_eq!(record.patient, patient);
        assert_eq!(record.provider, provider);
    }

    #[test]
    fn test_access_control() {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register(VisionRecordsContract, ());
        let client = VisionRecordsContractClient::new(&env, &contract_id);

        let admin = Address::generate(&env);
        client.initialize(&admin);

        let patient = Address::generate(&env);
        let doctor = Address::generate(&env);

        // Initially no access
        assert_eq!(client.check_access(&patient, &doctor), AccessLevel::None);

        // Grant access
        client.grant_access(&patient, &doctor, &AccessLevel::Read, &86400);

        assert_eq!(client.check_access(&patient, &doctor), AccessLevel::Read);

        // Revoke access
        client.revoke_access(&patient, &doctor);
        assert_eq!(client.check_access(&patient, &doctor), AccessLevel::None);
    }

    #[test]
    fn test_request_emergency_access() {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register(VisionRecordsContract, ());
        let client = VisionRecordsContractClient::new(&env, &contract_id);

        let admin = Address::generate(&env);
        client.initialize(&admin);

        let patient = Address::generate(&env);
        let requester = Address::generate(&env);
        let contacts = Vec::new(&env);
        let attestation = String::from_str(&env, "Patient unconscious, life-threatening emergency");

        let access_id = client.request_emergency_access(
            &requester,
            &patient,
            &EmergencyCondition::Unconscious,
            &attestation,
            &contacts,
            &14400_u64,
        );

        assert_eq!(access_id, 1);

        let grant = client.get_emergency_access(&access_id);
        assert_eq!(grant.patient, patient);
        assert_eq!(grant.requester, requester);
        assert_eq!(grant.condition, EmergencyCondition::Unconscious);
        assert_eq!(grant.status, EmergencyStatus::Active);
    }

    #[test]
    fn test_emergency_access_invalid_without_attestation() {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register(VisionRecordsContract, ());
        let client = VisionRecordsContractClient::new(&env, &contract_id);

        let admin = Address::generate(&env);
        client.initialize(&admin);

        let patient = Address::generate(&env);
        let requester = Address::generate(&env);
        let contacts = Vec::new(&env);
        let empty_attestation = String::from_str(&env, "");

        let result = client.try_request_emergency_access(
            &requester,
            &patient,
            &EmergencyCondition::LifeThreatening,
            &empty_attestation,
            &contacts,
            &14400_u64,
        );

        assert!(result.is_err());
    }

    #[test]
    fn test_is_emergency_access_valid() {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register(VisionRecordsContract, ());
        let client = VisionRecordsContractClient::new(&env, &contract_id);

        let admin = Address::generate(&env);
        client.initialize(&admin);

        let patient = Address::generate(&env);
        let requester = Address::generate(&env);
        let contacts = Vec::new(&env);
        let attestation = String::from_str(&env, "Surgical emergency requiring immediate access");

        let access_id = client.request_emergency_access(
            &requester,
            &patient,
            &EmergencyCondition::SurgicalEmergency,
            &attestation,
            &contacts,
            &14400_u64,
        );

        assert!(client.is_emergency_access_valid(&access_id));
    }

    #[test]
    fn test_revoke_emergency_access_by_patient() {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register(VisionRecordsContract, ());
        let client = VisionRecordsContractClient::new(&env, &contract_id);

        let admin = Address::generate(&env);
        client.initialize(&admin);

        let patient = Address::generate(&env);
        let requester = Address::generate(&env);
        let contacts = Vec::new(&env);
        let attestation = String::from_str(&env, "Mass casualty event");

        let access_id = client.request_emergency_access(
            &requester,
            &patient,
            &EmergencyCondition::Masscasualties,
            &attestation,
            &contacts,
            &14400_u64,
        );

        assert!(client.is_emergency_access_valid(&access_id));

        client.revoke_emergency_access(&patient, &access_id);

        let grant = client.get_emergency_access(&access_id);
        assert_eq!(grant.status, EmergencyStatus::Revoked);
        assert!(!client.is_emergency_access_valid(&access_id));
    }

    #[test]
    fn test_revoke_emergency_access_by_admin() {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register(VisionRecordsContract, ());
        let client = VisionRecordsContractClient::new(&env, &contract_id);

        let admin = Address::generate(&env);
        client.initialize(&admin);

        let patient = Address::generate(&env);
        let requester = Address::generate(&env);
        let contacts = Vec::new(&env);
        let attestation = String::from_str(&env, "Life threatening situation");

        let access_id = client.request_emergency_access(
            &requester,
            &patient,
            &EmergencyCondition::LifeThreatening,
            &attestation,
            &contacts,
            &14400_u64,
        );

        // Admin can also revoke
        client.revoke_emergency_access(&admin, &access_id);

        let grant = client.get_emergency_access(&access_id);
        assert_eq!(grant.status, EmergencyStatus::Revoked);
    }

    #[test]
    fn test_unauthorized_revoke_fails() {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register(VisionRecordsContract, ());
        let client = VisionRecordsContractClient::new(&env, &contract_id);

        let admin = Address::generate(&env);
        client.initialize(&admin);

        let patient = Address::generate(&env);
        let requester = Address::generate(&env);
        let random = Address::generate(&env);
        let contacts = Vec::new(&env);
        let attestation = String::from_str(&env, "Emergency situation");

        let access_id = client.request_emergency_access(
            &requester,
            &patient,
            &EmergencyCondition::Unconscious,
            &attestation,
            &contacts,
            &14400_u64,
        );

        // Random address cannot revoke
        let result = client.try_revoke_emergency_access(&random, &access_id);
        assert!(result.is_err());
    }

    #[test]
    fn test_log_emergency_record_access() {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register(VisionRecordsContract, ());
        let client = VisionRecordsContractClient::new(&env, &contract_id);

        let admin = Address::generate(&env);
        client.initialize(&admin);

        let patient = Address::generate(&env);
        let requester = Address::generate(&env);
        let contacts = Vec::new(&env);
        let attestation = String::from_str(&env, "Emergency access required");

        let access_id = client.request_emergency_access(
            &requester,
            &patient,
            &EmergencyCondition::LifeThreatening,
            &attestation,
            &contacts,
            &14400_u64,
        );

        // Should succeed — grant is active
        client.log_emergency_record_access(&requester, &access_id);
    }

    #[test]
    fn test_log_access_fails_after_revoke() {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register(VisionRecordsContract, ());
        let client = VisionRecordsContractClient::new(&env, &contract_id);

        let admin = Address::generate(&env);
        client.initialize(&admin);

        let patient = Address::generate(&env);
        let requester = Address::generate(&env);
        let contacts = Vec::new(&env);
        let attestation = String::from_str(&env, "Emergency access required");

        let access_id = client.request_emergency_access(
            &requester,
            &patient,
            &EmergencyCondition::LifeThreatening,
            &attestation,
            &contacts,
            &14400_u64,
        );

        client.revoke_emergency_access(&patient, &access_id);

        // Should fail — grant is revoked
        let result = client.try_log_emergency_record_access(&requester, &access_id);
        assert!(result.is_err());
    }

    #[test]
    fn test_emergency_contacts_stored() {
        let env = Env::default();
        env.mock_all_auths();

        let contract_id = env.register(VisionRecordsContract, ());
        let client = VisionRecordsContractClient::new(&env, &contract_id);

        let admin = Address::generate(&env);
        client.initialize(&admin);

        let patient = Address::generate(&env);
        let requester = Address::generate(&env);
        let contact1 = Address::generate(&env);
        let contact2 = Address::generate(&env);

        let mut contacts = Vec::new(&env);
        contacts.push_back(contact1.clone());
        contacts.push_back(contact2.clone());

        let attestation = String::from_str(&env, "Life-threatening emergency");

        let access_id = client.request_emergency_access(
            &requester,
            &patient,
            &EmergencyCondition::LifeThreatening,
            &attestation,
            &contacts,
            &14400_u64,
        );

        let grant = client.get_emergency_access(&access_id);
        assert_eq!(grant.notified_contacts.len(), 2);
    }
}
