use soroban_sdk::{Env, Vec, String, Symbol, symbol_short};
use common::{
    transaction::{TransactionOperation, TransactionError, DeadlockInfo, RESOURCE_LOCKS},
    ContractType,
};

use super::events::EventPublisher;

/// Deadlock detector for preventing and resolving transaction deadlocks
pub struct DeadlockDetector {
    env: Env,
}

impl DeadlockDetector {
    pub fn new(env: &Env) -> Self {
        Self { env: env.clone() }
    }

    /// Check if a new transaction would cause a deadlock
    pub fn would_cause_deadlock(&self, transaction_id: &u64, operations: &Vec<TransactionOperation>) -> bool {
        // Build resource dependency graph
        let dependency_graph = self.build_dependency_graph(transaction_id, operations);
        
        // Check for cycles in the dependency graph
        self.has_cycles(&dependency_graph)
    }

    /// Build a dependency graph for the current transaction
    fn build_dependency_graph(&self, transaction_id: &u64, operations: &Vec<TransactionOperation>) -> DependencyGraph {
        let mut graph = DependencyGraph::new(&self.env);
        
        // Get current resource locks
        let current_locks: Vec<(String, u64)> = self.env.storage().instance()
            .get(&RESOURCE_LOCKS)
            .unwrap_or(Vec::new(&self.env));

        // Add nodes for the new transaction
        for operation in operations {
            for resource in &operation.locked_resources {
                graph.add_resource(resource.clone());
                
                // Check if this resource is locked by another transaction
                for (locked_resource, locked_tx_id) in &current_locks {
                    if locked_resource == resource && *locked_tx_id != *transaction_id {
                        // Add dependency: new transaction -> existing transaction
                        graph.add_dependency(*transaction_id, *locked_tx_id, resource.clone());
                    }
                }
            }
        }

        // Add existing transaction dependencies
        self.add_existing_dependencies(&mut graph, &current_locks);

        graph
    }

