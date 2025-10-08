# Orbit-RS Consolidated Benchmark Scripts

This directory contains all benchmark-related scripts for Orbit-RS, providing a centralized location for performance testing and analysis.

## 🚀 Quick Start

```bash
# Run safe benchmarks (recommended)
./run_benchmarks.sh -t safe

# Analyze results
./analyze_results.py --results-dir ../results
```

## 📁 Directory Structure

```
orbit-benchmarks/
├── scripts/                    # Consolidated benchmark scripts
│   ├── run_benchmarks.sh      # Master benchmark runner
│   ├── analyze_results.py     # Result analysis tool
│   └── README.md              # This file
├── src/                       # Consolidated benchmark code
│   ├── compute/               # Compute benchmarks (from orbit-compute)
│   ├── performance/           # Performance benchmarks (from orbit-shared)
│   └── persistence/           # Storage benchmarks (original)
├── benches/                   # Criterion benchmark definitions
│   ├── actor_benchmarks.rs
│   ├── leader_election_benchmarks.rs
│   └── persistence_comparison.rs
└── results/                   # Benchmark results (generated)
    ├── *.json                 # Raw benchmark data
    ├── *.log                  # Execution logs
    ├── benchmark_summary.md   # Generated summary
    └── analysis_report.html   # Analysis report
```

## 🔧 Scripts Overview

### `run_benchmarks.sh` - Master Benchmark Runner

Consolidated script that replaces the old `verification/checks/check_benchmarks.sh`.

**Features:**
- ✅ Timeout protection for problematic benchmarks
- ✅ Multiple benchmark types support
- ✅ Configurable output directory
- ✅ Automatic report generation
- ✅ Verbose and quiet modes
- ✅ Error handling and recovery

**Usage:**
```bash
./run_benchmarks.sh [OPTIONS]

OPTIONS:
    -t, --type TYPE         Benchmark type (default: all)
    -d, --duration TIME     Timeout duration (default: 5m)
    -o, --output DIR        Output directory (default: ./results)
    -v, --verbose           Enable verbose output
    -n, --no-report        Disable report generation
    -T, --no-timeout       Disable timeout protection
    -h, --help             Show help

BENCHMARK TYPES:
    all             Run all available benchmarks (safe ones)
    actor           Actor system performance
    leader          Leader election benchmarks  
    persistence     Storage persistence (⚠️ may hang)
    compute         Heterogeneous compute (if available)
    performance     Transaction performance
    safe            Run only safe benchmarks (actor + leader)
```

**Examples:**
```bash
# Basic usage
./run_benchmarks.sh

# Specific benchmark with verbose output
./run_benchmarks.sh -t actor -v

# Persistence with timeout protection
./run_benchmarks.sh -t persistence -d 2m

# Custom output directory
./run_benchmarks.sh -t safe -o ./my-results
```

### `analyze_results.py` - Result Analysis Tool

Python script for analyzing benchmark results and generating reports.

**Features:**
- ✅ Multiple output formats (HTML, JSON, text)
- ✅ Performance trend analysis
- ✅ Regression detection
- ✅ Benchmark comparison
- ✅ Success rate monitoring
- ✅ Interactive HTML reports

**Requirements:**
```bash
# Optional Python packages (for enhanced features)
pip install matplotlib pandas  # For advanced analysis
```

**Usage:**
```bash
./analyze_results.py [OPTIONS]

OPTIONS:
    --results-dir DIR       Results directory (default: ./results)
    --output FILE          Output file (default: benchmark_report.html)
    --format FORMAT        Output format: html, json, text (default: html)
    --compare PATTERN1 PATTERN2  Compare two runs
    --regression-threshold PERCENT   Regression threshold (default: 10.0)

EXAMPLES:
    # Generate HTML report
    ./analyze_results.py
    
    # JSON summary
    ./analyze_results.py --format json --output summary.json
    
    # Compare benchmark runs
    ./analyze_results.py --compare "*actor*" "*leader*"
    
    # Console summary
    ./analyze_results.py --format text
```

## 🎯 Benchmark Types

### Core Benchmarks (Always Available)

#### Actor Benchmarks (`actor`)
- **File**: `benches/actor_benchmarks.rs`
- **Status**: ✅ Safe
- **Focus**: Virtual actor system performance
- **Metrics**: Message throughput, activation latency

#### Leader Election Benchmarks (`leader`)
- **File**: `benches/leader_election_benchmarks.rs`
- **Status**: ✅ Safe
- **Focus**: Raft consensus performance
- **Metrics**: Election timing, state persistence

#### Persistence Benchmarks (`persistence`)
- **File**: `benches/persistence_comparison.rs`
- **Status**: ⚠️ **Known WAL replay issues**
- **Focus**: Storage backend comparison
- **Metrics**: Read/write performance, COW B+ Trees vs RocksDB

### Extended Benchmarks (Centralized)

#### Compute Benchmarks (`compute`)
- **Location**: `src/compute/`
- **Status**: ✅ Available (moved from orbit-compute)
- **Focus**: Heterogeneous compute acceleration
- **Metrics**: CPU SIMD, GPU, Neural Engine performance

