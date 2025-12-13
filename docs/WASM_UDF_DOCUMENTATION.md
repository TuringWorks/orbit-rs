# WebAssembly (WASM) User-Defined Functions

## Overview

Orbit-RS now supports **WebAssembly (WASM) user-defined functions (UDFs)**, enabling you to write high-performance, sandboxed functions in any language that compiles to WASM.

## Key Features

### 🚀 Language Agnostic

- **Rust**: Compile with `rustc --target wasm32-unknown-unknown`
- **C/C++**: Use Emscripten or clang with WASM target
- **Go**: TinyGo for WASM compilation
- **AssemblyScript**: TypeScript-like syntax for WASM
- **Many others**: Any language with WASM compilation support

### ⚡ High Performance

- **Near-Native Speed**: 10-20% overhead vs native code
- **JIT Compilation**: wasmtime compiles WASM to native code
- **Module Caching**: Compiled modules cached for repeated use
- **No IPC Overhead**: Unlike Python subprocess approach

### 🔒 Security & Sandboxing

- **Memory Isolation**: Cannot access server memory
- **CPU Limits**: Fuel-based instruction counting (default: 1 billion instructions)
- **Time Limits**: Execution timeouts (default: 30 seconds)
- **No System Access**: No file I/O, network, or system calls by default
- **Module Validation**: Binary format verification

### 📊 Resource Management

- **Memory Limit**: 64MB per function (configurable)
- **Fuel Limit**: 1 billion instructions (configurable)
- **Timeout**: 30 seconds (configurable)
- **Cache Size**: 100 compiled modules (configurable)

## Architecture

```text
┌──────────────────────────────────────────┐
│          SQL CREATE FUNCTION             │
│  CREATE FUNCTION add(a INT, b INT)       │
│  RETURNS INT LANGUAGE WASM AS '0x...'    │
└────────────────┬─────────────────────────┘
                 │
                 ▼
┌──────────────────────────────────────────┐
│         WasmUdfHandler                   │
│  • Hex decode WASM binary                │
│  • Validate module                       │
└────────────────┬─────────────────────────┘
                 │
                 ▼
┌──────────────────────────────────────────┐
│         WasmUdfRegistry                  │
│  • Function metadata                     │
│  • Argument validation                   │
└────────────────┬─────────────────────────┘
                 │
                 ▼
┌──────────────────────────────────────────┐
│          WasmRuntime                     │
│  • wasmtime JIT compiler                 │
│  • LRU module cache                      │
│  • Resource limits                       │
└──────────────────────────────────────────┘
```

## Quick Start

### 1. Write a WASM Function (Rust)

```rust
// add.rs
#[no_mangle]
pub extern "C" fn add(a: i32, b: i32) -> i32 {
    a + b
}
```

### 2. Compile to WASM

```bash
rustc --target wasm32-unknown-unknown --crate-type=cdylib -O add.rs
```

This produces `add.wasm`.

### 3. Convert to Hex

```bash
xxd -p add.wasm | tr -d '\n' > add.hex
```

### 4. Register Function via SQL

```sql
CREATE FUNCTION add(a INTEGER, b INTEGER)
RETURNS INTEGER
LANGUAGE WASM
AS '0061736d0100000001070160027f7f017f...';  -- paste hex here
```

### 5. Use the Function

```sql
SELECT add(5, 3);  -- Returns: 8
SELECT id, add(score1, score2) AS total FROM students;
```

## SQL Syntax

```sql
CREATE FUNCTION function_name(param1 TYPE1, param2 TYPE2, ...)
RETURNS RETURN_TYPE
LANGUAGE WASM
AS 'hex_encoded_wasm_binary';
```

### Supported Types

| SQL Type | WASM Type | Notes |
|----------|-----------|-------|
| INTEGER, INT | i32 | 32-bit signed integer |
| BIGINT | i64 | 64-bit signed integer |
| REAL, FLOAT | f32 | 32-bit float |
| DOUBLE PRECISION | f64 | 64-bit float |
| BOOLEAN | i32 | 0 = false, 1 = true |
| TEXT, VARCHAR | (bytes) | Serialized via MessagePack |
| BYTEA | (bytes) | Binary data |
| Arrays, JSON | (bytes) | Serialized via MessagePack |

## Examples

### Example 1: Simple Math (Rust)

```rust
#[no_mangle]
pub extern "C" fn multiply(a: i32, b: i32) -> i32 {
    a * b
}
```

```sql
CREATE FUNCTION multiply(a INTEGER, b INTEGER)
RETURNS INTEGER
LANGUAGE WASM
AS '...hex...';

SELECT multiply(7, 6);  -- 42
```

### Example 2: Factorial (C)

```c
// factorial.c
int factorial(int n) {
    if (n <= 1) return 1;
    return n * factorial(n - 1);
}
```

Compile:

```bash
clang --target=wasm32 --no-standard-libraries \
      -Wl,--export-all -Wl,--no-entry \
      -o factorial.wasm factorial.c
```

### Example 3: String Length (AssemblyScript)

```typescript
// strlen.ts
export function strlen(s: string): i32 {
  return s.length;
}
```

