# Cognitive Complexity Analysis and Refactoring Plan

## Overview

This document identifies functions with high cognitive complexity (>15) in the orbit-rs codebase and provides refactoring recommendations to improve maintainability and readability.

## Identified High Complexity Functions

### 1. `CommandHandler::handle_command()` - **CRITICAL PRIORITY**

**File**: `orbit-protocols/src/resp/commands.rs` (lines 51-237)
**Estimated Cognitive Complexity**: ~80

**Issues**:

- Massive match statement with 70+ command cases
- Single function handling all Redis protocol commands
- No grouping or modularization of related commands
- Linear complexity growth as new commands are added

**Refactoring Plan**:

```rust
// Break down into command groups with separate handlers
impl CommandHandler {
    pub async fn handle_command(&self, command: RespValue) -> ProtocolResult<RespValue> {
        // Simplified dispatch logic
        let (command_name, args) = self.parse_command(command)?;
        
        match self.get_command_category(&command_name) {
            CommandCategory::Connection => self.handle_connection_commands(&command_name, args).await,
            CommandCategory::String => self.handle_string_commands(&command_name, args).await,
            CommandCategory::Hash => self.handle_hash_commands(&command_name, args).await,
            CommandCategory::List => self.handle_list_commands(&command_name, args).await,
            CommandCategory::Set => self.handle_set_commands(&command_name, args).await,
            CommandCategory::Vector => self.handle_vector_commands(&command_name, args).await,
            CommandCategory::TimeSeries => self.handle_timeseries_commands(&command_name, args).await,
            CommandCategory::Graph => self.handle_graph_commands(&command_name, args).await,
            CommandCategory::GraphRAG => self.handle_graphrag_commands(&command_name, args).await,
            CommandCategory::Server => self.handle_server_commands(&command_name, args).await,
            CommandCategory::Unknown => Err(ProtocolError::UnknownCommand(command_name)),
        }
    }
}
```

### 2. `MultiHopReasoningEngine::find_paths()` - **HIGH PRIORITY**

**File**: `orbit-protocols/src/graphrag/multi_hop_reasoning.rs` (lines 249-398)
**Estimated Cognitive Complexity**: ~35

**Issues**:

- Complex nested control flow with multiple levels of conditions
- Large while loop with nested match statements
- Mix of graph traversal logic with result management
- Multiple nested loops and conditionals

**Refactoring Plan**:

```rust
impl MultiHopReasoningEngine {
    pub async fn find_paths(&mut self, /* params */) -> OrbitResult<Vec<ReasoningPath>> {
        let search_context = self.initialize_search_context(query)?;
        let mut search_state = PathSearchState::new(search_context);
        
        while let Some(current_state) = search_state.next_state() {
            if self.is_target_reached(&current_state, &query.to_entity) {
                search_state.add_found_path(self.create_reasoning_path(&current_state));
                if search_state.has_enough_results() { break; }
                continue;
            }
            
            self.expand_current_state(&mut search_state, current_state, orbit_client.clone(), kg_name, &query).await?;
        }
        
        Ok(search_state.get_results())
    }
    
    // Extract complex nested logic into focused methods
    async fn expand_current_state(/* ... */) -> OrbitResult<()> { /* ... */ }
    fn create_reasoning_path(&self, state: &PathState) -> ReasoningPath { /* ... */ }
    fn is_target_reached(&self, state: &PathState, target: &str) -> bool { /* ... */ }
}
```

### 3. `create_stateful_set()` - **MEDIUM PRIORITY**

**File**: `orbit-operator/src/cluster_controller.rs` (lines 456-685)  
**Estimated Cognitive Complexity**: ~25

**Issues**:

- Extremely long function (230 lines)
- Complex nested struct initialization
- Multiple embedded conditional logic
- Mixed concerns: environment setup + Kubernetes resource creation

**Refactoring Plan**:

