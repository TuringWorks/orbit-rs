#  Priority TODO List - Orbit-RS Development Roadmap

> Generated: October 12, 2025

##  **SPRINT 1 - IMMEDIATE FIXES (Week 1-2)**

### ** Testing Infrastructure - CRITICAL**

**Impact**: Blocks all development and CI/CD

- [ ] **Fix hanging tests** in cargo test suite
  - File: Various test files
  - Issue: Tests timeout after 60+ seconds
  - Action: Identify and isolate problematic tests

- [ ] **Re-enable or remove disabled tests**
  - File: `orbit-protocols/src/postgres_wire/sql/tests.rs`  
  - Lines: 882-1007 (40+ disabled DCL tests)
  - Action: Either fix or remove commented-out test cases

- [ ] **Add test categorization**
  - Create `#[ignore]` tags for slow/integration tests
  - Separate unit tests from integration tests
  - Add benchmark tests category

### ** Documentation Accuracy - HIGH**

**Impact**: Misleading users and contributors  

- [ ] **Update main README.md**
  - Remove claims about "Full PostgreSQL compatibility"
  - Add implementation status percentages per protocol
  - Update feature matrix with accurate status

- [ ] **Fix protocol documentation**
  - File: `docs/protocols/protocol_adapters.md`
  - Action: Mark incomplete features as "Experimental" or "In Development"
  - Add completion percentages

- [ ] **Create honest feature matrix**

  ```markdown
  | Protocol | Status | Completeness | Production Ready |
  |----------|--------|--------------|------------------|
  | RESP     |      | 100%         | Yes             |
  | PostgreSQL|     | 30%          | No              |
  | Neo4j    |      | 5%           | No              |
  | ArangoDB |      | 5%           | No              |
  ```

---

##  **SPRINT 2 - CORE STABILITY (Week 3-4)**

### ** Critical Code Issues - CRITICAL**

- [ ] **Fix unimplemented!() macros in core paths**
  - File: `orbit-protocols/src/postgres_wire/sql/expression_evaluator.rs`
  - Lines: 75, 1143, 1154, 1167, 1181 (5 critical functions)
  - Action: Either implement or gracefully return errors

- [ ] **Complete basic SQL execution**  
  - File: `orbit-protocols/src/postgres_wire/sql/executor.rs`
  - Lines: 284, 406, 531-534 (core execution methods)
  - Action: Implement SELECT, INSERT, UPDATE, DELETE basics

- [ ] **Fix panic! statements in production code**
  - Search for all `panic!` calls and replace with proper error handling
  - Priority: Any panic in request-handling code paths

### ** Development Experience**

- [ ] **Make cargo test pass completely**
  - Fix or isolate failing tests
  - Add timeout configuration
  - Create stable CI pipeline

- [ ] **Add basic integration tests**
  - RESP server end-to-end test
  - PostgreSQL basic connection test  
  - Example application smoke tests

---

##  **SPRINT 3-4 - STABILIZE ONE PROTOCOL (Week 5-8)**

### **Option A: Complete PostgreSQL Wire Protocol**

**Rationale**: Most mature after RESP server

#### **Phase 1 - Parser Completion**

- [ ] **Fix SQL parser gaps**
  - File: `orbit-protocols/src/postgres_wire/sql/parser/`
  - Action: Complete SELECT/INSERT/UPDATE/DELETE parsing
  - Test: All basic SQL should parse without errors

- [ ] **Implement expression evaluator basics**
  - File: `orbit-protocols/src/postgres_wire/sql/expression_evaluator.rs`
  - Priority functions: arithmetic, comparison, logical operators
  - Skip: window functions, complex aggregates (Phase 2)

#### **Phase 2 - Execution Engine**  

- [ ] **Complete SQL executor core**
  - File: `orbit-protocols/src/postgres_wire/sql/executor.rs`
  - Action: Implement data insertion, selection, modification
  - Target: Handle simple CRUD operations

- [ ] **Add basic transaction support**
  - BEGIN/COMMIT/ROLLBACK functionality
  - Skip: Savepoints, isolation levels (Phase 3)

#### **Phase 3 - Integration**

- [ ] **Connect to actor system**  
  - Replace in-memory HashMap with actor storage
  - Integrate with OrbitClient
  - Test: Data persists across connections

### **Option B: Focus on RESP Server Excellence**

**Rationale**: Build on existing success

- [ ] **Add Redis Cluster support**
  - CLUSTER command implementation
  - Hash slot routing
  - Node discovery