Compile:

```bash
asc strlen.ts --outFile strlen.wasm --optimize
```

## Configuration

WASM runtime can be configured via Rust API:

```rust
use orbit_server::wasm::{WasmConfig, WasmRuntime};

let config = WasmConfig {
    enabled: true,
    max_memory_bytes: 128 * 1024 * 1024,  // 128MB
    timeout: Duration::from_secs(60),       // 60 seconds
    fuel_limit: 2_000_000_000,              // 2 billion instructions
    enable_cache: true,
    cache_size: 200,
    ..Default::default()
};

let runtime = WasmRuntime::new(config)?;
```

## Performance Characteristics

| Metric | Value |
|--------|-------|
| **Compilation Time** | <10ms for small modules |
| **Execution Overhead** | 10-20% vs native |
| **Call Latency** | <1μs for simple functions |
| **Cache Hit Time** | <1μs (LRU cache) |
| **Memory Footprint** | ~1MB per cached module |

## Comparison with Other UDF Systems

| Feature | WASM | Python | Lua |
|---------|------|--------|-----|
| **Performance** | ⭐⭐⭐⭐⭐ (near-native) | ⭐⭐ (interpreted) | ⭐⭐⭐⭐ (JIT) |
| **Language Support** | Any language→WASM | Python only | Lua only |
| **Sandbox Security** | ⭐⭐⭐⭐⭐ (native) | ⭐⭐⭐⭐ (process isolation) | ⭐⭐⭐ (limited) |
| **Startup Overhead** | ⭐⭐⭐⭐⭐ (low) | ⭐⭐ (subprocess) | ⭐⭐⭐⭐⭐ (low) |
| **Memory Safety** | ⭐⭐⭐⭐⭐ (guaranteed) | ⭐⭐⭐ (runtime errors) | ⭐⭐⭐ (runtime errors) |
| **External Libraries** | ⭐⭐ (limited) | ⭐⭐⭐⭐⭐ (numpy, pandas) | ⭐⭐ (limited) |
| **IPC Overhead** | None | High (subprocess) | None |

## Security Best Practices

1. **Disable WASI in production** (default: disabled)
2. **Set appropriate memory limits** based on workload
3. **Use fuel limits** to prevent infinite loops
4. **Set timeouts** for long-running functions
5. **Validate WASM binaries** before deployment
6. **Audit WASM source code** for security issues

## Limitations

1. **No WASI by default**: File I/O and network access disabled for security
2. **Limited complex types**: Arrays and JSON require serialization
3. **No async operations**: WASM functions run synchronously
4. **Module size limit**: 10MB maximum (configurable)
5. **Single-threaded**: Each function call runs in one thread

## Troubleshooting

### Function Not Found

```text
Error: Function not found: my_function
```

**Solution**: Ensure the exported function name matches the CREATE FUNCTION name.

### Module Compilation Failed

```text
Error: Failed to compile WASM module: invalid magic number
```

**Solution**: Verify the hex encoding is correct and the WASM binary is valid.

### Timeout Error

```text
Error: Timeout error: function exceeded 30000ms
```

**Solution**: Increase timeout in configuration or optimize the function.

### Out of Fuel

```text
Error: Out of fuel: function exceeded instruction limit
```

**Solution**: Increase `fuel_limit` or optimize the function for fewer instructions.

## Implementation Details

| Component | Lines of Code | Purpose |
|-----------|---------------|---------|
| `types.rs` | ~300 | Type conversions (SQL ↔ WASM) |
| `config.rs` | ~180 | Configuration and limits |
| `runtime.rs` | ~400 | wasmtime integration |
| `udf_registry.rs` | ~280 | Function management |
| `udf_handler.rs` | ~250 | SQL integration |
| **Total** | **~1,410** | Complete implementation |

## Dependencies

- **wasmtime** v28.0: WASM runtime with JIT compilation
- **lru** v0.12: LRU cache for compiled modules
- **hex**: Hex encoding/decoding for WASM binaries

## Feature Flags

```toml
# Enable WASM UDF support for specific protocols
[features]
wasm-udf = ["wasmtime"]                        # Base WASM support
wasm-postgres = ["wasm-udf", "protocol-postgres"]  # PostgreSQL
wasm-mysql = ["wasm-udf", "protocol-mysql"]        # MySQL
wasm-redis = ["wasm-udf", "protocol-redis"]        # Redis
wasm-all = ["wasm-postgres", "wasm-mysql", "wasm-redis"]
```

## Future Enhancements

- [ ] WASI support (opt-in) for file/network I/O
- [ ] SIMD operations for vectorized computation
- [ ] Streaming I/O for large datasets
- [ ] Async WASM functions
- [ ] Component Model support
- [ ] Multi-threading with WASM threads proposal

## References

- [WebAssembly Official Site](https://webassembly.org/)
- [wasmtime Documentation](https://docs.wasmtime.dev/)
- [Rust WASM Book](https://rustwasm.github.io/docs/book/)
- [AssemblyScript](https://www.assemblyscript.org/)

---

**Last Updated**: December 13, 2025
**Version**: 1.0
**Status**: Production Ready