```rust
async fn create_stateful_set(/* params */) -> Result<()> {
    let statefulsets: Api<StatefulSet> = Api::namespaced(client.clone(), ns);
    
    let env_vars = build_environment_variables(cluster, name);
    let pod_spec = build_pod_spec(cluster, name, env_vars);
    let statefulset_spec = build_statefulset_spec(cluster, name, pod_spec);
    let statefulset = build_statefulset_resource(cluster, ns, name, statefulset_spec);
    
    deploy_statefulset(&statefulsets, &statefulset, name).await
}

fn build_environment_variables(cluster: &OrbitCluster, name: &str) -> Vec<EnvVar> { /* ... */ }
fn build_pod_spec(cluster: &OrbitCluster, name: &str, env_vars: Vec<EnvVar>) -> PodSpec { /* ... */ }
fn build_statefulset_spec(cluster: &OrbitCluster, name: &str, pod_spec: PodSpec) -> StatefulSetSpec { /* ... */ }
async fn deploy_statefulset(api: &Api<StatefulSet>, statefulset: &StatefulSet, name: &str) -> Result<()> { /* ... */ }
```

### 4. Time Series Functions - **LOW-MEDIUM PRIORITY**

**File**: `orbit-protocols/src/time_series.rs`
**Functions**: `add_sample()`, `increment_by()`, `apply_retention()`
**Estimated Cognitive Complexity**: 15-20 each

**Issues**:

- Multiple nested conditional branches
- Complex policy application logic
- Mixed validation and business logic

**Refactoring Plan**:

- Extract policy handlers into separate methods
- Create validation helper functions
- Separate timestamp handling logic

## Refactoring Priority Order

1. **CRITICAL**: `CommandHandler::handle_command()` - Break into command category handlers
2. **HIGH**: `MultiHopReasoningEngine::find_paths()` - Extract search state management  
3. **MEDIUM**: `create_stateful_set()` - Break into resource building functions
4. **LOW-MEDIUM**: Time series functions - Extract policy and validation logic

## Implementation Guidelines

### For Command Handlers

- Use the Command Pattern to encapsulate each command type
- Create command category traits for shared behavior
- Implement command registry for dynamic dispatch
- Add command validation middleware

### For Graph Algorithms

- Separate search state management from algorithm logic
- Extract path scoring into strategy pattern
- Create separate classes for different search strategies
- Use builder pattern for complex search configurations

### For Kubernetes Resources

- Create builder pattern for complex resource construction
- Extract repeated patterns into helper functions
- Use configuration objects instead of parameter passing
- Implement resource template system

### General Principles

- **Single Responsibility**: Each function should have one clear purpose
- **Extract Method**: When a function has multiple logical sections, extract them
- **Strategy Pattern**: For complex conditional logic based on types/modes
- **Builder Pattern**: For complex object construction
- **Command Pattern**: For action dispatching

## Tooling Setup

### Clippy Configuration

Add to `Cargo.toml`:

```toml
[package.metadata.clippy]
cognitive-complexity-threshold = 15
```

### Pre-commit Hook

```rust
#!/bin/bash
# Check cognitive complexity before commit
cargo clippy -- -W clippy::cognitive-complexity
if [ $? -ne 0 ]; then
    echo " Cognitive complexity check failed"
    echo "Please refactor functions with complexity > 15"
    exit 1
fi
```

## Monitoring and Prevention

1. **CI Integration**: Add cognitive complexity checks to CI pipeline
2. **Code Review Guidelines**: Require complexity analysis for functions >50 lines
3. **Architecture Guidelines**: Document complexity thresholds for different function types
4. **Training**: Team education on cognitive complexity and refactoring techniques

## Success Metrics

- Target: All functions should have cognitive complexity ≤ 15
- Code review efficiency: Faster reviews due to smaller, focused functions  
- Bug reduction: Fewer bugs in refactored code sections
- Maintainability: Easier to add new features without increasing complexity
