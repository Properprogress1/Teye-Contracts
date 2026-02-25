#![no_std]

pub mod recovery;

use recovery::{RecoveryError, RecoveryRequest};
use soroban_sdk::{contract, contractimpl, contracttype, symbol_short, Address, Env, Symbol, Vec, String};

/// Preparation data for guardian addition
#[contracttype]
#[derive(Clone, Debug)]
pub struct PrepareGuardianAddition {
    pub caller: Address,
    pub guardian: Address,
    pub timestamp: u64,
}

/// Preparation data for guardian removal
#[contracttype]
#[derive(Clone, Debug)]
pub struct PrepareGuardianRemoval {
    pub caller: Address,
    pub guardian: Address,
    pub timestamp: u64,
}

/// Preparation data for recovery threshold change
#[contracttype]
#[derive(Clone, Debug)]
pub struct PrepareThresholdChange {
    pub caller: Address,
    pub threshold: u32,
    pub timestamp: u64,
}

// ── Storage keys ─────────────────────────────────────────────────────────────

const ADMIN: Symbol = symbol_short!("ADMIN");
const INITIALIZED: Symbol = symbol_short!("INIT");

// ── Contract ─────────────────────────────────────────────────────────────────

#[contract]
pub struct IdentityContract;

#[contractimpl]
impl IdentityContract {
    /// Initialize the identity contract with an owner address.
    pub fn initialize(env: Env, owner: Address) -> Result<(), RecoveryError> {
        if env.storage().instance().has(&INITIALIZED) {
            return Err(RecoveryError::AlreadyInitialized);
        }

        env.storage().instance().set(&ADMIN, &owner);
        env.storage().instance().set(&INITIALIZED, &true);
        recovery::set_owner_active(&env, &owner);

        Ok(())
    }

    /// Add a guardian address for social recovery (max 5).
    pub fn add_guardian(env: Env, caller: Address, guardian: Address) -> Result<(), RecoveryError> {
        caller.require_auth();
        Self::require_active_owner(&env, &caller)?;
        recovery::add_guardian(&env, &caller, guardian)
    }

    /// Remove a guardian address.
    pub fn remove_guardian(
        env: Env,
        caller: Address,
        guardian: Address,
    ) -> Result<(), RecoveryError> {
        caller.require_auth();
        Self::require_active_owner(&env, &caller)?;
        recovery::remove_guardian(&env, &caller, &guardian)
    }

    /// Set the M-of-N approval threshold for recovery.
    pub fn set_recovery_threshold(
        env: Env,
        caller: Address,
        threshold: u32,
    ) -> Result<(), RecoveryError> {
        caller.require_auth();
        Self::require_active_owner(&env, &caller)?;
        recovery::set_threshold(&env, &caller, threshold)
    }

    /// A guardian initiates recovery, proposing a new address.
    /// The initiating guardian counts as the first approval.
    pub fn initiate_recovery(
        env: Env,
        guardian: Address,
        owner: Address,
        new_address: Address,
    ) -> Result<(), RecoveryError> {
        guardian.require_auth();
        recovery::initiate_recovery(&env, &guardian, &owner, new_address)
    }

    /// A guardian approves an active recovery request.
    pub fn approve_recovery(
        env: Env,
        guardian: Address,
        owner: Address,
    ) -> Result<(), RecoveryError> {
        guardian.require_auth();
        recovery::approve_recovery(&env, &guardian, &owner)
    }

    /// Execute recovery after cooldown and sufficient approvals.
    /// Transfers identity ownership and deactivates the old address.
    pub fn execute_recovery(
        env: Env,
        caller: Address,
        owner: Address,
    ) -> Result<Address, RecoveryError> {
        caller.require_auth();
        recovery::execute_recovery(&env, &owner)
    }

    /// Owner cancels an active recovery request.
    pub fn cancel_recovery(env: Env, caller: Address) -> Result<(), RecoveryError> {
        caller.require_auth();
        Self::require_active_owner(&env, &caller)?;
        recovery::cancel_recovery(&env, &caller)
    }

