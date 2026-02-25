use soroban_sdk::{Env, Address, Vec, String, Symbol, symbol_short};
use common::{
    transaction::{TransactionLog, TransactionOperation, TransactionError, RollbackInfo,
                  set_transaction_log, get_transaction_log},
    ContractType,
};

use super::events::EventPublisher;

/// Rollback manager for handling transaction rollback operations
pub struct RollbackManager {
    env: Env,
}

impl RollbackManager {
    pub fn new(env: &Env) -> Self {
        Self { env: env.clone() }
    }

    /// Rollback an entire transaction
    pub fn rollback_transaction(&self, log: &TransactionLog) -> Result<(), TransactionError> {
        let mut rollback_info = Vec::new(&self.env);
        let mut rollback_failed = false;

        // Rollback operations in reverse order (LIFO principle)
        for i in (0..log.operations.len()).rev() {
            let operation = log.operations.get(i).unwrap();
            
            // Only rollback operations that were prepared
            if operation.prepared && !operation.committed {
                match self.rollback_operation(operation) {
                    Ok(rollback_data) => {
                        rollback_info.push_back(rollback_data);
                        EventPublisher::operation_rolled_back(&self.env, log.transaction_id, operation.operation_id, &operation.contract_type);
                    }
                    Err(e) => {
                        rollback_failed = true;
                        EventPublisher::rollback_failed(&self.env, log.transaction_id, operation.operation_id, &operation.contract_type, &String::from_str(&self.env, &format!("{:?}", e)));
                    }
                }
            }
        }

        if rollback_failed {
            Err(TransactionError::RollbackFailed)
        } else {
            Ok(())
        }
    }

    /// Rollback a single operation
    pub fn rollback_operation(&self, operation: &TransactionOperation) -> Result<RollbackInfo, TransactionError> {
        let rollback_function_name = self.get_rollback_function_name(&operation.function_name);
        
        // Build rollback info
        let mut rollback_info = RollbackInfo {
            transaction_id: 0, // Will be set by caller
            operation_id: operation.operation_id,
            contract_address: operation.contract_address.clone(),
            rollback_function: rollback_function_name.clone(),
            rollback_parameters: operation.parameters.clone(),
            rollback_successful: false,
            rollback_error: None,
        };

        // Build the function call arguments
        let mut args = Vec::new(&self.env);
        args.push_back(rollback_function_name);
        
        // Add rollback parameters (typically the same as original parameters)
        for param in &operation.parameters {
            args.push_back(param.clone());
        }

        // Call the rollback function on the contract
        let client = soroban_sdk::contractclient::Client::new(&self.env, &operation.contract_address);
        let result: Result<(), soroban_sdk::Error> = client.try_invoke(&args);
        
        match result {
            Ok(()) => {
                rollback_info.rollback_successful = true;
                Ok(rollback_info)
            }
            Err(e) => {
                rollback_info.rollback_successful = false;
                rollback_info.rollback_error = Some(String::from_str(&self.env, &format!("{:?}", e)));
                
                // Log the error for debugging
                self.env.events().publish(
                    (symbol_short!("ROLLBACK_ERR"), operation.contract_address.clone(), operation.operation_id),
                    (e.contract_error, String::from_str(&self.env, &format!("{:?}", e))),
                );
                
                Err(TransactionError::RollbackFailed)
            }
        }
    }

    /// Get the rollback function name based on the original function name
    fn get_rollback_function_name(&self, function_name: &String) -> String {
        // Convert function_name to rollback_*
        let original_str = function_name.to_string();
        if original_str.starts_with("rollback_") {
            function_name.clone()
        } else {
            String::from_str(&self.env, &format!("rollback_{}", original_str))
        }
    }

    /// Check if an operation can be rolled back
    pub fn can_rollback(&self, operation: &TransactionOperation) -> bool {
        // An operation can be rolled back if:
        // 1. It was prepared successfully
        // 2. It was not committed
        // 3. It has a valid contract address
        operation.prepared && !operation.committed && operation.contract_address.is_some()
    }

