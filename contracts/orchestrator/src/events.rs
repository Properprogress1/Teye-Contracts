use soroban_sdk::{Env, Symbol, symbol_short, Address, String, Vec};
use common::{
    transaction::{TransactionLog, TransactionPhase, TransactionOperation, DeadlockInfo, ContractType},
};

/// Event published when transaction is started
#[soroban_sdk::contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TransactionStartedEvent {
    pub transaction_id: u64,
    pub initiator: Address,
    pub created_at: u64,
    pub timeout_seconds: u64,
}

/// Event published when transaction is prepared
#[soroban_sdk::contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TransactionPreparedEvent {
    pub transaction_id: u64,
    pub updated_at: u64,
    pub operation_count: u32,
}

/// Event published when transaction is committed
#[soroban_sdk::contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TransactionCommittedEvent {
    pub transaction_id: u64,
    pub updated_at: u64,
    pub operation_count: u32,
}

/// Event published when transaction is rolled back
#[soroban_sdk::contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TransactionRolledBackEvent {
    pub transaction_id: u64,
    pub updated_at: u64,
    pub phase: TransactionPhase,
    pub error: Option<String>,
}

/// Event published when transaction times out
#[soroban_sdk::contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TransactionTimedOutEvent {
    pub transaction_id: u64,
    pub updated_at: u64,
    pub deadline: u64,
}

/// Event publisher for orchestrator events
pub struct EventPublisher;

impl EventPublisher {
    /// Publish transaction started event
    pub fn transaction_started(env: &Env, log: &TransactionLog) {
        let event = TransactionStartedEvent {
            transaction_id: log.transaction_id,
            initiator: log.initiator.clone(),
            created_at: log.created_at,
            timeout_seconds: log.timeout_seconds,
        };
        env.events().publish(
            (symbol_short!("TX_STARTED"), log.transaction_id),
            (event,),
        );
    }

    /// Publish transaction prepared event
    pub fn transaction_prepared(env: &Env, log: &TransactionLog) {
        let event = TransactionPreparedEvent {
            transaction_id: log.transaction_id,
            updated_at: log.updated_at,
            operation_count: log.operations.len() as u32,
        };
        env.events().publish(
            (symbol_short!("TX_PREPARED"), log.transaction_id),
            (event,),
        );
    }

    /// Publish transaction committed event
    pub fn transaction_committed(env: &Env, log: &TransactionLog) {
        let event = TransactionCommittedEvent {
            transaction_id: log.transaction_id,
            updated_at: log.updated_at,
            operation_count: log.operations.len() as u32,
        };
        env.events().publish(
            (symbol_short!("TX_COMMITTED"), log.transaction_id),
            (event,),
        );
    }

    /// Publish transaction rolled back event
    pub fn transaction_rolled_back(env: &Env, log: &TransactionLog) {
        let event = TransactionRolledBackEvent {
            transaction_id: log.transaction_id,
            updated_at: log.updated_at,
            phase: log.phase.clone(),
            error: log.error.clone(),
        };
        env.events().publish(
            (symbol_short!("TX_ROLLBACK"), log.transaction_id),
            (event,),
        );
    }

    /// Publish transaction timed out event
    pub fn transaction_timed_out(env: &Env, log: &TransactionLog) {
        let event = TransactionTimedOutEvent {
            transaction_id: log.transaction_id,
            updated_at: log.updated_at,
            deadline: log.created_at + log.timeout_seconds,
        };
        env.events().publish(
            (symbol_short!("TX_TIMEOUT"), log.transaction_id),
            (event,),
        );
    }

    /// Publish operation prepared event
    pub fn operation_prepared(env: &Env, transaction_id: u64, operation_id: u64, contract_type: &ContractType) {
        env.events().publish(
            (symbol_short!("OP_PREPARED"), transaction_id, operation_id),
            (contract_type.clone(), env.ledger().timestamp()),
        );
    }

    /// Publish operation committed event
    pub fn operation_committed(env: &Env, transaction_id: u64, operation_id: u64, contract_type: &ContractType) {
        env.events().publish(
            (symbol_short!("OP_COMMITTED"), transaction_id, operation_id),
            (contract_type.clone(), env.ledger().timestamp()),
        );
    }

    /// Publish operation failed event
    pub fn operation_failed(env: &Env, transaction_id: u64, operation_id: u64, contract_type: &ContractType, error: &String) {
        env.events().publish(
            (symbol_short!("OP_FAILED"), transaction_id, operation_id),
            (contract_type.clone(), error.clone(), env.ledger().timestamp()),
        );
    }

    /// Publish operation rolled back event
    pub fn operation_rolled_back(env: &Env, transaction_id: u64, operation_id: u64, contract_type: &ContractType) {
        env.events().publish(
            (symbol_short!("OP_ROLLBACK"), transaction_id, operation_id),
            (contract_type.clone(), env.ledger().timestamp()),
        );
    }

