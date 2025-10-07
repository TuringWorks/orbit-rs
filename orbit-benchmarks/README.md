# Orbit Benchmarks

This crate contains performance benchmarks for the Orbit distributed database system.

## Note

This crate has been **excluded from the main workspace build** due to WAL (Write-Ahead Log) replay issues in the persistence comparison benchmarks that can cause infinite loops. The benchmarks are still functional but must be run independently.

## Running Benchmarks

To run benchmarks, you need to navigate to this directory and run them directly:

```bash
cd orbit-benchmarks

# Run all tests to ensure everything works
cargo test

# Run specific benchmarks
cargo bench --bench actor_benchmarks
cargo bench --bench leader_election_benchmarks

# WARNING: The persistence_comparison benchmark may hang due to WAL replay issues
# Use with caution or avoid running it
# cargo bench --bench persistence_comparison
```

## Available Benchmarks

### 1. Actor Benchmarks (`actor_benchmarks.rs`)
- Simple placeholder benchmarks for actor system performance

### 2. Leader Election Benchmarks (`leader_election_benchmarks.rs`)
- Benchmarks for Raft leader election performance
- Tests various scenarios including save/load operations and concurrent state management

### 3. Persistence Comparison Benchmarks (`persistence_comparison.rs`) ⚠️
- **WARNING**: These benchmarks have known WAL replay issues that can cause infinite loops
- Compares COW B+ Tree vs RocksDB performance
- Includes comprehensive workload simulations
- **Avoid running unless you understand the WAL replay issue**

## Examples

The crate also includes several examples demonstrating different persistence backends:

```bash
# Run examples
cargo run --example cow_btree_demo
cargo run --example cow_btree_persistence_demo
cargo run --example rocksdb_demo
cargo run --example configurable_backends_demo
```

## Known Issues

### WAL Replay Loop Issue

The persistence comparison benchmarks suffer from a WAL (Write-Ahead Log) replay issue where:

1. Each benchmark iteration creates a new persistence instance
2. The instance replays all existing WAL entries from the temporary directory
3. New operations add more entries to the WAL
4. This creates an ever-growing WAL that takes longer and longer to replay
5. Eventually causing the benchmark to appear to hang

This issue affects both COW B+ Tree and LSM Tree implementations that use WAL for durability.

### Workaround

If you need to run persistence benchmarks:
1. Ensure each benchmark run uses a completely fresh temporary directory
2. Consider running individual benchmark functions rather than the full suite
3. Monitor the benchmark process and kill it if it hangs

## Integration with Main Workspace

The benchmarks can still be run from the main workspace root using:

```bash
# From the workspace root
cd orbit-benchmarks && cargo bench --bench actor_benchmarks
cd orbit-benchmarks && cargo bench --bench leader_election_benchmarks
```

But they are excluded from `cargo build --workspace` and similar commands to prevent build issues.