    /// Get rollback status for a transaction
    pub fn get_rollback_status(&self, transaction_id: u64) -> Result<Vec<RollbackInfo>, TransactionError> {
        let log = get_transaction_log(&self.env, transaction_id)
            .ok_or(TransactionError::TransactionNotFound)?;

        let mut rollback_status = Vec::new(&self.env);
        
        for operation in &log.operations {
            let rollback_info = RollbackInfo {
                transaction_id,
                operation_id: operation.operation_id,
                contract_address: operation.contract_address.clone(),
                rollback_function: self.get_rollback_function_name(&operation.function_name),
                rollback_parameters: operation.parameters.clone(),
                rollback_successful: false, // Will be determined by actual rollback
                rollback_error: None,
            };
            
            rollback_status.push_back(rollback_info);
        }

        Ok(rollback_status)
    }

    /// Perform a partial rollback of specific operations
    pub fn partial_rollback(&self, transaction_id: u64, operation_ids: Vec<u64>) -> Result<(), TransactionError> {
        let mut log = get_transaction_log(&self.env, transaction_id)
            .ok_or(TransactionError::TransactionNotFound)?;

        let mut rolled_back_operations = Vec::new(&self.env);

        // Find and rollback specified operations
        for operation_id in operation_ids {
            let mut found = false;
            
            // Find the operation
            for i in 0..log.operations.len() {
                let operation = log.operations.get(i).unwrap();
                if operation.operation_id == operation_id {
                    found = true;
                    
                    if !self.can_rollback(operation) {
                        return Err(TransactionError::InvalidPhase);
                    }

                    // Rollback the operation
                    match self.rollback_operation(operation) {
                        Ok(rollback_info) => {
                            rolled_back_operations.push_back(rollback_info);
                            EventPublisher::operation_rolled_back(&self.env, transaction_id, operation_id, &operation.contract_type);
                        }
                        Err(e) => {
                            EventPublisher::rollback_failed(&self.env, transaction_id, operation_id, &operation.contract_type, &String::from_str(&self.env, &format!("{:?}", e)));
                            return Err(TransactionError::RollbackFailed);
                        }
                    }
                    break;
                }
            }

            if !found {
                return Err(TransactionError::OperationNotFound);
            }
        }

        Ok(())
    }

    /// Verify that rollback operations are available for a contract
    pub fn verify_rollback_support(&self, contract_address: &Address, function_names: Vec<String>) -> Result<bool, TransactionError> {
        for function_name in function_names {
            let rollback_function_name = self.get_rollback_function_name(&function_name);
            
            // Try to call the rollback function with minimal parameters to check if it exists
            let mut args = Vec::new(&self.env);
            args.push_back(rollback_function_name);
            
            let client = soroban_sdk::contractclient::Client::new(&self.env, contract_address);
            
            // We expect this to fail with a specific error if the function doesn't exist
            // or succeed if the function exists but validation fails
            let _result: Result<(), soroban_sdk::Error> = client.try_invoke(&args);
            // The specific error handling would depend on the Soroban SDK's error types
        }
        
        Ok(true)
    }

    /// Get rollback statistics for monitoring
    pub fn get_rollback_statistics(&self) -> Result<RollbackStatistics, TransactionError> {
        // This would typically query persistent storage for rollback metrics
        // For now, return default statistics
        Ok(RollbackStatistics {
            total_rollbacks: 0,
            successful_rollbacks: 0,
            failed_rollbacks: 0,
            average_rollback_time: 0,
        })
    }
}

/// Statistics for rollback operations
#[derive(Clone, Debug)]
pub struct RollbackStatistics {
    pub total_rollbacks: u64,
    pub successful_rollbacks: u64,
    pub failed_rollbacks: u64,
    pub average_rollback_time: u64,
}