    /// Check if an address is an active identity owner.
    pub fn is_owner_active(env: Env, owner: Address) -> bool {
        recovery::is_owner_active(&env, &owner)
    }

    /// Get the list of guardians for an owner.
    pub fn get_guardians(env: Env, owner: Address) -> Vec<Address> {
        recovery::get_guardians(&env, &owner)
    }

    /// Get the recovery threshold for an owner.
    pub fn get_recovery_threshold(env: Env, owner: Address) -> u32 {
        recovery::get_threshold(&env, &owner)
    }

    /// Get the active recovery request for an owner, if any.
    pub fn get_recovery_request(env: Env, owner: Address) -> Option<RecoveryRequest> {
        recovery::get_recovery_request(&env, &owner)
    }

    // ===== Two-Phase Commit Hooks =====

    /// Prepare phase for add_guardian operation
    pub fn prepare_add_guardian(
        env: Env,
        caller: Address,
        guardian: Address,
    ) -> Result<(), RecoveryError> {
        // Validate all inputs without making state changes
        Self::require_active_owner(&env, &caller)?;
        
        // Check if guardian already exists
        let guardians = recovery::get_guardians(&env, &caller);
        if guardians.contains(&guardian) {
            return Err(RecoveryError::GuardianAlreadyExists);
        }

        // Check guardian limit
        if guardians.len() >= 5 {
            return Err(RecoveryError::TooManyGuardians);
        }

        // Store temporary preparation data
        let prep_key = (symbol_short!("PREP_ADD_GUARD"), caller.clone(), guardian.clone());
        let prep_data = PrepareGuardianAddition {
            caller: caller.clone(),
            guardian: guardian.clone(),
            timestamp: env.ledger().timestamp(),
        };
        env.storage().temporary().set(&prep_key, &prep_data);

        Ok(())
    }

    /// Commit phase for add_guardian operation
    pub fn commit_add_guardian(
        env: Env,
        caller: Address,
        guardian: Address,
    ) -> Result<(), RecoveryError> {
        // Retrieve preparation data
        let prep_key = (symbol_short!("PREP_ADD_GUARD"), caller.clone(), guardian.clone());
        let prep_data: PrepareGuardianAddition = env.storage().temporary().get(&prep_key)
            .ok_or(RecoveryError::Unauthorized)?; // Using Unauthorized as InvalidPhase equivalent

        // Verify preparation data matches commit parameters
        if prep_data.caller != caller || prep_data.guardian != guardian {
            return Err(RecoveryError::Unauthorized);
        }

        // Execute the actual guardian addition
        recovery::add_guardian(&env, &caller, guardian.clone())?;

        // Clean up preparation data
        env.storage().temporary().remove(&prep_key);

        Ok(())
    }

    /// Rollback for add_guardian operation
    pub fn rollback_add_guardian(
        env: Env,
        caller: Address,
        guardian: Address,
    ) -> Result<(), RecoveryError> {
        // Clean up preparation data
        let prep_key = (symbol_short!("PREP_ADD_GUARD"), caller, guardian);
        env.storage().temporary().remove(&prep_key);

        Ok(())
    }

    /// Prepare phase for remove_guardian operation
    pub fn prepare_remove_guardian(
        env: Env,
        caller: Address,
        guardian: Address,
    ) -> Result<(), RecoveryError> {
        // Validate all inputs without making state changes
        Self::require_active_owner(&env, &caller)?;
        
        // Check if guardian exists
        let guardians = recovery::get_guardians(&env, &caller);
        if !guardians.contains(&guardian) {
            return Err(RecoveryError::GuardianNotFound);
        }

        // Store temporary preparation data
        let prep_key = (symbol_short!("PREP_REM_GUARD"), caller.clone(), guardian.clone());
        let prep_data = PrepareGuardianRemoval {
            caller: caller.clone(),
            guardian: guardian.clone(),
            timestamp: env.ledger().timestamp(),
        };
        env.storage().temporary().set(&prep_key, &prep_data);

        Ok(())
    }

