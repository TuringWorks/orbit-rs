# JavaScript Engine Decision Document

**Date**: 2025-12-08
**Decision**: Multi-Engine Approach (Boa + rquickjs)
**Status**: Approved for Implementation

---

## Decision Summary

After evaluating 5 JavaScript engine options for OrbitRS, we have decided to adopt a **multi-engine approach**:

1. **Boa** for PostgreSQL PL/JavaScript
2. **rquickjs** for MongoDB and Redis scripting

---

## Evaluation Criteria

| Criterion | Weight | Boa | rustyscript | rquickjs | Deno Core | QuickJS |
|-----------|--------|-----|-------------|----------|-----------|---------|
| **Performance** | 25% | 6/10 | 10/10 | 9/10 | 10/10 | 9/10 |
| **Security** | 30% | 10/10 | 8/10 | 8/10 | 9/10 | 7/10 |
| **Binary Size** | 15% | 8/10 | 2/10 | 9/10 | 2/10 | 10/10 |
| **Maturity** | 15% | 6/10 | 4/10 | 9/10 | 10/10 | 10/10 |
| **Integration** | 15% | 10/10 | 7/10 | 8/10 | 5/10 | 4/10 |
| **Total Score** | 100% | **8.1** | 6.7 | **8.6** | 7.3 | 7.5 |

---

## Decision Rationale

### PostgreSQL → Boa

**Why Boa?**
1. **Pure Rust**: No C/C++ dependencies = smaller attack surface
2. **Security-First**: Built-in sandboxing, memory safety
3. **Type Integration**: Seamless Rust ↔ JavaScript type conversion
4. **Future-Proof**: Active development, ES2023 support
5. **Acceptable Performance**: Function calls are not hot path

**Why Not Others?**
- **rustyscript/Deno Core**: 20MB binary overhead unacceptable
- **rquickjs**: C dependency less desirable for security-critical code
- **QuickJS**: No high-level Rust bindings

**Trade-offs Accepted**:
- 2-5x slower than V8 (acceptable for UDF calls)
- Less battle-tested (mitigated by extensive testing)

### MongoDB/Redis → rquickjs

**Why rquickjs?**
1. **Performance**: Near-V8 speed, critical for query/command path
2. **Small Footprint**: ~1MB binary, low memory overhead
3. **Battle-Tested**: Production-ready, stable API
4. **Fast Startup**: Critical for script execution
5. **Mature Bindings**: Well-maintained Rust wrapper

**Why Not Others?**
- **Boa**: Too slow for hot path (query evaluation)
- **rustyscript/Deno Core**: Binary size prohibitive
- **QuickJS**: Requires unsafe Rust, manual memory management

**Trade-offs Accepted**:
- C dependency (acceptable for performance-critical code)
- ES2020 only (sufficient for MongoDB/Redis use cases)

---

## Architecture Decision

### Multi-Engine Justification

**Why Not Single Engine?**

We considered using a single engine for all protocols but rejected it because:

1. **Different Requirements**:
   - PostgreSQL: Security > Performance
   - MongoDB/Redis: Performance > Pure Rust

2. **Optimal Trade-offs**:
   - Boa: Best security for untrusted user functions
   - rquickjs: Best performance for query path

3. **Acceptable Complexity**:
   - Two engines manageable
   - Clear separation of concerns
   - Minimal code duplication

**Cost-Benefit Analysis**:
- **Cost**: +1MB binary, +2 dependencies, dual maintenance
- **Benefit**: Optimal performance + security for each protocol
- **Verdict**: Benefits outweigh costs

---

## Implementation Strategy

### Phase 1: Boa for PostgreSQL (Weeks 1-4)
- Foundation and PL/JavaScript support
- Security sandboxing
- Type conversions
- Testing

### Phase 2: rquickjs for MongoDB (Weeks 5-6)
- $where operator
- $function aggregation
- Script caching
- Testing

### Phase 3: rquickjs for Redis (Weeks 7-8)
- EVAL/EVALSHA commands
- Redis API bindings
- Script caching
- Testing

### Phase 4: Hardening (Weeks 9-10)
- Security audits
- Performance optimization
- Documentation
- Benchmarking

---

## Security Considerations

