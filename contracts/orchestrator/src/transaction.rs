use soroban_sdk::{Env, Address, Vec, String, Symbol, symbol_short};
use common::{
    transaction::{TransactionLog, TransactionPhase, TransactionOperation, TransactionError, 
                  set_transaction_log, get_transaction_log},
    ContractType,
};

use super::events::EventPublisher;

/// Transaction manager for handling two-phase commit protocol
pub struct TransactionManager {
    env: Env,
}

impl TransactionManager {
    pub fn new(env: &Env) -> Self {
        Self { env: env.clone() }
    }

    /// Prepare phase: call prepare_* functions on all participating contracts
    pub fn prepare_phase(&self, log: &mut TransactionLog) -> Result<(), TransactionError> {
        log.phase = TransactionPhase::Preparing;
        set_transaction_log(&self.env, log);

        let mut prepared_operations = Vec::new(&self.env);
        
        // Prepare each operation
        for i in 0..log.operations.len() {
            let mut operation = log.operations.get(i).unwrap().clone();
            
            // Call prepare function on the target contract
            match self.call_prepare_function(&operation) {
                Ok(()) => {
                    operation.prepared = true;
                    prepared_operations.push_back(operation);
                    
                    // Publish operation prepared event
                    EventPublisher::operation_prepared(&self.env, log.transaction_id, operation.operation_id, &operation.contract_type);
                }
                Err(e) => {
                    operation.error = Some(String::from_str(&self.env, &format!("Prepare failed: {:?}", e)));
                    
                    // Publish operation failed event
                    EventPublisher::operation_failed(&self.env, log.transaction_id, operation.operation_id, &operation.contract_type, &operation.error.clone().unwrap());
                    
                    return Err(TransactionError::ContractCallFailed);
                }
            }
        }

        // Update operations with prepared status
        log.operations = prepared_operations;
        log.phase = TransactionPhase::Prepared;
        log.updated_at = self.env.ledger().timestamp();
        set_transaction_log(&self.env, log);

        // Publish transaction prepared event
        EventPublisher::transaction_prepared(&self.env, log);

        Ok(())
    }

    /// Commit phase: call commit_* functions on all prepared contracts
    pub fn commit_phase(&self, log: &mut TransactionLog) -> Result<(), TransactionError> {
        if log.phase != TransactionPhase::Prepared {
            return Err(TransactionError::InvalidPhase);
        }

        let mut committed_operations = Vec::new(&self.env);
        
        // Commit each operation in order
        for i in 0..log.operations.len() {
            let mut operation = log.operations.get(i).unwrap().clone();
            
            if !operation.prepared {
                return Err(TransactionError::InvalidPhase);
            }

            // Call commit function on the target contract
            match self.call_commit_function(&operation) {
                Ok(()) => {
                    operation.committed = true;
                    committed_operations.push_back(operation);
                    
                    // Publish operation committed event
                    EventPublisher::operation_committed(&self.env, log.transaction_id, operation.operation_id, &operation.contract_type);
                }
                Err(e) => {
                    operation.error = Some(String::from_str(&self.env, &format!("Commit failed: {:?}", e)));
                    
                    // Publish operation failed event
                    EventPublisher::operation_failed(&self.env, log.transaction_id, operation.operation_id, &operation.contract_type, &operation.error.clone().unwrap());
                    
                    return Err(TransactionError::ContractCallFailed);
                }
            }
        }

        // Update operations with committed status
        log.operations = committed_operations;
        log.updated_at = self.env.ledger().timestamp();
        set_transaction_log(&self.env, log);

        Ok(())
    }

    /// Call the prepare function on a target contract
    fn call_prepare_function(&self, operation: &TransactionOperation) -> Result<(), TransactionError> {
        let prepare_function_name = self.get_prepare_function_name(&operation.function_name);
        
        // Build the function call arguments
        let mut args = Vec::new(&self.env);
        args.push_back(prepare_function_name);
        
        // Add parameters
        for param in &operation.parameters {
            args.push_back(param.clone());
        }

        // Call the contract
        let client = soroban_sdk::contractclient::Client::new(&self.env, &operation.contract_address);
        let result: Result<(), soroban_sdk::Error> = client.try_invoke(&args);
        
        match result {
            Ok(()) => Ok(()),
            Err(e) => {
                // Log the error for debugging
                self.env.events().publish(
                    (symbol_short!("PREP_ERR"), operation.contract_address.clone(), operation.operation_id),
                    (e.contract_error, String::from_str(&self.env, &format!("{:?}", e))),
                );
                Err(TransactionError::ContractCallFailed)
            }
        }
    }