    /// Commit phase for remove_guardian operation
    pub fn commit_remove_guardian(
        env: Env,
        caller: Address,
        guardian: Address,
    ) -> Result<(), RecoveryError> {
        // Retrieve preparation data
        let prep_key = (symbol_short!("PREP_REM_GUARD"), caller.clone(), guardian.clone());
        let prep_data: PrepareGuardianRemoval = env.storage().temporary().get(&prep_key)
            .ok_or(RecoveryError::Unauthorized)?;

        // Verify preparation data matches commit parameters
        if prep_data.caller != caller || prep_data.guardian != guardian {
            return Err(RecoveryError::Unauthorized);
        }

        // Execute the actual guardian removal
        recovery::remove_guardian(&env, &caller, &guardian)?;

        // Clean up preparation data
        env.storage().temporary().remove(&prep_key);

        Ok(())
    }

    /// Rollback for remove_guardian operation
    pub fn rollback_remove_guardian(
        env: Env,
        caller: Address,
        guardian: Address,
    ) -> Result<(), RecoveryError> {
        // Clean up preparation data
        let prep_key = (symbol_short!("PREP_REM_GUARD"), caller, guardian);
        env.storage().temporary().remove(&prep_key);

        Ok(())
    }

    /// Prepare phase for set_recovery_threshold operation
    pub fn prepare_set_recovery_threshold(
        env: Env,
        caller: Address,
        threshold: u32,
    ) -> Result<(), RecoveryError> {
        // Validate all inputs without making state changes
        Self::require_active_owner(&env, &caller)?;
        
        // Validate threshold
        if threshold == 0 || threshold > 5 {
            return Err(RecoveryError::InvalidThreshold);
        }

        // Check current guardians count
        let guardians = recovery::get_guardians(&env, &caller);
        if threshold > guardians.len() as u32 {
            return Err(RecoveryError::InvalidThreshold);
        }

        // Store temporary preparation data
        let prep_key = (symbol_short!("PREP_SET_THRESH"), caller.clone());
        let prep_data = PrepareThresholdChange {
            caller: caller.clone(),
            threshold,
            timestamp: env.ledger().timestamp(),
        };
        env.storage().temporary().set(&prep_key, &prep_data);

        Ok(())
    }

    /// Commit phase for set_recovery_threshold operation
    pub fn commit_set_recovery_threshold(
        env: Env,
        caller: Address,
        threshold: u32,
    ) -> Result<(), RecoveryError> {
        // Retrieve preparation data
        let prep_key = (symbol_short!("PREP_SET_THRESH"), caller.clone());
        let prep_data: PrepareThresholdChange = env.storage().temporary().get(&prep_key)
            .ok_or(RecoveryError::Unauthorized)?;

        // Verify preparation data matches commit parameters
        if prep_data.caller != caller || prep_data.threshold != threshold {
            return Err(RecoveryError::Unauthorized);
        }

        // Execute the actual threshold change
        recovery::set_threshold(&env, &caller, threshold)?;

        // Clean up preparation data
        env.storage().temporary().remove(&prep_key);

        Ok(())
    }

    /// Rollback for set_recovery_threshold operation
    pub fn rollback_set_recovery_threshold(
        env: Env,
        caller: Address,
        _threshold: u32,
    ) -> Result<(), RecoveryError> {
        // Clean up preparation data
        let prep_key = (symbol_short!("PREP_SET_THRESH"), caller);
        env.storage().temporary().remove(&prep_key);

        Ok(())
    }

    // ── Internal helpers ─────────────────────────────────────────────────────

    fn require_active_owner(env: &Env, caller: &Address) -> Result<(), RecoveryError> {
        if !recovery::is_owner_active(env, caller) {
            return Err(RecoveryError::Unauthorized);
        }
        Ok(())
    }
}
