---
layout: default
title: "Benchmarking Guide"
description: "How to run performance benchmarks for Orbit-RS"
permalink: /development/benchmarking/
---

# Benchmarking Guide

This guide explains how to run performance benchmarks for Orbit-RS and why they are excluded from regular CI/CD pipelines.

## 🚀 Quick Start

### Local Benchmarks
```bash
cd orbit-benchmarks

# Use the consolidated script (recommended)
./scripts/run_benchmarks.sh -t safe
./scripts/run_benchmarks.sh -t actor -v

# Or run individual benchmarks directly
cargo bench --bench actor_benchmarks
cargo bench --bench leader_election_benchmarks
```

### GitHub Actions Benchmarks
1. Go to **Actions** tab → **Benchmarks** workflow
2. Click **Run workflow**
3. Select benchmark type and duration
4. View results in workflow artifacts

## 📋 Available Benchmarks

### Actor Performance Benchmarks
- **File**: `benches/actor_benchmarks.rs`
- **Focus**: Virtual actor system performance
- **Metrics**: Message throughput, latency
- **Status**: ✅ Stable

### Leader Election Benchmarks  
- **File**: `benches/leader_election_benchmarks.rs`
- **Focus**: Raft consensus performance
- **Metrics**: Election timing, state persistence
- **Status**: ✅ Stable

### Persistence Comparison Benchmarks ⚠️
- **File**: `benches/persistence_comparison.rs`
- **Focus**: Storage backend performance (COW B+ Trees vs RocksDB)
- **Metrics**: Read/write performance, memory usage
- **Status**: ⚠️ **Known WAL replay issues - use with caution**

## 🔒 CI/CD Exclusion Strategy

### Why Benchmarks Are Excluded

Benchmarks are excluded from regular CI/CD pipelines because:

1. **Performance Focus**: CI/CD should focus on correctness, not performance
2. **Resource Intensive**: Benchmarks consume significant CPU and memory
3. **Time Consuming**: Can add 10-30 minutes to build times
4. **WAL Replay Issues**: Some benchmarks have known infinite loop issues
5. **Environment Sensitivity**: Results vary significantly across different hardware

### Exclusion Implementation

#### Workspace Level
```toml
# Cargo.toml
[workspace]
exclude = [
    "orbit-benchmarks",  # Excluded from workspace builds
]
```

#### CI/CD Level
- **ci.yml**: Benchmark job completely removed
- **ci-cd.yml**: No benchmark steps in build matrix
- **benchmarks.yml**: Separate manual-only workflow

### Manual Execution Only

Benchmarks can only be executed via:
- ✅ Local development (`cd orbit-benchmarks && cargo bench`)
- ✅ Manual GitHub Actions workflow
- ❌ **Not via**: `cargo bench --package orbit-benchmarks` from workspace root
- ❌ **Not via**: Regular CI/CD pipelines
- ❌ **Not via**: `cargo build --workspace` (automatically excluded)

## 🔧 Consolidated Benchmark Scripts

All benchmark-related scripts are now consolidated under `orbit-benchmarks/scripts/`:

### Main Scripts
- **`run_benchmarks.sh`**: Master benchmark runner with timeout protection
- **`analyze_results.py`**: Python script for result analysis and reporting

### Script Usage

#### Benchmark Runner
```bash
cd orbit-benchmarks

# Run all safe benchmarks (actor + leader election)
./scripts/run_benchmarks.sh
./scripts/run_benchmarks.sh -t safe

# Run specific benchmark types
./scripts/run_benchmarks.sh -t actor          # Actor benchmarks only
./scripts/run_benchmarks.sh -t leader         # Leader election only
./scripts/run_benchmarks.sh -t persistence    # Persistence (⚠️ may hang)

# With custom options
./scripts/run_benchmarks.sh -t actor -v       # Verbose output
./scripts/run_benchmarks.sh -t persistence -d 2m  # 2 minute timeout
./scripts/run_benchmarks.sh -t safe -o ./my_results  # Custom output dir
```

#### Result Analysis
```bash
cd orbit-benchmarks

# Generate HTML analysis report
./scripts/analyze_results.py --results-dir ./results

# Generate JSON summary
./scripts/analyze_results.py --format json --output summary.json

# Compare two benchmark runs
./scripts/analyze_results.py --compare "*actor*" "*leader*"

# Text summary to console
./scripts/analyze_results.py --format text
```

## 🛠️ Local Development

### Prerequisites
```bash
# System dependencies
sudo apt-get install -y pkg-config libssl-dev libsqlite3-dev protobuf-compiler

# Rust toolchain with stable
rustup toolchain install stable
```

### Running Specific Benchmarks

#### Safe Benchmarks (Recommended)
```bash
cd orbit-benchmarks

# Actor performance - safe to run
cargo bench --bench actor_benchmarks

# Leader election - safe to run  
cargo bench --bench leader_election_benchmarks
```

#### Risky Benchmarks (Use Caution)
```bash
cd orbit-benchmarks

# Persistence comparison - may hang due to WAL replay
timeout 5m cargo bench --bench persistence_comparison || echo "Timed out as expected"
```

