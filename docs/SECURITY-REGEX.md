# Regular Expression Security (ReDoS Mitigation)

## Overview

This document outlines the security measures implemented to prevent Regular Expression Denial of Service (ReDoS) attacks in the Orbit Desktop application, specifically in the query formatting functionality.

## What is ReDoS?

ReDoS occurs when specially crafted input strings cause regular expressions to exhibit exponential time complexity due to excessive backtracking. This can lead to:

- Application freezing or becoming unresponsive
- CPU consumption spikes
- Denial of service for legitimate users
- Resource exhaustion

## Vulnerable Patterns

Common regex patterns that can cause ReDoS:

```javascript
// DANGEROUS: Nested quantifiers with alternation
/(a+)+b/
/(a|a)*b/
/(a*)*b/

// DANGEROUS: Alternation with overlapping patterns
/(?:a|a)*$/
/(?:SELECT|FROM|WHERE|...)+/  // Our original vulnerable pattern
```

## Our Mitigation Strategy

### 1. Input Size Limits

```typescript
const MAX_QUERY_SIZE = 1024 * 100; // 100KB limit
if (input.length > MAX_QUERY_SIZE) {
  throw new Error(`Query too large for formatting`);
}
```

**Rationale**: Prevents attackers from submitting extremely large inputs that could amplify regex processing time.

### 2. Timeout Protection

```typescript
const formatWithTimeout = (text: string, timeoutMs: number = 5000): string => {
  const start = Date.now();
  
  const checkTimeout = () => {
    if (Date.now() - start > timeoutMs) {
      throw new Error('Query formatting timeout - potential ReDoS detected');
    }
  };
  // ... checkTimeout() called before each regex operation
};
```

**Rationale**: Prevents infinite or near-infinite regex execution by enforcing time limits.

### 3. Safe Regex Patterns

#### Before (Vulnerable):
```javascript
// Dangerous alternation with word boundaries
.replaceAll(/\\b(?:SELECT|FROM|WHERE|JOIN|GROUP BY|HAVING|ORDER BY|LIMIT)\\b/gi, '\\n$1')
```

#### After (Safe):
```javascript
// Individual replacements prevent alternation backtracking
const keywords = ['SELECT', 'FROM', 'WHERE', 'JOIN', 'GROUP BY', 'HAVING', 'ORDER BY', 'LIMIT'];
for (const keyword of keywords) {
  const regex = new RegExp(`\\\\b${keyword}\\\\b`, 'gi');
  result = result.replaceAll(regex, `\\n${keyword}`);
}
```

#### Safe Patterns Used:

1. **Atomic Groups**: `(?:[ \\t\\r\\n])+` prevents backtracking on whitespace
2. **Character Classes**: `[\\t ]*` with limited quantifiers
3. **Anchored Patterns**: `^[ \\t]+` anchored to line start
4. **Individual Matching**: Separate regex for each keyword

### 4. Graceful Fallback

```typescript
try {
  const formatted = safeFormatQuery(value);
  onChange(formatted);
} catch (error) {
  console.error('Query formatting failed:', error);
  // Safe fallback without regex
  const basicFormatted = value
    .split(/\\s+/)
    .filter(word => word.length > 0)
    .join(' ')
    .trim();
  onChange(basicFormatted);
}
```

**Rationale**: If regex processing fails or times out, fall back to simple string operations.

## Testing Strategy

Our test suite (`queryFormatterSafety.test.ts`) includes:

### 1. ReDoS Attack Simulation
- Tests patterns known to cause exponential backtracking
- Verifies execution time remains under reasonable limits
- Validates timeout protection works correctly

### 2. Performance Benchmarks
- Measures execution time for various input sizes
- Ensures linear time complexity O(n)
- Tests edge cases like massive whitespace sequences

### 3. Functional Validation
- Verifies formatting still works correctly
- Tests unicode and special character handling
- Ensures query semantics are preserved

## Regex Complexity Analysis

### Safe Patterns (Linear Time - O(n)):
- `(?:[ \\t\\r\\n])+` - Atomic group, no backtracking
- `[ \\t]*,[ \\t]*` - Character classes with bounded quantifiers
- `^[ \\t]+` - Anchored to line start, no alternation
- `\\b${keyword}\\b` - Individual word boundaries

### Avoided Patterns (Exponential Time - O(2^n)):
- `(a+)+` - Nested quantifiers
- `(a|a)*` - Overlapping alternation
- `(?:word1|word2|...)+` - Multiple alternation with quantifiers

## Monitoring and Alerts

Consider implementing:

1. **Performance Monitoring**: Track regex execution times
2. **Error Logging**: Log timeout events and input patterns
3. **Rate Limiting**: Limit formatting requests per user/session
4. **Input Validation**: Additional validation beyond size limits

## Best Practices for Future Development

1. **Always analyze regex complexity** before implementing
2. **Use online tools** like [regex101.com](https://regex101.com) to test patterns
3. **Prefer string methods** over regex when possible
4. **Implement timeouts** for any regex processing
5. **Add comprehensive tests** for regex security
6. **Consider using safe regex libraries** that prevent ReDoS

## References

- [OWASP Regular Expression Security](https://owasp.org/www-community/attacks/Regular_expression_Denial_of_Service_-_ReDoS)
- [ReDoS Attack Examples](https://github.com/attackercan/regexp-security-cheatsheet)
- [Safe Regex Patterns](https://blog.superhuman.com/how-to-eliminate-regular-expression-denial-of-service/)

## Security Contact

For security-related issues or questions about regex patterns, please contact the development team through appropriate security channels.