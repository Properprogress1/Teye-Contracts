use soroban_sdk::{Address, String, Vec, Env};
use common::transaction::TransactionError;

const MIN_TIMEOUT_SECONDS: u64 = 30; // 30 seconds
const MAX_TIMEOUT_SECONDS: u64 = 86400 * 7; // 7 days
const MAX_OPERATIONS_PER_TRANSACTION: u32 = 50;
const MAX_METADATA_ITEMS: u32 = 20;
const MAX_PARAMETERS_PER_OPERATION: u32 = 10;

/// Validates timeout configuration
pub fn validate_timeout(timeout_seconds: u64) -> Result<(), TransactionError> {
    if timeout_seconds < MIN_TIMEOUT_SECONDS || timeout_seconds > MAX_TIMEOUT_SECONDS {
        Err(TransactionError::InvalidInput)
    } else {
        Ok(())
    }
}

/// Validates address format
pub fn validate_address(address: &Address) -> Result<(), TransactionError> {
    // In Soroban, Address is always valid if it exists
    // Additional validation could be added here if needed
    if address.is_none() {
        Err(TransactionError::InvalidInput)
    } else {
        Ok(())
    }
}

/// Validates operation count
pub fn validate_operation_count(count: u32) -> Result<(), TransactionError> {
    if count == 0 || count > MAX_OPERATIONS_PER_TRANSACTION {
        Err(TransactionError::InvalidInput)
    } else {
        Ok(())
    }
}

/// Validates metadata size
pub fn validate_metadata(metadata: &Vec<String>) -> Result<(), TransactionError> {
    if metadata.len() > MAX_METADATA_ITEMS {
        Err(TransactionError::InvalidInput)
    } else {
        Ok(())
    }
}

/// Validates operation parameters
pub fn validate_operation_parameters(parameters: &Vec<String>) -> Result<(), TransactionError> {
    if parameters.len() > MAX_PARAMETERS_PER_OPERATION {
        Err(TransactionError::InvalidInput)
    } else {
        Ok(())
    }
}

/// Validates function name
pub fn validate_function_name(name: &String) -> Result<(), TransactionError> {
    let len = name.len();
    if len == 0 || len > 64 {
        Err(TransactionError::InvalidInput)
    } else {
        Ok(())
    }
}

/// Validates resource identifier
pub fn validate_resource_id(resource_id: &String) -> Result<(), TransactionError> {
    let len = resource_id.len();
    if len == 0 || len > 128 {
        Err(TransactionError::InvalidInput)
    } else {
        Ok(())
    }
}

/// Validates transaction metadata
pub fn validate_transaction_metadata(
    initiator: &Address,
    operations_count: u32,
    timeout_seconds: u64,
    metadata: &Vec<String>,
) -> Result<(), TransactionError> {
    // Validate initiator address
    validate_address(initiator)?;
    
    // Validate operation count
    validate_operation_count(operations_count)?;
    
    // Validate timeout
    validate_timeout(timeout_seconds)?;
    
    // Validate metadata
    validate_metadata(metadata)?;
    
    Ok(())
}

/// Validates transaction operation
pub fn validate_transaction_operation(
    operation_id: u64,
    contract_address: &Address,
    function_name: &String,
    parameters: &Vec<String>,
    locked_resources: &Vec<String>,
) -> Result<(), TransactionError> {
    // Validate operation ID
    if operation_id == 0 {
        return Err(TransactionError::InvalidInput);
    }
    
    // Validate contract address
    validate_address(contract_address)?;
    
    // Validate function name
    validate_function_name(function_name)?;
    
    // Validate parameters
    validate_operation_parameters(parameters)?;
    
    // Validate locked resources
    for resource in locked_resources {
        validate_resource_id(resource)?;
    }
    
    Ok(())
}

/// Validates transaction phase transition
pub fn validate_phase_transition(
    from_phase: &common::transaction::TransactionPhase,
    to_phase: &common::transaction::TransactionPhase,
) -> Result<(), TransactionError> {
    use common::transaction::TransactionPhase;
    
    match (from_phase, to_phase) {
        // Valid transitions
        (TransactionPhase::Preparing, TransactionPhase::Prepared) |
        (TransactionPhase::Prepared, TransactionPhase::Committed) |
        (TransactionPhase::Prepared, TransactionPhase::RolledBack) |
        (TransactionPhase::Preparing, TransactionPhase::RolledBack) |
        (TransactionPhase::Preparing, TransactionPhase::TimedOut) |
        (TransactionPhase::Prepared, TransactionPhase::TimedOut) => Ok(()),
        
        // Invalid transitions
        _ => Err(TransactionError::InvalidPhase),
    }
}

/// Validates rollback operation
pub fn validate_rollback_operation(
    prepared: bool,
    committed: bool,
) -> Result<(), TransactionError> {
    // Can only rollback operations that were prepared but not committed
    if !prepared || committed {
        Err(TransactionError::InvalidPhase)
    } else {
        Ok(())
    }
}

/// Validates deadlock detection parameters
pub fn validate_deadlock_detection(
    transaction_id: u64,
    operations: &common::transaction::Vec<common::transaction::TransactionOperation>,
) -> Result<(), TransactionError> {
    if transaction_id == 0 {
        return Err(TransactionError::InvalidInput);
    }
    
    if operations.is_empty() {
        return Err(TransactionError::InvalidInput);
    }
    
    // Validate each operation's locked resources
    for operation in operations {
        for resource in &operation.locked_resources {
            validate_resource_id(resource)?;
        }
    }
    
    Ok(())
}

/// Validates timeout configuration
pub fn validate_timeout_config(
    config: &common::transaction::TransactionTimeoutConfig,
) -> Result<(), TransactionError> {
    // Validate default timeout
    validate_timeout(config.default_timeout)?;
    
    // Validate max timeout
    if config.max_timeout < config.default_timeout {
        return Err(TransactionError::InvalidInput);
    }
    
    // Validate contract-specific timeouts
    for (_, timeout) in &config.contract_timeouts {
        validate_timeout(*timeout)?;
    }
    
    Ok(())
}

/// Checks if a transaction is expired
pub fn is_transaction_expired_check(
    created_at: u64,
    timeout_seconds: u64,
    current_timestamp: u64,
) -> bool {
    let deadline = created_at.saturating_add(timeout_seconds);
    current_timestamp > deadline
}

/// Validates batch operation parameters
pub fn validate_batch_operation(
    transaction_ids: &Vec<u64>,
    max_batch_size: u32,
) -> Result<(), TransactionError> {
    if transaction_ids.is_empty() {
        return Err(TransactionError::InvalidInput);
    }
    
    if transaction_ids.len() > max_batch_size as usize {
        return Err(TransactionError::InvalidInput);
    }
    
    // Validate each transaction ID
    for tx_id in transaction_ids {
        if *tx_id == 0 {
            return Err(TransactionError::InvalidInput);
        }
    }
    
    Ok(())
}

/// Validates configuration update parameters
pub fn validate_config_update(
    caller: &Address,
    admin: &Address,
    new_config: &common::transaction::TransactionTimeoutConfig,
) -> Result<(), TransactionError> {
    // Validate caller is admin
    if caller != admin {
        return Err(TransactionError::Unauthorized);
    }
    
    // Validate new configuration
    validate_timeout_config(new_config)?;
    
    Ok(())
}
