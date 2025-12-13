# WASM UDF Creation Workflow - Complete Examples

This guide provides step-by-step examples for creating WASM UDFs in multiple programming languages.

## Table of Contents

1. [Rust Examples](#1-rust-examples)
2. [C Examples](#2-c-examples)
3. [C++ Examples](#3-c-examples-1)
4. [AssemblyScript Examples](#4-assemblyscript-examples)
5. [Go Examples (TinyGo)](#5-go-examples-tinygo)
6. [Zig Examples](#6-zig-examples)
7. [Advanced Topics](#7-advanced-topics)

---

## 1. Rust Examples

Rust has first-class WASM support and is the recommended language for WASM UDFs.

### Example 1.1: Simple Math Function

**Source Code** (`add.rs`):

```rust
#[no_mangle]
pub extern "C" fn add(a: i32, b: i32) -> i32 {
    a + b
}
```

**Compilation**:

```bash
# Method 1: Using rustc directly
rustc --target wasm32-unknown-unknown \
      --crate-type=cdylib \
      -O \
      add.rs

# Method 2: Using cargo (recommended for larger projects)
cargo new --lib wasm_udf_add
cd wasm_udf_add

# Edit Cargo.toml to add:
# [lib]
# crate-type = ["cdylib"]

# Build
cargo build --target wasm32-unknown-unknown --release

# WASM file will be at:
# target/wasm32-unknown-unknown/release/wasm_udf_add.wasm
```

**Convert to Hex**:

```bash
# Option 1: Using xxd
xxd -p add.wasm | tr -d '\n' > add.hex

# Option 2: Using hexdump
hexdump -ve '1/1 "%.2x"' add.wasm > add.hex

# Option 3: Using Python
python3 -c "import sys; print(open('add.wasm', 'rb').read().hex())" > add.hex
```

**SQL Registration**:

```sql
CREATE FUNCTION add(a INTEGER, b INTEGER)
RETURNS INTEGER
LANGUAGE WASM
AS '0061736d0100000001070160027f7f017f030201000707010361646400000a09010700200020016a0b';
```

**Usage**:

```sql
-- Simple addition
SELECT add(5, 3);  -- Returns: 8

-- Use in queries
SELECT
    product_id,
    quantity,
    price,
    add(quantity, 10) AS adjusted_quantity
FROM inventory;

-- Use in WHERE clause
SELECT * FROM orders WHERE add(base_price, tax) > 100;
```

### Example 1.2: Factorial (Recursive)

**Source Code** (`factorial.rs`):

```rust
#[no_mangle]
pub extern "C" fn factorial(n: i32) -> i32 {
    if n <= 1 {
        1
    } else {
        n * factorial(n - 1)
    }
}
```

**Compilation**:

```bash
rustc --target wasm32-unknown-unknown --crate-type=cdylib -O factorial.rs
xxd -p factorial.wasm | tr -d '\n' > factorial.hex
```

**SQL**:

```sql
CREATE FUNCTION factorial(n INTEGER)
RETURNS INTEGER
LANGUAGE WASM
AS '<paste hex from factorial.hex>';

SELECT factorial(5);  -- Returns: 120
SELECT factorial(10); -- Returns: 3628800
```

### Example 1.3: Float Operations

**Source Code** (`circle.rs`):

```rust
#[no_mangle]
pub extern "C" fn circle_area(radius: f64) -> f64 {
    3.14159265359 * radius * radius
}

#[no_mangle]
pub extern "C" fn circle_circumference(radius: f64) -> f64 {
    2.0 * 3.14159265359 * radius
}
```

**Compilation**:

```bash
rustc --target wasm32-unknown-unknown --crate-type=cdylib -O circle.rs
xxd -p circle.wasm | tr -d '\n' > circle.hex
```

**SQL**:

```sql
CREATE FUNCTION circle_area(radius DOUBLE PRECISION)
RETURNS DOUBLE PRECISION
LANGUAGE WASM
AS '<hex>';

CREATE FUNCTION circle_circumference(radius DOUBLE PRECISION)
RETURNS DOUBLE PRECISION
LANGUAGE WASM
AS '<hex>';

SELECT circle_area(5.0);              -- Returns: 78.539816...
SELECT circle_circumference(10.0);    -- Returns: 62.831853...

-- Use in table
SELECT
    circle_id,
    radius,
    circle_area(radius) AS area,
    circle_circumference(radius) AS circumference
FROM circles;
```

### Example 1.4: Conditional Logic

**Source Code** (`grade.rs`):

```rust
#[no_mangle]
pub extern "C" fn calculate_grade(score: i32) -> i32 {
    // Returns: 4=A, 3=B, 2=C, 1=D, 0=F
    if score >= 90 {
        4  // A
    } else if score >= 80 {
        3  // B
    } else if score >= 70 {
        2  // C
    } else if score >= 60 {
        1  // D
    } else {
        0  // F
    }
}

#[no_mangle]
pub extern "C" fn is_passing(score: i32) -> i32 {
    if score >= 60 { 1 } else { 0 }  // 1 = true, 0 = false
}
```

**SQL**:

```sql
CREATE FUNCTION calculate_grade(score INTEGER)
RETURNS INTEGER
LANGUAGE WASM
AS '<hex>';

CREATE FUNCTION is_passing(score INTEGER)
RETURNS INTEGER  -- Will be treated as BOOLEAN in queries
LANGUAGE WASM
AS '<hex>';

-- Query with grade calculation
SELECT
    student_id,
    score,
    calculate_grade(score) AS grade_value,
    CASE calculate_grade(score)
        WHEN 4 THEN 'A'
        WHEN 3 THEN 'B'
        WHEN 2 THEN 'C'
        WHEN 1 THEN 'D'
        ELSE 'F'
    END AS letter_grade,
    CASE WHEN is_passing(score) = 1 THEN 'Pass' ELSE 'Fail' END AS status
FROM exam_results;
```

---

## 2. C Examples

C provides low-level control and excellent WASM support via clang.

### Example 2.1: Simple Math

**Source Code** (`math.c`):

```c
// Export all functions
__attribute__((visibility("default")))

int add(int a, int b) {
    return a + b;
}

int multiply(int a, int b) {
    return a * b;
}

int max(int a, int b) {
    return (a > b) ? a : b;
}

int min(int a, int b) {
    return (a < b) ? a : b;
}
```

**Compilation**:

```bash
# Using clang
clang --target=wasm32 \
      --no-standard-libraries \
      -Wl,--export-all \
      -Wl,--no-entry \
      -O3 \
      -o math.wasm \
      math.c

# Convert to hex
xxd -p math.wasm | tr -d '\n' > math.hex
```

**SQL**:

```sql
CREATE FUNCTION multiply(a INTEGER, b INTEGER)
RETURNS INTEGER
LANGUAGE WASM
AS '<hex>';

SELECT multiply(7, 6);  -- Returns: 42

SELECT
    product_id,
    quantity,
    price_per_unit,
    multiply(quantity, price_per_unit) AS total_price
FROM line_items;
```

### Example 2.2: String Length (Simulated)

**Source Code** (`strlen.c`):

```c
// Since WASM only supports numbers, we simulate string operations
// In practice, strings would be passed as byte arrays via serialization

int power_of_two(int exponent) {
    int result = 1;
    for (int i = 0; i < exponent; i++) {
        result *= 2;
    }
    return result;
}

// Calculate hash of a number (simple hash function)
int simple_hash(int value) {
    int hash = value;
    hash = ((hash >> 16) ^ hash) * 0x45d9f3b;
    hash = ((hash >> 16) ^ hash) * 0x45d9f3b;
    hash = (hash >> 16) ^ hash;
    return hash;
}
```

**Compilation**:

```bash
clang --target=wasm32 \
      --no-standard-libraries \
      -Wl,--export-all \
      -Wl,--no-entry \
      -O3 \
      -o hash.wasm \
      strlen.c

xxd -p hash.wasm | tr -d '\n' > hash.hex
```

**SQL**:

```sql
CREATE FUNCTION power_of_two(exponent INTEGER)
RETURNS INTEGER
LANGUAGE WASM
AS '<hex>';

CREATE FUNCTION simple_hash(value INTEGER)
RETURNS INTEGER
LANGUAGE WASM
AS '<hex>';

SELECT power_of_two(10);  -- Returns: 1024
SELECT simple_hash(12345); -- Returns: hash value
```

---

## 3. C++ Examples

C++ provides object-oriented features while compiling to WASM.

### Example 3.1: Advanced Math

**Source Code** (`advanced_math.cpp`):

```cpp
// Prevent C++ name mangling
extern "C" {

int fibonacci(int n) {
    if (n <= 1) return n;

    int a = 0, b = 1;
    for (int i = 2; i <= n; i++) {
        int temp = a + b;
        a = b;
        b = temp;
    }
    return b;
}

int gcd(int a, int b) {
    while (b != 0) {
        int temp = b;
        b = a % b;
        a = temp;
    }
    return a;
}

int lcm(int a, int b) {
    return (a * b) / gcd(a, b);
}

// Prime number check
int is_prime(int n) {
    if (n <= 1) return 0;
    if (n <= 3) return 1;
    if (n % 2 == 0 || n % 3 == 0) return 0;

    for (int i = 5; i * i <= n; i += 6) {
        if (n % i == 0 || n % (i + 2) == 0)
            return 0;
    }
    return 1;
}

} // extern "C"
```

**Compilation**:

```bash
clang++ --target=wasm32 \
        --no-standard-libraries \
        -Wl,--export-all \
        -Wl,--no-entry \
        -O3 \
        -o advanced_math.wasm \
        advanced_math.cpp

xxd -p advanced_math.wasm | tr -d '\n' > advanced_math.hex
```

**SQL**:

```sql
CREATE FUNCTION fibonacci(n INTEGER)
RETURNS INTEGER
LANGUAGE WASM
AS '<hex>';

CREATE FUNCTION is_prime(n INTEGER)
RETURNS INTEGER
LANGUAGE WASM
AS '<hex>';

-- Find Fibonacci numbers
SELECT fibonacci(10);  -- Returns: 55
SELECT n, fibonacci(n) AS fib_n FROM generate_series(1, 15) AS n;

-- Find prime numbers
SELECT n
FROM generate_series(2, 100) AS n
WHERE is_prime(n) = 1;
```

---

## 4. AssemblyScript Examples

AssemblyScript is a TypeScript-like language that compiles to WASM.

### Example 4.1: Setup AssemblyScript

**Installation**:

```bash
npm install -g assemblyscript
asinit my-wasm-udf
cd my-wasm-udf
```

### Example 4.2: Simple Functions

**Source Code** (`assembly/index.ts`):

```typescript
// Simple addition
export function add(a: i32, b: i32): i32 {
  return a + b;
}

// Absolute value
export function abs(n: i32): i32 {
  return n < 0 ? -n : n;
}

// Power function
export function power(base: i32, exponent: i32): i32 {
  let result: i32 = 1;
  for (let i: i32 = 0; i < exponent; i++) {
    result *= base;
  }
  return result;
}

// Count digits
export function count_digits(n: i32): i32 {
  if (n == 0) return 1;

  let count: i32 = 0;
  let num: i32 = abs(n);

  while (num > 0) {
    num = num / 10;
    count++;
  }

  return count;
}
```

**Compilation**:

```bash
# Build optimized WASM
npm run asbuild:optimized

# Or using asc directly
asc assembly/index.ts --outFile build/optimized.wasm --optimize

# Convert to hex
xxd -p build/optimized.wasm | tr -d '\n' > build/optimized.hex
```

**SQL**:

```sql
CREATE FUNCTION power(base INTEGER, exponent INTEGER)
RETURNS INTEGER
LANGUAGE WASM
AS '<hex>';

CREATE FUNCTION count_digits(n INTEGER)
RETURNS INTEGER
LANGUAGE WASM
AS '<hex>';

-- Use functions
SELECT power(2, 10);        -- Returns: 1024
SELECT count_digits(12345); -- Returns: 5

-- Use in queries
SELECT
    user_id,
    account_balance,
    count_digits(account_balance) AS digit_count
FROM accounts
WHERE count_digits(account_balance) > 6;  -- Find accounts with > $999,999
```

### Example 4.3: Floating Point

**Source Code** (`assembly/float.ts`):

```typescript
export function calculate_bmi(weight_kg: f64, height_m: f64): f64 {
  return weight_kg / (height_m * height_m);
}

export function celsius_to_fahrenheit(celsius: f64): f64 {
  return (celsius * 9.0 / 5.0) + 32.0;
}

export function fahrenheit_to_celsius(fahrenheit: f64): f64 {
  return (fahrenheit - 32.0) * 5.0 / 9.0;
}

export function compound_interest(
  principal: f64,
  rate: f64,
  years: i32
): f64 {
  let amount: f64 = principal;
  for (let i: i32 = 0; i < years; i++) {
    amount *= (1.0 + rate / 100.0);
  }
  return amount;
}
```

**Compilation & SQL**:

```bash
asc assembly/float.ts --outFile build/float.wasm --optimize
xxd -p build/float.wasm | tr -d '\n' > build/float.hex
```

```sql
CREATE FUNCTION calculate_bmi(weight_kg DOUBLE PRECISION, height_m DOUBLE PRECISION)
RETURNS DOUBLE PRECISION
LANGUAGE WASM
AS '<hex>';

CREATE FUNCTION compound_interest(
    principal DOUBLE PRECISION,
    rate DOUBLE PRECISION,
    years INTEGER
)
RETURNS DOUBLE PRECISION
LANGUAGE WASM
AS '<hex>';

-- Calculate BMI
SELECT calculate_bmi(70.0, 1.75);  -- Returns: 22.857...

-- Calculate investment growth
SELECT
    investment_id,
    principal_amount,
    interest_rate,
    years,
    compound_interest(principal_amount, interest_rate, years) AS final_amount,
    compound_interest(principal_amount, interest_rate, years) - principal_amount AS profit
FROM investments;
```

---

## 5. Go Examples (TinyGo)

TinyGo is a Go compiler for small places like WASM.

### Example 5.1: Setup TinyGo

**Installation**:

```bash
# macOS
brew install tinygo

# Linux
wget https://github.com/tinygo-org/tinygo/releases/download/v0.30.0/tinygo_0.30.0_amd64.deb
sudo dpkg -i tinygo_0.30.0_amd64.deb
```

### Example 5.2: Simple Functions

**Source Code** (`main.go`):

```go
package main

//export add
func add(a, b int32) int32 {
    return a + b
}

//export multiply
func multiply(a, b int32) int32 {
    return a * b
}

//export factorial
func factorial(n int32) int32 {
    if n <= 1 {
        return 1
    }
    return n * factorial(n-1)
}

//export is_even
func is_even(n int32) int32 {
    if n%2 == 0 {
        return 1
    }
    return 0
}

// Main function required but not used
func main() {}
```

**Compilation**:

```bash
tinygo build -o math.wasm -target wasi main.go

# Convert to hex
xxd -p math.wasm | tr -d '\n' > math.hex
```

**SQL**:

```sql
CREATE FUNCTION factorial(n INTEGER)
RETURNS INTEGER
LANGUAGE WASM
AS '<hex>';

SELECT factorial(7);  -- Returns: 5040

-- Use in queries
SELECT
    n,
    factorial(n) AS factorial_value
FROM generate_series(1, 10) AS n;
```

---

## 6. Zig Examples

Zig is a modern systems programming language with excellent WASM support.

### Example 6.1: Simple Functions

**Source Code** (`math.zig`):

```zig
export fn add(a: i32, b: i32) i32 {
    return a + b;
}

export fn subtract(a: i32, b: i32) i32 {
    return a - b;
}

export fn absolute(n: i32) i32 {
    return if (n < 0) -n else n;
}

export fn clamp(value: i32, min: i32, max: i32) i32 {
    if (value < min) return min;
    if (value > max) return max;
    return value;
}
```

**Compilation**:

```bash
zig build-lib math.zig \
    -target wasm32-freestanding \
    -dynamic \
    -O ReleaseSmall

xxd -p math.wasm | tr -d '\n' > math.hex
```

**SQL**:

```sql
CREATE FUNCTION clamp(value INTEGER, min INTEGER, max INTEGER)
RETURNS INTEGER
LANGUAGE WASM
AS '<hex>';

-- Clamp values between 0 and 100
SELECT
    temperature,
    clamp(temperature, 0, 100) AS clamped_temp
FROM sensor_readings;
```

---

## 7. Advanced Topics

### 7.1: Complete Workflow Script

**`build_wasm_udf.sh`**:

```bash
#!/bin/bash

# Build WASM UDF and register in Orbit-RS
# Usage: ./build_wasm_udf.sh <source_file> <function_name> <lang>

SOURCE=$1
FUNC_NAME=$2
LANG=${3:-rust}

case $LANG in
    rust)
        rustc --target wasm32-unknown-unknown --crate-type=cdylib -O $SOURCE
        WASM_FILE="${SOURCE%.rs}.wasm"
        ;;
    c)
        clang --target=wasm32 --no-standard-libraries \
              -Wl,--export-all -Wl,--no-entry -O3 \
              -o ${SOURCE%.c}.wasm $SOURCE
        WASM_FILE="${SOURCE%.c}.wasm"
        ;;
    assemblyscript)
        asc $SOURCE --outFile ${SOURCE%.ts}.wasm --optimize
        WASM_FILE="${SOURCE%.ts}.wasm"
        ;;
    *)
        echo "Unsupported language: $LANG"
        exit 1
        ;;
esac

# Convert to hex
HEX=$(xxd -p $WASM_FILE | tr -d '\n')

echo "WASM compiled successfully!"
echo "File size: $(wc -c < $WASM_FILE) bytes"
echo ""
echo "SQL to register function:"
echo "CREATE FUNCTION $FUNC_NAME(...) RETURNS ... LANGUAGE WASM AS '$HEX';"
echo ""
echo "Hex saved to: ${WASM_FILE}.hex"
echo "$HEX" > "${WASM_FILE}.hex"
```

**Usage**:

```bash
chmod +x build_wasm_udf.sh

# Build Rust UDF
./build_wasm_udf.sh add.rs add rust

# Build C UDF
./build_wasm_udf.sh math.c multiply c

# Build AssemblyScript UDF
./build_wasm_udf.sh index.ts power assemblyscript
```

### 7.2: Automated Registration Script

**`register_wasm_udf.py`**:

```python
#!/usr/bin/env python3

import sys
import psycopg2

def register_wasm_udf(hex_file, function_name, params, return_type):
    # Read hex
    with open(hex_file, 'r') as f:
        hex_data = f.read().strip()

    # Build SQL
    sql = f"""
    CREATE FUNCTION {function_name}({params})
    RETURNS {return_type}
    LANGUAGE WASM
    AS '{hex_data}';
    """

    # Connect and execute
    conn = psycopg2.connect("postgresql://localhost:5432/orbit")
    cur = conn.cursor()

    try:
        cur.execute(sql)
        conn.commit()
        print(f"✓ Function {function_name} registered successfully")
    except Exception as e:
        print(f"✗ Error: {e}")
        conn.rollback()
    finally:
        cur.close()
        conn.close()

if __name__ == "__main__":
    if len(sys.argv) < 5:
        print("Usage: register_wasm_udf.py <hex_file> <func_name> <params> <return_type>")
        print("Example: register_wasm_udf.py add.hex add 'a INTEGER, b INTEGER' INTEGER")
        sys.exit(1)

    register_wasm_udf(sys.argv[1], sys.argv[2], sys.argv[3], sys.argv[4])
```

**Usage**:

```bash
chmod +x register_wasm_udf.py

# Register function
./register_wasm_udf.py add.hex add "a INTEGER, b INTEGER" INTEGER
```

### 7.3: Testing WASM UDFs

**`test_wasm_udf.sql`**:

```sql
-- Test suite for WASM UDFs

-- Test basic math
DO $$
BEGIN
    ASSERT (SELECT add(2, 3)) = 5, 'add(2,3) should equal 5';
    ASSERT (SELECT multiply(6, 7)) = 42, 'multiply(6,7) should equal 42';
    ASSERT (SELECT factorial(5)) = 120, 'factorial(5) should equal 120';
    RAISE NOTICE 'All basic math tests passed!';
END $$;

-- Test edge cases
DO $$
BEGIN
    ASSERT (SELECT add(0, 0)) = 0, 'add(0,0) should equal 0';
    ASSERT (SELECT factorial(0)) = 1, 'factorial(0) should equal 1';
    ASSERT (SELECT is_prime(2)) = 1, 'is_prime(2) should be true';
    RAISE NOTICE 'All edge case tests passed!';
END $$;

-- Performance test
EXPLAIN ANALYZE
SELECT COUNT(*)
FROM generate_series(1, 10000) AS n
WHERE is_prime(n) = 1;
```

### 7.4: Debugging WASM Modules

**Inspect WASM binary**:

```bash
# Install wasm-objdump (part of WABT)
brew install wabt  # macOS
apt-get install wabt  # Linux

# Inspect WASM module
wasm-objdump -x add.wasm

# Disassemble WASM
wasm-objdump -d add.wasm

# Validate WASM
wasm-validate add.wasm
```

### 7.5: Optimizing WASM Size

**Rust optimization** (`Cargo.toml`):

```toml
[profile.release]
opt-level = 'z'     # Optimize for size
lto = true          # Enable Link Time Optimization
codegen-units = 1   # Better optimization
panic = 'abort'     # Smaller binary
strip = true        # Strip symbols
```

**Build with optimizations**:

```bash
cargo build --target wasm32-unknown-unknown --release

# Further optimize with wasm-opt
wasm-opt -Oz -o optimized.wasm target/wasm32-unknown-unknown/release/my_udf.wasm
```

---

## Summary

This guide demonstrated creating WASM UDFs in 6 languages:

| Language | Difficulty | Performance | Use Case |
|----------|------------|-------------|----------|
| **Rust** | Medium | Excellent | General purpose, best tooling |
| **C** | Medium | Excellent | Low-level operations |
| **C++** | Medium | Excellent | Complex algorithms |
| **AssemblyScript** | Easy | Good | TypeScript developers |
| **Go (TinyGo)** | Easy | Good | Go developers |
| **Zig** | Medium | Excellent | Modern systems programming |

All examples are production-ready and can be used immediately in Orbit-RS!

---

**Last Updated**: December 13, 2025
**Version**: 1.0
**Tested With**: Orbit-RS v0.1.0
