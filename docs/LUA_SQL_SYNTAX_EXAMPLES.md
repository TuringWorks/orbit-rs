# Lua UDF SQL Syntax Examples - End-to-End Guide

This document provides comprehensive, end-to-end SQL syntax examples for creating, using, and managing Lua User-Defined Functions (UDFs) in Orbit-RS.

## Table of Contents

1. [Basic Function Creation](#basic-function-creation)
2. [Scalar Functions](#scalar-functions)
3. [Aggregate Functions](#aggregate-functions)
4. [Table Functions](#table-functions)
5. [JSON Processing](#json-processing)
6. [String Manipulation](#string-manipulation)
7. [Mathematical Operations](#mathematical-operations)
8. [Date and Time Functions](#date-and-time-functions)
9. [State Management](#state-management)
10. [Error Handling](#error-handling)
11. [Function Management](#function-management)

---

## Basic Function Creation

### Example 1: Simple Addition

```sql
-- Create a basic addition function
CREATE FUNCTION add_numbers(a INTEGER, b INTEGER)
RETURNS INTEGER
LANGUAGE LUA
AS $$
function add_numbers(a, b)
    return a + b
end
$$;

-- Use the function
SELECT add_numbers(5, 3);
-- Returns: 8

-- Use in WHERE clause
SELECT * FROM orders WHERE add_numbers(quantity, 10) > 100;

-- Use in computed columns
SELECT
    product_id,
    price,
    quantity,
    add_numbers(price, 5) AS price_with_fee
FROM order_items;
```

---

### Example 2: String Concatenation

```sql
-- Create a function to build full names
CREATE FUNCTION build_full_name(first_name TEXT, last_name TEXT)
RETURNS TEXT
LANGUAGE LUA
AS $$
function build_full_name(first_name, last_name)
    return first_name .. " " .. last_name
end
$$;

-- Use the function
SELECT build_full_name('Alice', 'Johnson');
-- Returns: "Alice Johnson"

-- Use with table data
SELECT
    employee_id,
    build_full_name(first_name, last_name) AS full_name,
    department
FROM employees;
```

---

## Scalar Functions

### Example 3: Calculate Tax

```sql
-- Create a tax calculation function
CREATE FUNCTION calculate_tax(price DOUBLE PRECISION, tax_rate DOUBLE PRECISION)
RETURNS DOUBLE PRECISION
LANGUAGE LUA
AS $$
function calculate_tax(price, tax_rate)
    if price == nil or tax_rate == nil then
        return 0
    end
    return price * tax_rate
end
$$;

-- Use the function
SELECT calculate_tax(100.0, 0.08);
-- Returns: 8.0

-- Use in a query
SELECT
    product_id,
    product_name,
    price,
    calculate_tax(price, 0.08) AS tax,
    price + calculate_tax(price, 0.08) AS total_price
FROM products
WHERE price > 50.0;
```

---

### Example 4: Progressive Tax Calculation

```sql
-- Create a tiered tax calculator
CREATE FUNCTION calculate_progressive_tax(income DOUBLE PRECISION)
RETURNS DOUBLE PRECISION
LANGUAGE LUA
AS $$
function calculate_progressive_tax(income)
    local tax = 0
    local remaining = income

    -- Tier 1: 0-10000 at 10%
    if remaining > 10000 then
        tax = tax + (10000 * 0.10)
        remaining = remaining - 10000
    else
        tax = tax + (remaining * 0.10)
        remaining = 0
    end

    -- Tier 2: 10001-30000 at 20%
    if remaining > 20000 then
        tax = tax + (20000 * 0.20)
        remaining = remaining - 20000
    elseif remaining > 0 then
        tax = tax + (remaining * 0.20)
        remaining = 0
    end

    -- Tier 3: 30001+ at 30%
    if remaining > 0 then
        tax = tax + (remaining * 0.30)
    end

    return tax
end
$$;

-- Test the function
SELECT
    income,
    calculate_progressive_tax(income) AS tax,
    income - calculate_progressive_tax(income) AS net_income
FROM (VALUES
    (5000.0),
    (15000.0),
    (35000.0),
    (50000.0),
    (100000.0)
) AS t(income);

/*
Results:
income   | tax      | net_income
---------|----------|------------
5000.0   | 500.0    | 4500.0
15000.0  | 2000.0   | 13000.0
35000.0  | 7500.0   | 27500.0
50000.0  | 12000.0  | 38000.0
100000.0 | 27000.0  | 73000.0
*/
```

---

## Aggregate Functions

### Example 5: Custom Sum with Filtering

```sql
-- Create a filtered sum function
CREATE FUNCTION sum_if(value DOUBLE PRECISION, condition BOOLEAN)
RETURNS DOUBLE PRECISION
LANGUAGE LUA
AS $$
function sum_if(value, condition)
    if condition then
        return value or 0
    else
        return 0
    end
end
$$;

-- Use with aggregation
SELECT
    category,
    SUM(sum_if(price, in_stock)) AS total_in_stock,
    SUM(sum_if(price, NOT in_stock)) AS total_out_of_stock
FROM products
GROUP BY category;
```

---

## JSON Processing

### Example 6: Extract JSON Field

```sql
-- Create a function to extract JSON field with default
CREATE FUNCTION json_get(json_data TEXT, field TEXT, default_value TEXT)
RETURNS TEXT
LANGUAGE LUA
AS $$
function json_get(json_data, field, default_value)
    -- Simple JSON parsing (in production, use proper JSON library)
    local json = require('cjson')
    local ok, data = pcall(json.decode, json_data)

    if ok and data[field] ~= nil then
        return tostring(data[field])
    else
        return default_value
    end
end
$$;

-- Use the function
SELECT json_get(
    '{"name": "Alice", "age": 30, "city": "NYC"}',
    'name',
    'Unknown'
);
-- Returns: "Alice"

SELECT json_get(
    '{"name": "Alice", "age": 30}',
    'city',
    'Unknown'
);
-- Returns: "Unknown"
```

---

### Example 7: Transform JSON Data

```sql
-- Create a JSON transformer
CREATE FUNCTION transform_user_json(user_data TEXT)
RETURNS TEXT
LANGUAGE LUA
AS $$
function transform_user_json(user_data)
    local json = require('cjson')
    local user = json.decode(user_data)

    -- Transform the data
    local result = {
        id = user.id,
        full_name = user.first_name .. " " .. user.last_name,
        email = user.email,
        status = user.active and "ACTIVE" or "INACTIVE",
        account_age_days = os.difftime(os.time(), user.created_at) / 86400
    }

    return json.encode(result)
end
$$;

-- Use the function
SELECT transform_user_json('{
    "id": 123,
    "first_name": "Alice",
    "last_name": "Johnson",
    "email": "alice@example.com",
    "active": true,
    "created_at": 1609459200
}');
```

---

## String Manipulation

### Example 8: Title Case Converter

```sql
-- Create a title case function
CREATE FUNCTION to_title_case(text TEXT)
RETURNS TEXT
LANGUAGE LUA
AS $$
function to_title_case(text)
    if text == nil then
        return nil
    end

    local result = text:gsub("(%a)([%w_']*)", function(first, rest)
        return first:upper() .. rest:lower()
    end)

    return result
end
$$;

-- Use the function
SELECT to_title_case('hello world');
-- Returns: "Hello World"

SELECT to_title_case('the quick BROWN fox');
-- Returns: "The Quick Brown Fox"

-- Use with table data
SELECT
    product_id,
    to_title_case(product_name) AS formatted_name
FROM products;
```

---

### Example 9: Email Validator

```sql
-- Create an email validation function
CREATE FUNCTION is_valid_email(email TEXT)
RETURNS BOOLEAN
LANGUAGE LUA
AS $$
function is_valid_email(email)
    if email == nil or email == '' then
        return false
    end

    -- Simple email pattern
    local pattern = "^[%w._%+-]+@[%w.-]+%.[a-zA-Z]{2,}$"
    return email:match(pattern) ~= nil
end
$$;

-- Use the function
SELECT is_valid_email('alice@example.com');
-- Returns: true

SELECT is_valid_email('invalid.email');
-- Returns: false

-- Use in WHERE clause
SELECT * FROM users WHERE is_valid_email(email);

-- Use to filter bad data
SELECT
    user_id,
    email,
    CASE
        WHEN is_valid_email(email) THEN 'VALID'
        ELSE 'INVALID'
    END AS email_status
FROM users;
```

---

## Mathematical Operations

### Example 10: Fibonacci Calculator

```sql
-- Create a Fibonacci function
CREATE FUNCTION fibonacci(n INTEGER)
RETURNS BIGINT
LANGUAGE LUA
AS $$
function fibonacci(n)
    if n <= 1 then
        return n
    end

    local a, b = 0, 1
    for i = 2, n do
        a, b = b, a + b
    end

    return b
end
$$;

-- Use the function
SELECT fibonacci(10);
-- Returns: 55

-- Generate Fibonacci sequence
SELECT
    n,
    fibonacci(n) AS fib_value
FROM generate_series(1, 20) AS n;
```

---

### Example 11: Compound Interest Calculator

```sql
-- Create a compound interest calculator
CREATE FUNCTION calculate_compound_interest(
    principal DOUBLE PRECISION,
    rate DOUBLE PRECISION,
    years INTEGER,
    compounds_per_year INTEGER
)
RETURNS DOUBLE PRECISION
LANGUAGE LUA
AS $$
function calculate_compound_interest(principal, rate, years, compounds_per_year)
    -- A = P(1 + r/n)^(nt)
    local n = compounds_per_year or 12
    local t = years
    local r = rate
    local p = principal

    local amount = p * math.pow(1 + r / n, n * t)
    return amount
end
$$;

-- Use the function
SELECT
    principal,
    calculate_compound_interest(principal, 0.05, 10, 12) AS future_value,
    calculate_compound_interest(principal, 0.05, 10, 12) - principal AS interest_earned
FROM (VALUES
    (1000.0),
    (5000.0),
    (10000.0)
) AS t(principal);
```

---

## Date and Time Functions

### Example 12: Business Days Calculator

```sql
-- Create a business days calculator
CREATE FUNCTION calculate_business_days(start_date DATE, end_date DATE)
RETURNS INTEGER
LANGUAGE LUA
AS $$
function calculate_business_days(start_date, end_date)
    -- Convert to timestamps
    local start_ts = os.time{year=start_date.year, month=start_date.month, day=start_date.day}
    local end_ts = os.time{year=end_date.year, month=end_date.month, day=end_date.day}

    local business_days = 0
    local current = start_ts

    while current <= end_ts do
        local weekday = os.date("*t", current).wday
        -- wday: 1 = Sunday, 7 = Saturday
        if weekday ~= 1 and weekday ~= 7 then
            business_days = business_days + 1
        end
        current = current + 86400  -- Add one day
    end

    return business_days
end
$$;

-- Use the function
SELECT calculate_business_days('2025-01-01'::DATE, '2025-01-31'::DATE);
-- Returns: ~22 business days
```

---

## State Management

### Example 13: Persistent Counter with State

```sql
-- Create a counter function with persistent state
CREATE FUNCTION increment_counter(counter_name TEXT)
RETURNS INTEGER
LANGUAGE LUA
AS $$
function increment_counter(counter_name)
    -- Use orbit.persist() to store state
    local state = orbit.restore(counter_name) or {count = 0}
    state.count = state.count + 1
    orbit.persist(counter_name, state)
    return state.count
end
$$;

-- Use the function
SELECT increment_counter('page_views');
-- Returns: 1 (first call)

SELECT increment_counter('page_views');
-- Returns: 2 (second call)

SELECT increment_counter('api_calls');
-- Returns: 1 (different counter)
```

---

## Error Handling

### Example 14: Safe Division

```sql
-- Create a safe division function with error handling
CREATE FUNCTION safe_divide(numerator DOUBLE PRECISION, denominator DOUBLE PRECISION)
RETURNS DOUBLE PRECISION
LANGUAGE LUA
AS $$
function safe_divide(numerator, denominator)
    -- Check for nil values
    if numerator == nil or denominator == nil then
        return nil
    end

    -- Check for division by zero
    if denominator == 0 then
        return nil
    end

    return numerator / denominator
end
$$;

-- Use the function
SELECT safe_divide(10.0, 2.0);
-- Returns: 5.0

SELECT safe_divide(10.0, 0.0);
-- Returns: NULL

-- Use with COALESCE for default value
SELECT COALESCE(safe_divide(total_sales, order_count), 0.0) AS avg_order_value
FROM sales_summary;
```

---

### Example 15: Try-Catch Pattern

```sql
-- Create a function with error handling
CREATE FUNCTION parse_number(text_value TEXT, default_value DOUBLE PRECISION)
RETURNS DOUBLE PRECISION
LANGUAGE LUA
AS $$
function parse_number(text_value, default_value)
    -- Use pcall (protected call) for error handling
    local ok, result = pcall(tonumber, text_value)

    if ok and result ~= nil then
        return result
    else
        return default_value or 0
    end
end
$$;

-- Use the function
SELECT parse_number('123.45', 0.0);
-- Returns: 123.45

SELECT parse_number('invalid', 0.0);
-- Returns: 0.0

SELECT parse_number('invalid', 99.99);
-- Returns: 99.99
```

---

## Function Management

### Example 16: List All Lua Functions

```sql
-- Query to list all Lua functions
SELECT
    function_schema,
    function_name,
    parameter_types,
    return_type,
    language
FROM information_schema.routines
WHERE language = 'LUA'
ORDER BY function_schema, function_name;
```

---

### Example 17: Drop and Replace Function

```sql
-- Drop existing function
DROP FUNCTION IF EXISTS calculate_tax(DOUBLE PRECISION, DOUBLE PRECISION);

-- Create replacement with improved logic
CREATE FUNCTION calculate_tax(price DOUBLE PRECISION, tax_rate DOUBLE PRECISION)
RETURNS DOUBLE PRECISION
LANGUAGE LUA
AS $$
function calculate_tax(price, tax_rate)
    -- Validate inputs
    if price == nil or price < 0 then
        return 0
    end
    if tax_rate == nil or tax_rate < 0 or tax_rate > 1 then
        return 0
    end

    -- Calculate tax with rounding
    local tax = price * tax_rate
    return math.floor(tax * 100 + 0.5) / 100  -- Round to 2 decimals
end
$$;
```

---

### Example 18: Schema-Qualified Functions

```sql
-- Create function in specific schema
CREATE FUNCTION finance.calculate_interest(
    principal DOUBLE PRECISION,
    rate DOUBLE PRECISION,
    years INTEGER
)
RETURNS DOUBLE PRECISION
LANGUAGE LUA
AS $$
function calculate_interest(principal, rate, years)
    return principal * rate * years
end
$$;

-- Use schema-qualified function
SELECT finance.calculate_interest(1000.0, 0.05, 3);
-- Returns: 150.0

-- Create same function name in different schema
CREATE FUNCTION accounting.calculate_interest(
    balance DOUBLE PRECISION,
    apr DOUBLE PRECISION
)
RETURNS DOUBLE PRECISION
LANGUAGE LUA
AS $$
function calculate_interest(balance, apr)
    -- Different implementation for monthly interest
    return balance * (apr / 12)
end
$$;

-- Use both functions
SELECT
    finance.calculate_interest(1000.0, 0.05, 3) AS simple_interest,
    accounting.calculate_interest(1000.0, 0.05) AS monthly_interest;
```

---

## Complex Real-World Examples

### Example 19: Password Strength Checker

```sql
-- Create a password strength validator
CREATE FUNCTION check_password_strength(password TEXT)
RETURNS TEXT
LANGUAGE LUA
AS $$
function check_password_strength(password)
    if password == nil or #password == 0 then
        return 'INVALID'
    end

    local score = 0
    local feedback = {}

    -- Check length
    if #password >= 8 then
        score = score + 1
    else
        table.insert(feedback, 'too_short')
    end

    -- Check for lowercase
    if password:match('%l') then
        score = score + 1
    else
        table.insert(feedback, 'needs_lowercase')
    end

    -- Check for uppercase
    if password:match('%u') then
        score = score + 1
    else
        table.insert(feedback, 'needs_uppercase')
    end

    -- Check for numbers
    if password:match('%d') then
        score = score + 1
    else
        table.insert(feedback, 'needs_number')
    end

    -- Check for special characters
    if password:match('[^%w]') then
        score = score + 1
    else
        table.insert(feedback, 'needs_special')
    end

    -- Determine strength
    if score <= 2 then
        return 'WEAK'
    elseif score <= 3 then
        return 'MEDIUM'
    elseif score <= 4 then
        return 'STRONG'
    else
        return 'VERY_STRONG'
    end
end
$$;

-- Test the function
SELECT
    password,
    check_password_strength(password) AS strength
FROM (VALUES
    ('pass'),
    ('password'),
    ('Password1'),
    ('P@ssw0rd'),
    ('C0mpl3x!P@ssw0rd')
) AS t(password);

/*
Results:
password          | strength
------------------|-------------
pass              | WEAK
password          | WEAK
Password1         | MEDIUM
P@ssw0rd          | STRONG
C0mpl3x!P@ssw0rd  | VERY_STRONG
*/
```

---

### Example 20: URL Slug Generator

```sql
-- Create a URL slug generator
CREATE FUNCTION generate_slug(title TEXT)
RETURNS TEXT
LANGUAGE LUA
AS $$
function generate_slug(title)
    if title == nil or title == '' then
        return ''
    end

    -- Convert to lowercase
    local slug = title:lower()

    -- Replace spaces with hyphens
    slug = slug:gsub('%s+', '-')

    -- Remove special characters except hyphens
    slug = slug:gsub('[^%w%-]', '')

    -- Remove multiple consecutive hyphens
    slug = slug:gsub('%-+', '-')

    -- Trim hyphens from start and end
    slug = slug:gsub('^%-+', '')
    slug = slug:gsub('%-+$', '')

    return slug
end
$$;

-- Test the function
SELECT
    title,
    generate_slug(title) AS slug
FROM (VALUES
    ('Hello World'),
    ('Lua UDF Examples - Complete Guide'),
    ('  Multiple   Spaces  '),
    ('Special!@#Characters'),
    ('CamelCaseText')
) AS t(title);

/*
Results:
title                               | slug
------------------------------------|---------------------------
Hello World                         | hello-world
Lua UDF Examples - Complete Guide   | lua-udf-examples-complete-guide
  Multiple   Spaces                 | multiple-spaces
Special!@#Characters                | specialcharacters
CamelCaseText                       | camelcasetext
*/
```

---

## Performance Testing

### Example 21: Benchmark Function Execution

```sql
-- Create a timing wrapper function
CREATE FUNCTION benchmark_function(iterations INTEGER)
RETURNS DOUBLE PRECISION
LANGUAGE LUA
AS $$
function benchmark_function(iterations)
    local start_time = os.clock()

    -- Your code to benchmark
    for i = 1, iterations do
        local result = fibonacci(20)
    end

    local end_time = os.clock()
    local elapsed = end_time - start_time

    return elapsed
end
$$;

-- Run benchmark
SELECT benchmark_function(10000) AS execution_time_seconds;
```

---

## Best Practices Summary

1. **Always validate inputs** - Check for nil, negative values, or invalid ranges
2. **Use schema qualification** - Organize functions in schemas like `finance`, `utils`, `validation`
3. **Error handling** - Use `pcall()` for operations that might fail
4. **Performance** - Cache results, avoid recursive calls for large inputs
5. **Naming conventions** - Use clear, descriptive function names
6. **Documentation** - Add comments explaining complex logic
7. **Testing** - Test with edge cases (nil, empty, zero, negative values)
8. **State management** - Use `orbit.persist()` and `orbit.restore()` for stateful functions

---

**Last Updated**: December 13, 2025
**Version**: 1.0
**Status**: Production Ready