#### Performance Benchmarks (`performance`)
- **Location**: `src/performance/`
- **Status**: ✅ Available (moved from orbit-shared)
- **Focus**: Transaction and batch processing
- **Metrics**: Batch processing, connection pools, resource management

## 🔄 Migration from Old Scripts

### Before (Scattered)
```bash
# Old locations
verification/checks/check_benchmarks.sh     # ❌ Removed
orbit-compute/src/benchmarks/               # ❌ Moved
orbit-shared/src/transactions/performance.rs # ❌ Moved

# Old usage
cargo bench --package orbit-benchmarks      # ❌ Fails (excluded)
```

### After (Consolidated)
```bash
# New location
orbit-benchmarks/scripts/                   # ✅ All scripts here

# New usage
cd orbit-benchmarks
./scripts/run_benchmarks.sh                # ✅ Works
cargo bench                                # ✅ Works from benchmark dir
```

## 🚫 CI/CD Exclusion Strategy

### Why Benchmarks Are Excluded
1. **Resource Intensive**: Can consume significant CPU/memory
2. **Time Consuming**: Add 10-30 minutes to build times
3. **WAL Replay Issues**: Some benchmarks have known infinite loop issues
4. **Environment Sensitive**: Results vary across hardware

### Exclusion Implementation
- **Workspace Level**: `exclude = ["orbit-benchmarks"]` in root Cargo.toml
- **CI/CD Level**: Separate manual-only workflow
- **Script Level**: Timeout protection and error handling

### Manual Execution Methods
1. **Local**: `cd orbit-benchmarks && ./scripts/run_benchmarks.sh`
2. **GitHub Actions**: Manual workflow trigger
3. **Development**: Direct cargo bench from benchmark directory

## 📊 Result Analysis

### Generated Files

#### `results/benchmark_summary.md`
- Generated by `run_benchmarks.sh`
- Contains execution summary, timing, success rates
- Markdown format for easy viewing

#### `results/analysis_report.html`
- Generated by `analyze_results.py`
- Interactive HTML report with trend analysis
- Includes regression detection and recommendations

#### `results/*.json` and `results/*.log`
- Raw benchmark output and execution logs
- Used for detailed analysis and debugging

### Performance Baselines

Expected performance on MacBook Pro M2:
- **Actor Benchmarks**: 500k+ messages/second per core
- **Leader Election**: 1-15µs election time
- **Persistence**: 15-50k ops/second (COW B+ Tree)

## ⚠️ Known Issues & Workarounds

### WAL Replay Problem
**Issue**: Persistence benchmarks may hang due to Write-Ahead Log replay loops.

**Workaround**:
```bash
# Use timeout protection (recommended)
./run_benchmarks.sh -t persistence -d 2m

# Or run with timeout manually
timeout 2m cargo bench --bench persistence_comparison
```

### Benchmark Package Not Found
**Issue**: `cargo bench --package orbit-benchmarks` fails from workspace root.

**Solution**: Always run from the benchmark directory:
```bash
cd orbit-benchmarks
cargo bench  # ✅ Works
```

## 🔗 Integration Points

### GitHub Actions
- **Workflow**: `.github/workflows/benchmarks.yml`
- **Trigger**: Manual only (`workflow_dispatch`)
- **Script**: Uses `run_benchmarks.sh` with GitHub input mapping

### Development Workflow
```bash
# Before code changes
cd orbit-benchmarks && ./scripts/run_benchmarks.sh -t safe

# After development
./scripts/analyze_results.py --format text

# Before release
./scripts/run_benchmarks.sh -t all -d 10m
```

### Result Management
- **Local**: Results saved to `orbit-benchmarks/results/`
- **CI**: Uploaded as workflow artifacts (30 day retention)
- **Analysis**: Historical trend analysis across runs

## 📋 Best Practices

### For Developers
1. **Run safe benchmarks regularly** to establish baselines
2. **Use timeout protection** for persistence benchmarks
3. **Analyze trends over time** rather than individual results
4. **Document performance regressions** in GitHub issues

### For CI/CD
1. **Keep benchmarks excluded** from regular builds
2. **Use manual triggers only** for performance testing
3. **Monitor success rates** and investigate failures
4. **Archive results** for historical analysis

### For Performance Analysis
1. **Run multiple iterations** for statistical significance
2. **Control for environmental factors** (load, temperature)
3. **Compare relative performance** rather than absolute
4. **Document baseline expectations** for each benchmark type

## 🔮 Future Enhancements

- **Automated regression detection** with GitHub issue creation
- **Performance dashboard** with historical trends
- **Benchmark result database** for long-term analysis
- **Integration with performance testing frameworks**
- **Cross-platform performance comparison**

---

**📞 Need Help?**
- Check the [Benchmarking Guide](../../docs/development/benchmarking.md)
- Review script help: `./run_benchmarks.sh --help`
- Analyze existing results: `./analyze_results.py --format text`