    /// Call the commit function on a target contract
    fn call_commit_function(&self, operation: &TransactionOperation) -> Result<(), TransactionError> {
        let commit_function_name = self.get_commit_function_name(&operation.function_name);
        
        // Build the function call arguments
        let mut args = Vec::new(&self.env);
        args.push_back(commit_function_name);
        
        // Add parameters
        for param in &operation.parameters {
            args.push_back(param.clone());
        }

        // Call the contract
        let client = soroban_sdk::contractclient::Client::new(&self.env, &operation.contract_address);
        let result: Result<(), soroban_sdk::Error> = client.try_invoke(&args);
        
        match result {
            Ok(()) => Ok(()),
            Err(e) => {
                // Log the error for debugging
                self.env.events().publish(
                    (symbol_short!("COMMIT_ERR"), operation.contract_address.clone(), operation.operation_id),
                    (e.contract_error, String::from_str(&self.env, &format!("{:?}", e))),
                );
                Err(TransactionError::ContractCallFailed)
            }
        }
    }

    /// Get the prepare function name based on the original function name
    fn get_prepare_function_name(&self, function_name: &String) -> String {
        // Convert function_name to prepare_*
        let original_str = function_name.to_string();
        if original_str.starts_with("prepare_") {
            function_name.clone()
        } else {
            String::from_str(&self.env, &format!("prepare_{}", original_str))
        }
    }

    /// Get the commit function name based on the original function name
    fn get_commit_function_name(&self, function_name: &String) -> String {
        // Convert function_name to commit_*
        let original_str = function_name.to_string();
        if original_str.starts_with("commit_") {
            function_name.clone()
        } else {
            String::from_str(&self.env, &format!("commit_{}", original_str))
        }
    }

    /// Validate that all operations in a transaction are compatible
    pub fn validate_transaction(&self, operations: &Vec<TransactionOperation>) -> Result<(), TransactionError> {
        if operations.is_empty() {
            return Err(TransactionError::InvalidInput);
        }

        // Check for duplicate operation IDs
        let mut operation_ids = Vec::new(&self.env);
        for operation in operations {
            if operation_ids.contains(&operation.operation_id) {
                return Err(TransactionError::InvalidInput);
            }
            operation_ids.push_back(operation.operation_id);
        }

        // Validate each operation
        for operation in operations {
            self.validate_operation(operation)?;
        }

        Ok(())
    }

    /// Validate a single operation
    fn validate_operation(&self, operation: &TransactionOperation) -> Result<(), TransactionError> {
        // Check contract address is valid
        if operation.contract_address.is_none() {
            return Err(TransactionError::InvalidInput);
        }

        // Check function name is not empty
        if operation.function_name.is_empty() {
            return Err(TransactionError::InvalidInput);
        }

        // Check operation ID is not zero
        if operation.operation_id == 0 {
            return Err(TransactionError::InvalidInput);
        }

        Ok(())
    }

    /// Check if a transaction can be safely committed
    pub fn can_commit(&self, log: &TransactionLog) -> bool {
        match log.phase {
            TransactionPhase::Prepared => {
                // Check that all operations are prepared
                for operation in &log.operations {
                    if !operation.prepared {
                        return false;
                    }
                }
                true
            }
            _ => false,
        }
    }

    /// Get the status of a specific operation within a transaction
    pub fn get_operation_status(&self, transaction_id: u64, operation_id: u64) -> Result<String, TransactionError> {
        let log = get_transaction_log(&self.env, transaction_id)
            .ok_or(TransactionError::TransactionNotFound)?;

        for operation in &log.operations {
            if operation.operation_id == operation_id {
                if operation.committed {
                    return Ok(String::from_str(&self.env, "committed"));
                } else if operation.prepared {
                    return Ok(String::from_str(&self.env, "prepared"));
                } else if operation.error.is_some() {
                    return Ok(String::from_str(&self.env, "failed"));
                } else {
                    return Ok(String::from_str(&self.env, "pending"));
                }
            }
        }

        Err(TransactionError::OperationNotFound)
    }

    /// Get all operations that failed during prepare or commit phase
    pub fn get_failed_operations(&self, transaction_id: u64) -> Result<Vec<TransactionOperation>, TransactionError> {
        let log = get_transaction_log(&self.env, transaction_id)
            .ok_or(TransactionError::TransactionNotFound)?;

        let mut failed_operations = Vec::new(&self.env);
        for operation in &log.operations {
            if operation.error.is_some() {
                failed_operations.push_back(operation.clone());
            }
        }

        Ok(failed_operations)
    }
}