- [ ] **Implement Redis Streams**  
  - XADD, XREAD, XRANGE commands
  - Consumer groups
  - Stream replication

- [ ] **Add Redis Modules API**
  - Module loading infrastructure
  - Custom command registration  
  - Memory management

---

##  **SPRINT 5-6 - INFRASTRUCTURE (Week 9-12)**

### ** Core Systems Completion**

- [ ] **Complete OrbitQL optimizer**
  - File: `orbit/shared/src/orbitql/optimizer.rs`
  - Focus: cost-based optimization basics (lines 655, 783, 925)
  - Skip: Advanced optimizations for now

- [ ] **Stabilize persistence layer**
  - File: `orbit/server/src/persistence/cloud.rs`
  - Complete: Core operations (lines 632-675)
  - Test: All backends work consistently

- [ ] **Add monitoring and observability**
  - Metrics collection
  - Health checks  
  - Distributed tracing

### ** Security and Production Readiness**

- [ ] **Implement basic authentication**
  - User management system
  - Password authentication
  - Role-based access control

- [ ] **Add configuration management**
  - Environment-based configuration
  - Secrets management  
  - Feature flags

---

##  **SPRINT 7+ - ADVANCED FEATURES (Month 4+)**

### ** Performance and Scale**

- [ ] **Query optimization**
  - Complete distributed joins (orbitql/executor.rs:267)
  - Implement predicate pushdown
  - Add query caching

- [ ] **Horizontal scaling**  
  - Complete distributed query processing
  - Add load balancing
  - Implement sharding

### ** Advanced Capabilities**

- [ ] **Machine Learning Integration**
  - Complete neural network layers (orbit-ml/src/neural_networks/layers.rs:211,218)
  - Implement distributed training (orbit-ml/src/training.rs:244)  
  - Add SQL ML functions

- [ ] **Spatial and Vector Features**
  - Complete spatial indexing (orbit/shared/src/spatial/index.rs:480,485)
  - Add vector similarity search
  - Implement geospatial queries

---

##  **FILE-SPECIFIC TODO PRIORITIES**

### ** CRITICAL FILES** (Fix First)

1. `orbit-protocols/src/postgres_wire/sql/executor.rs` - 18 TODOs
2. `orbit-protocols/src/postgres_wire/sql/expression_evaluator.rs` - 17 TODOs  
3. `orbit-protocols/src/postgres_wire/sql/tests.rs` - 40+ disabled tests
4. `orbit/shared/src/orbitql/optimizer.rs` - 12 optimization TODOs
5. `orbit/server/src/persistence/cloud.rs` - 16 storage TODOs

### ** HIGH PRIORITY FILES**

1. `orbit/shared/src/orbitql/distributed.rs` - 21 distributed processing TODOs
2. `orbit/shared/src/timeseries/storage.rs` - 16 time-series TODOs
3. `orbit/shared/src/orbitql/streaming.rs` - 9 streaming query TODOs
4. `orbit-protocols/src/resp/commands.rs` - 15 Redis extension TODOs
5. `orbit-ml/src/neural_networks/layers.rs` - ML layer TODOs

---

##  **SUCCESS METRICS**

### **Sprint 1-2 (Month 1)**

- [ ] `cargo test` passes completely (0 failures)
- [ ] Documentation reflects reality (no false claims)
- [ ] CI/CD pipeline is stable
- [ ] All examples work as documented

### **Sprint 3-4 (Month 2)**

- [ ] One protocol (PostgreSQL or enhanced RESP) is fully functional
- [ ] Integration tests cover end-to-end scenarios  
- [ ] Performance benchmarks are established
- [ ] Production deployment guide exists

### **Sprint 5-6 (Month 3)**

- [ ] Core infrastructure is complete and tested
- [ ] Security and authentication work
- [ ] Monitoring and observability are functional
- [ ] Multiple storage backends are stable

### **Long-term (Month 4+)**

- [ ] 2+ protocols are production-ready
- [ ] Advanced features (ML, spatial) are functional
- [ ] Performance targets are met
- [ ] Documentation is comprehensive and accurate

---

##  **EXECUTION STRATEGY**

1. **Start with testing** - Nothing else matters if tests don't work
2. **Fix documentation** - Stop misleading users immediately  
3. **Choose one protocol** - Go deep instead of broad
4. **Build incrementally** - Each sprint should add measurable value
5. **Maintain quality** - Don't add new TODOs while fixing existing ones

**The goal is to transform Orbit-RS from a promising prototype with inflated claims into a reliable, focused platform that delivers on its promises.**