### Boa Security Features
- ✅ Pure Rust (memory-safe)
- ✅ Built-in sandboxing
- ✅ Execution timeouts
- ✅ Memory limits
- ✅ API restrictions
- ✅ No eval() by default

### rquickjs Security Features
- ✅ Interrupt handlers (timeouts)
- ✅ Memory limits
- ✅ Context isolation
- ✅ API whitelisting
- ⚠️ C dependency (requires careful review)

### Security Testing Plan
1. Fuzzing with AFL/libFuzzer
2. Timeout enforcement tests
3. Memory limit tests
4. Sandbox escape attempts
5. Third-party security audit

---

## Performance Expectations

### Boa (PostgreSQL)
- Function call overhead: **< 10μs**
- Type conversion: **< 100ns**
- Memory per context: **< 1MB**
- Acceptable for UDF calls (not hot path)

### rquickjs (MongoDB/Redis)
- Script compilation: **< 1ms**
- Execution overhead: **< 1μs**
- Memory per context: **< 500KB**
- Critical for query/command performance

### Benchmarking Plan
1. Micro-benchmarks for each operation
2. Real-world workload simulations
3. Comparison with native implementations
4. Memory profiling
5. Concurrency testing

---

## Risk Assessment

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|------------|
| Boa too slow | Medium | High | Fallback to rquickjs if needed |
| Security vulnerability | Low | Critical | Extensive testing + audits |
| Memory leaks | Medium | High | Automated leak detection |
| API instability | Low | Medium | Pin versions, test upgrades |
| Build complexity | Low | Low | CI/CD automation |

---

## Alternative Considered: Single Engine

### Option A: Boa Only
**Pros**: Pure Rust, simple
**Cons**: Too slow for MongoDB/Redis
**Verdict**: ❌ Rejected (performance)

### Option B: rquickjs Only
**Pros**: Fast, small
**Cons**: C dependency for all protocols
**Verdict**: ❌ Rejected (security)

### Option C: rustyscript Only
**Pros**: V8 performance
**Cons**: 20MB binary, complex build
**Verdict**: ❌ Rejected (binary size)

### Option D: Deno Core Only
**Pros**: Production-ready
**Cons**: Overkill, 20MB binary
**Verdict**: ❌ Rejected (complexity)

---

## Success Metrics

### Functional
- [ ] PostgreSQL PL/JavaScript functions execute correctly
- [ ] MongoDB $where/$function operators work
- [ ] Redis EVAL/EVALSHA commands functional
- [ ] All security tests pass
- [ ] No memory leaks detected

### Performance
- [ ] Boa function calls < 10μs overhead
- [ ] rquickjs script compilation < 1ms
- [ ] Memory usage < 1MB per context
- [ ] 1000+ concurrent executions supported

### Quality
- [ ] 90%+ test coverage
- [ ] Zero critical security issues
- [ ] Documentation complete
- [ ] Benchmarks published

---

## Dependencies

```toml
[dependencies]
# PostgreSQL JavaScript (Boa)
boa_engine = "0.18"
boa_gc = "0.18"

# MongoDB/Redis JavaScript (rquickjs)
rquickjs = { version = "0.6", features = ["array-buffer", "classes"] }

[features]
default = ["js-boa", "js-quickjs"]
js-boa = ["dep:boa_engine", "dep:boa_gc"]
js-quickjs = ["dep:rquickjs"]
```

**Binary Size Impact**:
- Boa: ~2MB
- rquickjs: ~1MB
- **Total**: ~3MB additional

---

## Approval

**Recommended**: ✅ Approve multi-engine approach (Boa + rquickjs)

**Approvers**:
- [ ] Technical Lead
- [ ] Security Team
- [ ] Performance Team
- [ ] Product Owner

**Next Steps**:
1. Create GitHub issues for each phase
2. Setup CI/CD for JavaScript testing
3. Begin Phase 1 implementation
4. Schedule security review

---

## References

- [Boa Engine Documentation](https://boa-dev.github.io/boa/)
- [rquickjs Documentation](https://docs.rs/rquickjs/)
- [PostgreSQL PL/v8 Reference](https://plv8.github.io/)
- [MongoDB Server-Side JavaScript](https://www.mongodb.com/docs/manual/core/server-side-javascript/)
- [Redis EVAL Documentation](https://redis.io/commands/eval/)