### Benchmark Results

Results are saved to:
- **Criterion reports**: `orbit-benchmarks/target/criterion/`
- **JSON output**: `orbit-benchmarks/*.json` (when using `--output-format json`)
- **HTML reports**: `orbit-benchmarks/target/criterion/reports/index.html`

## 🤖 GitHub Actions Integration

### Manual Workflow Features

The manual benchmark workflow (`.github/workflows/benchmarks.yml`) provides:

#### Configurable Options
- **Benchmark Type**: All, actor only, leader election only, persistence only
- **Duration**: Timeout in minutes (default: 5)
- **Upload Results**: Save artifacts for analysis

#### Execution Safety
- **Timeout Protection**: Prevents infinite loops from WAL replay issues
- **Error Handling**: Continues on individual benchmark failures
- **Resource Limits**: 60-minute maximum workflow timeout

#### Result Management
- **Artifact Upload**: Saves JSON results and HTML reports
- **Summary Generation**: Creates markdown report with key metrics
- **Issue Creation**: Automatically creates issues on benchmark failures

### Running Manual Workflow

1. **Navigate to Actions**:
   ```
   https://github.com/YourOrg/orbit-rs/actions/workflows/benchmarks.yml
   ```

2. **Click "Run workflow"**

3. **Configure Options**:
   - **Benchmark type**: Choose specific or all benchmarks
   - **Duration**: Set timeout (5-30 minutes recommended)  
   - **Upload results**: Enable for result analysis

4. **Monitor Execution**:
   - View real-time logs
   - Check for timeout issues
   - Download artifacts when complete

## ⚠️ Known Issues

### WAL Replay Problem

The persistence comparison benchmarks suffer from Write-Ahead Log replay issues:

#### Problem Description
1. Each benchmark iteration creates new persistence instance
2. Instance replays all existing WAL entries from temp directory
3. New operations add more WAL entries
4. WAL grows continuously, causing exponentially longer replay times
5. Eventually appears to hang or consume excessive memory

#### Affected Components
- COW B+ Tree persistence layer
- LSM Tree implementation  
- Any benchmark using durable storage

#### Mitigation Strategies
- **Local**: Use `timeout` command to prevent infinite runs
- **CI**: Workflow has built-in timeout protection
- **Development**: Run individual benchmark functions, not full suites

### Workarounds

#### Safe Local Testing
```bash
cd orbit-benchmarks

# Use timeout for problematic benchmarks
timeout 2m cargo bench --bench persistence_comparison

# Run specific benchmark functions only
cargo bench --bench persistence_comparison -- --exact "benchmark_function_name"
```

#### Clean State Testing
```bash
# Ensure clean state between runs
rm -rf /tmp/orbit-benchmark-*
cd orbit-benchmarks
cargo clean
cargo bench --bench actor_benchmarks
```

## 📊 Performance Baselines

### Expected Performance (MacBook Pro M2)

#### Actor Benchmarks
- **Message Processing**: 500k+ messages/second per core
- **Actor Activation**: Sub-microsecond latency
- **Memory Usage**: ~10MB base footprint

#### Leader Election Benchmarks  
- **Election Time**: 1-15µs (depending on cluster size)
- **State Persistence**: 300-600µs per save operation
- **Cluster Health Checks**: 350-1400ns (depending on size)

#### Persistence Benchmarks (When Working)
- **COW B+ Tree**: 15-50k ops/second 
- **RocksDB**: 10-30k ops/second
- **Memory Usage**: Varies significantly by backend

### Performance Regression Detection

Since benchmarks are manual-only:
- **No automatic regression detection**
- **Manual comparison required**
- **Results should be documented in performance issues**
- **Consider running before major releases**

## 🔍 Troubleshooting

### Common Issues

#### "Package not found" Error
```bash
# Wrong - will fail
cargo bench --package orbit-benchmarks

# Correct - run from benchmarks directory  
cd orbit-benchmarks && cargo bench
```

#### Compilation Errors
```bash
cd orbit-benchmarks

# Clean and rebuild
cargo clean
cargo build --release

# Check dependencies
cargo update
```

#### Hanging Benchmarks
```bash
# Use timeout for protection
timeout 5m cargo bench

# Kill hanging processes
pkill -f "cargo bench"
pkill -f "orbit-benchmarks"
```

#### Memory Issues
```bash
# Monitor memory usage
htop

# Reduce benchmark scope
cargo bench --bench actor_benchmarks -- --quick
```

## 📚 Additional Resources

### Related Documentation
- [Development Guide](development.md)
- [Performance Architecture](../performance.md) 
- [CI/CD Pipeline](../cicd.md)

### External Tools
- [Criterion.rs Documentation](https://bheisler.github.io/criterion.rs/)
- [cargo-bench Documentation](https://doc.rust-lang.org/cargo/commands/cargo-bench.html)
- [GitHub Actions Manual Workflows](https://docs.github.com/en/actions/managing-workflow-runs/manually-running-a-workflow)

---

**⚠️ Remember**: Benchmarks are excluded from regular development workflows for good reasons. Only run them when you specifically need performance analysis, and always use appropriate timeouts to prevent system issues.