    /// Add dependencies for existing locked resources
    fn add_existing_dependencies(&self, graph: &mut DependencyGraph, current_locks: &Vec<(String, u64)>) {
        // Group locks by transaction
        let mut tx_resources = Vec::new(&self.env);
        
        for (resource, tx_id) in current_locks {
            let mut found = false;
            
            // Find existing entry for this transaction
            for i in 0..tx_resources.len() {
                if let Some((existing_tx_id, resources)) = tx_resources.get(i) {
                    if *existing_tx_id == *tx_id {
                        // Add resource to existing transaction
                        let mut new_resources = resources.clone();
                        new_resources.push_back(resource.clone());
                        tx_resources.set(i, (*existing_tx_id, new_resources));
                        found = true;
                        break;
                    }
                }
            }
            
            if !found {
                // Create new entry for this transaction
                let mut resources = Vec::new(&self.env);
                resources.push_back(resource.clone());
                tx_resources.push_back((*tx_id, resources));
            }
        }

        // Add dependencies between transactions that share resources
        for i in 0..tx_resources.len() {
            for j in (i + 1)..tx_resources.len() {
                if let Some((tx1, resources1)) = tx_resources.get(i) {
                    if let Some((tx2, resources2)) = tx_resources.get(j) {
                        // Check if transactions share any resources
                        for resource1 in resources1 {
                            for resource2 in resources2 {
                                if resource1 == resource2 {
                                    // Transactions share a resource, add bidirectional dependency
                                    graph.add_dependency(*tx1, *tx2, resource1.clone());
                                    graph.add_dependency(*tx2, *tx1, resource2.clone());
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    /// Check if the dependency graph has cycles (indicating deadlock)
    fn has_cycles(&self, graph: &DependencyGraph) -> bool {
        let mut visited = Vec::new(&self.env);
        let mut recursion_stack = Vec::new(&self.env);

        // Get all unique transactions in the graph
        let transactions = graph.get_all_transactions();

        for transaction in transactions {
            if !visited.contains(&transaction) {
                if self.dfs_has_cycle(graph, transaction, &mut visited, &mut recursion_stack) {
                    return true;
                }
            }
        }

        false
    }

    /// Depth-first search to detect cycles
    fn dfs_has_cycle(
        &self,
        graph: &DependencyGraph,
        transaction: u64,
        visited: &mut Vec<u64>,
        recursion_stack: &mut Vec<u64>,
    ) -> bool {
        visited.push_back(transaction);
        recursion_stack.push_back(transaction);

        // Get all dependencies for this transaction
        let dependencies = graph.get_dependencies(transaction);

        for (dependent_tx, _resource) in dependencies {
            if !visited.contains(&dependent_tx) {
                if self.dfs_has_cycle(graph, dependent_tx, visited, recursion_stack) {
                    return true;
                }
            } else if recursion_stack.contains(&dependent_tx) {
                // Found a back edge, indicating a cycle
                return true;
            }
        }

        recursion_stack.pop();
        false
    }

    /// Detect and resolve existing deadlocks
    pub fn detect_and_resolve_deadlocks(&self) -> Result<Vec<DeadlockInfo>, TransactionError> {
        let current_locks: Vec<(String, u64)> = self.env.storage().instance()
            .get(&RESOURCE_LOCKS)
            .unwrap_or(Vec::new(&self.env));

        if current_locks.is_empty() {
            return Ok(Vec::new(&self.env));
        }

        // Build dependency graph for current state
        let graph = self.build_current_dependency_graph(&current_locks);
        
        // Detect cycles
        let cycles = self.find_cycles(&graph);
        
        let mut resolved_deadlocks = Vec::new(&self.env);

        for cycle in cycles {
            let deadlock_info = self.resolve_deadlock(&cycle)?;
            resolved_deadlocks.push_back(deadlock_info);
        }

        Ok(resolved_deadlocks)
    }

    /// Build dependency graph for current locked resources
    fn build_current_dependency_graph(&self, current_locks: &Vec<(String, u64)>) -> DependencyGraph {
        let mut graph = DependencyGraph::new(&self.env);
        self.add_existing_dependencies(&mut graph, current_locks);
        graph
    }

    /// Find all cycles in the dependency graph
    fn find_cycles(&self, graph: &DependencyGraph) -> Vec<Vec<u64>> {
        let mut cycles = Vec::new(&self.env);
        let mut visited = Vec::new(&self.env);
        let mut path = Vec::new(&self.env);

        let transactions = graph.get_all_transactions();

        for transaction in transactions {
            if !visited.contains(&transaction) {
                self.find_cycles_from_node(graph, transaction, &mut visited, &mut path, &mut cycles);
            }
        }

        cycles
    }

    /// Find cycles starting from a specific node
    fn find_cycles_from_node(
        &self,
        graph: &DependencyGraph,
        transaction: u64,
        visited: &mut Vec<u64>,
        path: &mut Vec<u64>,
        cycles: &mut Vec<Vec<u64>>,
    ) {
        visited.push_back(transaction);
        path.push_back(transaction);

        let dependencies = graph.get_dependencies(transaction);

        for (dependent_tx, _resource) in dependencies {
            if let Some(cycle_start) = path.iter().position(|&tx| tx == dependent_tx) {
                // Found a cycle
                let mut cycle = Vec::new(&self.env);
                for i in cycle_start..path.len() {
                    if let Some(tx) = path.get(i) {
                        cycle.push_back(*tx);
                    }
                }
                cycles.push_back(cycle);
            } else if !visited.contains(&dependent_tx) {
                self.find_cycles_from_node(graph, dependent_tx, visited, path, cycles);
            }
        }

        path.pop();
    }

    /// Resolve a deadlock by choosing a victim transaction
    fn resolve_deadlock(&self, cycle: &Vec<u64>) -> Result<DeadlockInfo, TransactionError> {
        // Choose the transaction with the lowest ID as the victim (simple strategy)
        let victim = cycle.iter().min().unwrap();
        
        let conflicting_transactions = cycle.clone();
        let conflicting_resources = self.get_conflicting_resources(cycle);

        let deadlock_info = DeadlockInfo {
            transaction_id: *victim,
            conflicting_transactions,
            conflicting_resources,
            detected_at: self.env.ledger().timestamp(),
        };

        // Publish deadlock detected event
        EventPublisher::deadlock_detected(&self.env, &deadlock_info);

        Ok(deadlock_info)
    }

    /// Get resources that are causing conflicts in a deadlock cycle
    fn get_conflicting_resources(&self, cycle: &Vec<u64>) -> Vec<String> {
        let current_locks: Vec<(String, u64)> = self.env.storage().instance()
            .get(&RESOURCE_LOCKS)
            .unwrap_or(Vec::new(&self.env));

        let mut conflicting_resources = Vec::new(&self.env);

        // Find resources locked by transactions in the cycle
        for (resource, tx_id) in current_locks {
            if cycle.contains(&tx_id) {
                if !conflicting_resources.contains(&resource) {
                    conflicting_resources.push_back(resource);
                }
            }
        }

        conflicting_resources
    }

    /// Get deadlock prevention suggestions
    pub fn get_deadlock_prevention_suggestions(&self, operations: &Vec<TransactionOperation>) -> Vec<String> {
        let mut suggestions = Vec::new(&self.env);

        // Suggest resource ordering
        suggestions.push_back(String::from_str(&self.env, "Consider acquiring resources in a consistent order across all transactions"));

        // Suggest timeout configuration
        suggestions.push_back(String::from_str(&self.env, "Configure appropriate timeouts to prevent indefinite waiting"));

        // Suggest operation batching
        if operations.len() > 5 {
            suggestions.push_back(String::from_str(&self.env, "Consider breaking down large transactions into smaller batches"));
        }

        // Suggest resource granularity
        for operation in operations {
            if operation.locked_resources.len() > 3 {
                suggestions.push_back(String::from_str(&self.env, "Consider using more granular resource locking"));
                break;
            }
        }

        suggestions
    }
}

/// Dependency graph for deadlock detection
struct DependencyGraph {
    env: Env,
    dependencies: Vec<(u64, u64, String)>, // (from_tx, to_tx, resource)
}

impl DependencyGraph {
    fn new(env: &Env) -> Self {
        Self {
            env: env.clone(),
            dependencies: Vec::new(env),
        }
    }

    fn add_dependency(&mut self, from_tx: u64, to_tx: u64, resource: String) {
        self.dependencies.push_back((from_tx, to_tx, resource));
    }

    fn add_resource(&mut self, _resource: String) {
        // Resource tracking would be implemented here if needed
    }

    fn get_dependencies(&self, transaction: u64) -> Vec<(u64, String)> {
        let mut deps = Vec::new(&self.env);
        
        for (from_tx, to_tx, resource) in &self.dependencies {
            if *from_tx == transaction {
                deps.push_back((*to_tx, resource.clone()));
            }
        }
        
        deps
    }

    fn get_all_transactions(&self) -> Vec<u64> {
        let mut transactions = Vec::new(&self.env);
        
        for (from_tx, to_tx, _) in &self.dependencies {
            if !transactions.contains(from_tx) {
                transactions.push_back(*from_tx);
            }
            if !transactions.contains(to_tx) {
                transactions.push_back(*to_tx);
            }
        }
        
        transactions
    }
}
