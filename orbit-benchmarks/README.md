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

## CI/CD Integration

### Exclusion from Regular CI/CD

Benchmarks are **completely excluded** from regular CI/CD pipelines to:
- Prevent accidental execution that could slow down builds
- Avoid WAL replay issues in automated environments
- Keep CI/CD focused on correctness rather than performance

### Manual Benchmark Execution

Benchmarks can be run manually via GitHub Actions:
1. Go to the **Actions** tab in the repository
2. Select the **Benchmarks** workflow
3. Click **Run workflow** 
4. Choose benchmark type and duration

The manual workflow supports:
- **All benchmarks**: Runs all available benchmarks
- **Specific benchmarks**: Run only actor, leader election, or persistence benchmarks
- **Custom duration**: Set timeout to prevent infinite loops
- **Artifact upload**: Save results for analysis

### Local Development

#### Running from Workspace Root
```bash
# From the workspace root
cd orbit-benchmarks && cargo bench --bench actor_benchmarks
cd orbit-benchmarks && cargo bench --bench leader_election_benchmarks
```

#### Running from Benchmarks Directory
```bash
# Navigate to benchmarks directory
cd orbit-benchmarks

# Run all benchmarks (be careful with persistence_comparison)
cargo bench

# Run specific benchmarks
cargo bench --bench actor_benchmarks
cargo bench --bench leader_election_benchmarks
```

#### Integration with Workspace
Benchmarks are excluded from:
- `cargo build --workspace` (to prevent accidental builds)
- `cargo test --workspace` (to avoid test interference) 
- Regular CI/CD pipelines (to keep builds fast)
- Automatic dependency updates (to avoid version conflicts)

But they can still access workspace dependencies through path-based references.