    /// Publish rollback failed event
    pub fn rollback_failed(env: &Env, transaction_id: u64, operation_id: u64, contract_type: &ContractType, error: &String) {
        env.events().publish(
            (symbol_short!("ROLLBACK_FAIL"), transaction_id, operation_id),
            (contract_type.clone(), error.clone(), env.ledger().timestamp()),
        );
    }

    /// Publish deadlock detected event
    pub fn deadlock_detected(env: &Env, deadlock_info: &DeadlockInfo) {
        env.events().publish(
            (symbol_short!("DEADLOCK"), deadlock_info.transaction_id),
            (deadlock_info.conflicting_transactions.clone(), deadlock_info.conflicting_resources.clone(), deadlock_info.detected_at),
        );
    }

    /// Publish resource locked event
    pub fn resource_locked(env: &Env, transaction_id: u64, resource: &String, contract_address: &Address) {
        env.events().publish(
            (symbol_short!("RES_LOCKED"), transaction_id),
            (resource.clone(), contract_address.clone(), env.ledger().timestamp()),
        );
    }

    /// Publish resource unlocked event
    pub fn resource_unlocked(env: &Env, transaction_id: u64, resource: &String) {
        env.events().publish(
            (symbol_short!("RES_UNLOCKED"), transaction_id),
            (resource.clone(), env.ledger().timestamp()),
        );
    }

    /// Publish timeout configuration updated event
    pub fn timeout_config_updated(env: &Env, admin: &Address, default_timeout: u64, max_timeout: u64) {
        env.events().publish(
            (symbol_short!("TIMEOUT_CFG"), admin.clone()),
            (default_timeout, max_timeout, env.ledger().timestamp()),
        );
    }

    /// Publish transaction phase transition event
    pub fn phase_transition(env: &Env, transaction_id: u64, from_phase: &TransactionPhase, to_phase: &TransactionPhase) {
        env.events().publish(
            (symbol_short!("PHASE_TRANS"), transaction_id),
            (from_phase.clone(), to_phase.clone(), env.ledger().timestamp()),
        );
    }

    /// Publish batch operation started event
    pub fn batch_started(env: &Env, batch_id: u64, transaction_count: u64) {
        env.events().publish(
            (symbol_short!("BATCH_START"), batch_id),
            (transaction_count, env.ledger().timestamp()),
        );
    }

    /// Publish batch operation completed event
    pub fn batch_completed(env: &Env, batch_id: u64, successful_count: u64, failed_count: u64) {
        env.events().publish(
            (symbol_short!("BATCH_COMP"), batch_id),
            (successful_count, failed_count, env.ledger().timestamp()),
        );
    }

    /// Publish performance metrics event
    pub fn performance_metrics(env: &Env, transaction_id: u64, prepare_time: u64, commit_time: u64, total_time: u64) {
        env.events().publish(
            (symbol_short!("PERF_METRICS"), transaction_id),
            (prepare_time, commit_time, total_time),
        );
    }

    /// Publish gas consumption event
    pub fn gas_consumption(env: &Env, transaction_id: u64, operation_id: u64, gas_used: u64) {
        env.events().publish(
            (symbol_short!("GAS_USED"), transaction_id, operation_id),
            (gas_used, env.ledger().timestamp()),
        );
    }

    /// Publish error recovery event
    pub fn error_recovery(env: &Env, transaction_id: u64, error_type: &String, recovery_action: &String) {
        env.events().publish(
            (symbol_short!("ERROR_RECOVERY"), transaction_id),
            (error_type.clone(), recovery_action.clone(), env.ledger().timestamp()),
        );
    }

    /// Publish health check event
    pub fn health_check(env: &Env, active_transactions: u64, locked_resources: u64, pending_timeouts: u64) {
        env.events().publish(
            (symbol_short!("HEALTH_CHECK")),
            (active_transactions, locked_resources, pending_timeouts, env.ledger().timestamp()),
        );
    }

    /// Publish configuration change event
    pub fn configuration_changed(env: &Env, admin: &Address, config_key: &String, old_value: &String, new_value: &String) {
        env.events().publish(
            (symbol_short!("CONFIG_CHANGE"), admin.clone()),
            (config_key.clone(), old_value.clone(), new_value.clone(), env.ledger().timestamp()),
        );
    }

    /// Publish audit trail event
    pub fn audit_trail(env: &Env, transaction_id: u64, action: &String, actor: &Address, details: Vec<String>) {
        env.events().publish(
            (symbol_short!("AUDIT_TRAIL"), transaction_id, action.clone()),
            (actor.clone(), details, env.ledger().timestamp()),
        );
    }

    /// Publish security event
    pub fn security_event(env: &Env, event_type: &String, severity: &String, details: Vec<String>) {
        env.events().publish(
            (symbol_short!("SECURITY"), event_type.clone(), severity.clone()),
            (details, env.ledger().timestamp()),
        );
    }

    /// Publish monitoring event
    pub fn monitoring_event(env: &Env, metric_name: &String, metric_value: u64, threshold: Option<u64>) {
        env.events().publish(
            (symbol_short!("MONITORING"), metric_name.clone()),
            (metric_value, threshold, env.ledger().timestamp()),
        );
    }
}
