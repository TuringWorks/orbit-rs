# Orbit-RS Competitive Landscape & Fit-Gap Analysis

**Date:** 2026-08-05
**Status:** Living document — refresh alongside `PRD.md` when capabilities change
**Scope:** Competitor landscape, feature-by-feature fit-gap, prioritized improvement areas, with
particular depth on the AI/LLM surface (provider and model switchability).

> **Method note.** Every "Orbit-RS today" claim below was checked against the tree at
> commit `2aded3b8` (branch `refactor/functional-idioms-and-deps`) — by reading the module, not by
> reading the docs about the module. Where a capability exists as scaffolding rather than working
> code, it is marked **Scaffold**, not **Yes**. A green build is not evidence a feature works
> (see `CLAUDE.md` → *Verification*), and neither is a heading in a design doc.

---

## 1. Where Orbit-RS Actually Sits

Orbit-RS is an unusual shape. Almost nothing in the market occupies the same square:

- **Multi-protocol at the wire level.** PostgreSQL, MySQL, CQL, RESP, Cypher/Bolt, AQL, MongoDB,
  gRPC, REST, Flight, and OrbitQL are served from *one process over one storage layer*. Competitors
  generally pick one wire protocol and offer drivers for the rest.
- **Actor-model core.** Virtual actors are the unit of state and addressing, not tables or
  documents. This is a Microsoft-Orleans lineage, not a database lineage.
- **Multi-model storage.** Relational, document, graph, time-series, vector, and key-value share
  one engine.
- **Written in Rust**, with a heterogeneous-compute crate (`orbit/compute`, 28.7k LOC) targeting
  SIMD/GPU acceleration.

The nearest single competitor is **SurrealDB**; the realistic competitive set is a *stack* of
four to six products that Orbit-RS proposes to replace with one.

### 1.1 Competitor set

| Tier | Products | Why they compete |
|------|----------|------------------|
| **Direct — multi-model, AI-native** | SurrealDB 3.0, ArangoDB, Fauna (EOL), EdgeDB/Gel | Same "one database, many models, one query language" pitch |
| **Vector-first** | Pinecone, Qdrant, Weaviate, Milvus/Zilliz, Chroma, LanceDB | Own the RAG retrieval layer Orbit-RS wants |
| **Incumbent + extension** | Postgres + pgvector/pgvectorscale/AGE/TimescaleDB, MongoDB Atlas (Vector Search), Redis 8 (RediSearch), Oracle AI Database, SingleStore | The default choice; "good enough and already deployed" |
| **Graph** | Neo4j (+ GraphRAG package), TigerGraph, Memgraph, Kuzu | Own the graph + GraphRAG narrative |
| **Analytics/lakehouse** | ClickHouse, DuckDB, Databricks, Snowflake | Own the analytical half of HTAP |
| **AI gateway (adjacent, and the model for §4)** | LiteLLM, Portkey, OpenRouter, Cloudflare AI Gateway, Bedrock/Vertex | Define what "provider and model switchability" means in 2026 |
| **Rust LLM libraries** | `rig`, `genai`, `async-openai`, `llm-connector` | What Orbit-RS would otherwise depend on |

### 1.2 What the market moved to in 2025–2026

Three shifts matter, and Orbit-RS is positioned for all three but delivers on none of them fully:

1. **Agent memory is the new workload.** SurrealDB 3.0 (GA 2026-02-17, $23M raise) reframed the
   multi-model database explicitly as *AI agent memory and context graphs* — one ACID transaction
   spanning graph traversal, structured filter, and vector similarity. This is the single most
   direct competitive threat, because it is Orbit-RS's architecture with a finished AI story
   bolted on top.
2. **Inference moved inside the database boundary.** Oracle AI Database, SingleStore, MongoDB, and
   Postgres extensions now generate embeddings *during query execution* — no application round
   trip, no separate embedding service, data never leaves the security boundary. ONNX is the de
   facto handoff format.
3. **The model gateway became a required component.** LiteLLM/Portkey/OpenRouter normalized a
   feature set — unified API over 100+ providers, fallback chains, retries, circuit breakers,
   load balancing, semantic caching, per-tenant keys, budget limits, cost attribution. Any product
   that calls an LLM is now measured against that list.

---

## 2. Feature-by-Feature Fit-Gap

Legend: **Yes** = working and exercised · **Partial** = works with material limits ·
**Scaffold** = types/signatures exist, behavior does not · **No** = absent

### 2.1 Core database

| Feature | Orbit-RS | Best-in-class | Gap |
|---|---|---|---|
| Multi-protocol wire compatibility | **Yes** (10+) | Nobody | **Orbit-RS advantage — the differentiator** |
| Multi-model in one engine | **Yes** | SurrealDB, ArangoDB | Parity |
| ACID transactions | **Yes** (MVCC, 2PC, Saga) | SurrealDB, Postgres | Parity |
| Distributed consensus | **Partial** (Raft present) | CockroachDB, TiKV | Maturity/scale-test gap |
| Virtual actor model | **Yes** | Orleans (not a DB) | Unique |
| SIMD/GPU query acceleration | **Partial** | ClickHouse (SIMD), HeavyDB (GPU) | Unproven at benchmark level |
| Storage tiering (S3/Iceberg) | **Partial** | Databricks, Snowflake | Cold-tier maturity |
| Managed cloud offering | **No** | Every competitor | **Adoption blocker** |

### 2.2 Vector / retrieval

| Feature | Orbit-RS | Best-in-class | Gap |
|---|---|---|---|
| HNSW index | **Yes** (`protocols/vector_index.rs`) | Qdrant, Weaviate | Parity on algorithm |
| IVFFlat index | **Yes** | pgvector | Parity |
| Quantization (SQ/PQ/binary) | **No** | Qdrant, Milvus, pgvectorscale | **Memory cost 4–32× worse at scale** |
| Filtered vector search (pre-filter) | **Partial** | Qdrant (best-in-class) | Post-filter only ⇒ recall collapses under selective filters |
| Hybrid search (BM25 + vector + RRF) | **Partial** | Weaviate, Elastic | Full-text side is the weak half; GraphRAG context builder has a literal `// TODO: Add full-text search context` |
| Multi-vector / late interaction (ColBERT) | **No** | Vespa, Qdrant | Emerging table stakes |
| Reranking (cross-encoder) | **No** | Cohere, Weaviate, Vespa | Quality gap in RAG |
| **Automatic embedding generation on write** | **No** | Weaviate (vectorizers), MongoDB, Oracle | **Highest-leverage retrieval gap** |
| Vector index persistence/recovery | **Partial** | All | Rebuild cost on restart |

### 2.3 AI / LLM (the focus area)

This is where the distance from the market is largest, and it is also the cheapest to close.

| Feature | Orbit-RS **before** this work | Best-in-class | Gap severity |
|---|---|---|---|
| LLM provider abstraction | **Partial** — `LLMClient` trait, 348 LOC, GraphRAG-internal | `rig` (20+), `genai` (26+), LiteLLM (100+) | High |
| Anthropic support | **No** — `Err("Anthropic client not yet implemented")` | Universal | **High — a named enum variant that returns an error** |
| Azure OpenAI / Bedrock / Vertex / Gemini | **No** | LiteLLM, Portkey | High (enterprise procurement blocker) |
| OpenAI-compatible endpoints (vLLM, Groq, Together, OpenRouter, LM Studio) | **Partial** — one hardcoded `Local` variant | LiteLLM | Medium |
| **Runtime model switching (no restart)** | **No** — provider baked into actor construction | LiteLLM, Portkey | **High — the explicit ask** |
| Fallback chains / failover | **No** | LiteLLM, Portkey, OpenRouter | High (availability) |
| Retries with backoff + jitter | **No** — single attempt, error on failure | Universal | High |
| Circuit breaker | **No** | Portkey, LiteLLM | Medium |
| Request timeouts | **No** — unbounded `reqwest` default | Universal | **High — a hung provider hangs a query** |
| Streaming responses | **No** — `"stream": false` hardcoded | Universal | Medium |
| Embeddings via provider API | **No** abstraction | Universal | High |
| Token usage accounting | **Partial** — captured, never aggregated | LiteLLM | Medium |
| Cost tracking / budgets | **No** | LiteLLM, Portkey | Medium |
| Semantic caching | **No** | LiteLLM, Portkey | Medium — *and Orbit-RS already owns a vector index, so this is nearly free* |
| Per-tenant keys / rate limits | **No** | LiteLLM, Portkey | Medium (multi-tenant blocker) |
| Connection pooling | **No** — `Client::new()` **per request**, 3 sites | Universal | **High — new TLS handshake and connection pool per LLM call** |
| Secret handling | **No** — `api_key: String`, printable via `Debug` derive on config paths | Universal | **High — credential leak into logs** |
| Config honesty | **Broken** — `temperature`/`max_tokens` accepted by `create_llm_client`, then bound to `_` and discarded; the caller re-extracts them by matching the enum a second time | n/a | **Correctness. Direct violation of `CLAUDE.md` → Modelling Honesty: "decorative parameters invite false confidence"** |
| In-database inference (SQL-callable) | **Scaffold** (`orbit/ml/sql_extensions`) | Oracle, SingleStore, MindsDB | High |
| ONNX runtime | **No** | Oracle, SingleStore | Medium |
| GraphRAG | **Partial** — entity extraction, multi-hop reasoning, KG build all present | Neo4j GraphRAG, SurrealDB 3.0 | **Real asset; underserved by the LLM layer beneath it** |
| MCP server | **Partial** (`protocols/mcp/`, 12 modules) | Growing | Good position |

### 2.4 Operations

| Feature | Orbit-RS | Gap |
|---|---|---|
| Structured logging (`tracing`) | **Yes** | — |
| Prometheus metrics | **Yes** (`orbit-server-prometheus`) | LLM/AI metrics absent |
| K8s operator + Helm | **Yes** | — |
| Config from env over TOML | **Partial** | AI subsystem read env ad hoc (`std::env::var("OPENAI_API_KEY")` inline in a RESP handler) |
| Backup/PITR | **Partial** | Maturity |
| Multi-region | **Partial** | Maturity |

### 2.5 Honest weak spots outside AI

Measured, not guessed — `TODO`/`unimplemented!`/`FIXME` density per crate:

| Crate | TODOs | LOC | Read |
|---|---:|---:|---|
| `orbit/ml` | **470** | 31,094 | **Largely scaffolding.** `industry_models/` is a directory tree of `// TODO: Implement …` method bodies across healthcare/fintech/adtech/defense/logistics/banking/insurance. `graph_neural_networks/mod.rs` is a one-line placeholder. This crate promises far more than it does. |
| `orbit/server` | 296 | 258,484 | Normal density for its size |
| `orbit/shared` | 169 | 86,771 | Normal |
| `orbit/compute` | 44 | 28,745 | Reasonable |
| `orbit/engine` | 30 | 36,743 | Good |

**The `orbit/ml` verdict is the most important non-LLM finding in this document.** Seven industry
verticals' worth of model APIs exist as signatures with empty bodies. That is a liability, not an
asset: it inflates the apparent surface area, it cannot be tested, and any user who calls into it
gets silence or a default. The recommendation is in §3, item 6.

---

## 3. Prioritized Improvement Areas

Ranked by (competitive damage if unfixed) ÷ (effort to fix).

1. **LLM provider/model switchability** — *addressed by this workstream.* Small, self-contained,
   unblocks every AI feature above it, and directly closes the Anthropic/Azure/Bedrock enterprise
   procurement objection. **Doing now.**
2. **Automatic embedding generation on write.** Orbit-RS has HNSW *and* (after item 1) an embedding
   provider abstraction. Wiring "column X of table Y is auto-embedded by model Z" turns two
   components into the feature Weaviate charges for. Requires item 1 first.
3. **Semantic cache over the existing vector index.** Once items 1–2 land, this is a lookup against
   an index Orbit-RS already ships. LiteLLM needs a bolt-on Redis + vector store for this; Orbit-RS
   needs a query. Highest ratio of competitive-story to code in the whole list.
4. **Vector quantization + pre-filtered search.** The pure scale/quality gap vs Qdrant. Larger
   effort, no dependency on items 1–3, can run in parallel.
5. **Managed cloud offering.** Largest adoption blocker, entirely outside this workstream.
6. **Decide `orbit/ml`'s fate.** Either (a) gate `industry_models` behind a `experimental-`
   feature flag and remove it from public docs until real, or (b) delete it and reintroduce
   verticals one at a time with tests. Leaving 470 stub bodies in a shipped crate is a
   modelling-honesty failure at the package level. Recommendation: **(a) now, (b) as capacity
   allows** — cheaper, reversible, and immediately stops overclaiming.
7. **Benchmark the SIMD/GPU claim.** `orbit/compute` is 28.7k LOC of differentiator with no
   published number against ClickHouse or DuckDB. Unmeasured performance work is indistinguishable
   from no performance work.

---

## 4. Deep Dive — LLM Provider & Model Switchability

### 4.1 What the code did before this workstream

`orbit/server/src/protocols/graphrag/llm_client.rs`, 348 lines, three clients (OpenAI, Ollama,
Local) and a factory. Reading it against the market list produces eleven concrete defects:

| # | Defect | Evidence | Consequence |
|---|---|---|---|
| 1 | Anthropic returns an error | `create_llm_client` → `Err("Anthropic client not yet implemented")` | A configured provider fails at call time, not config time |
| 2 | Decorative config | `LLMProvider::OpenAI { temperature: _, max_tokens: _ }` in the factory | Config knobs that change nothing; caller compensates with a second `match` |
| 3 | New HTTP client per call | `Client::new()` inside each `generate()` | No pooling; TLS handshake per LLM call |
| 4 | No timeout | `reqwest` default is none | A hung provider hangs the calling query indefinitely |
| 5 | No retry | Single attempt | A 429 or transient 503 fails the user's query |
| 6 | No fallback | One provider per call | Provider outage = feature outage |
| 7 | No streaming | `"stream": false` literal | Cannot support incremental UX |
| 8 | Secrets in plain `String` | `api_key: String` on a `Serialize` type | Credential reachable by log/serialize path |
| 9 | No runtime switching | Provider chosen at actor construction | Changing model requires a restart |
| 10 | Env read inline in a handler | `std::env::var("OPENAI_API_KEY")` in `resp/commands/graphrag.rs` with a hardcoded `"gpt-4"` | 12-factor violation; model name unconfigurable |
| 11 | No embeddings abstraction | Absent | GraphRAG's embedding path has no provider story |

### 4.2 Build vs. buy

| Option | Verdict |
|---|---|
| Depend on `rig` | Agent framework — brings a vector-store abstraction Orbit-RS *is*, and an agent loop it does not want. Impedance mismatch. |
| Depend on `genai` | Closest fit, 26+ providers. But: no fallback/circuit-breaker/cost layer, no `OrbitError` integration, and it owns the retry policy Orbit-RS must own. |
| Depend on `async-openai` | OpenAI-shaped only. |
| **Build `orbit-llm`** | **Chosen.** ~1.5k LOC of HTTP shaping over `reqwest` (already a dependency). The value is not the HTTP calls — it is the *router*: fallback, breaker, cost, registry, and hot-swap, all of which need to be Orbit-native to integrate with `OrbitError`, `tracing` spans, Prometheus, and the config layering. A wrapper around `genai` would still need all of that, plus a translation layer. |

**Decision recorded:** build `orbit/llm` as a first-class workspace crate, depending only on
`reqwest`/`serde`/`tokio`/`orbit-shared`. No new heavyweight dependency. Providers are
`OpenAI-compatible`-shaped where possible so one implementation covers Azure, vLLM, Groq, Together,
OpenRouter, LM Studio, and DeepSeek.

### 4.3 Target design

```
                       ┌──────────────────────────────────────────┐
  GraphRAG ───────────▶│              LlmRegistry                 │
  RESP LLM.*  ────────▶│  name → ModelProfile (provider+model+    │
  SQL/OrbitQL ────────▶│         params+fallbacks), hot-swappable │
  MCP         ────────▶│  Arc<ArcSwap>-style read-mostly access   │
                       └────────────────┬─────────────────────────┘
                                        │ resolve(name | default)
                                        ▼
                       ┌──────────────────────────────────────────┐
                       │                Router                    │
                       │  timeout → retry(backoff+jitter) →       │
                       │  circuit breaker → fallback chain →      │
                       │  usage/cost accounting                   │
                       └────────────────┬─────────────────────────┘
                                        ▼
        ┌───────────┬───────────┬──────────────┬──────────────────┐
        │  OpenAI   │ Anthropic │    Ollama    │ OpenAI-compatible│
        │           │           │              │ (Azure/vLLM/Groq │
        │           │           │              │ /Together/…)     │
        └───────────┴───────────┴──────────────┴──────────────────┘
             shared reqwest::Client (pooled, timeout-bounded)
```

**Design rules adopted (all traceable to `CLAUDE.md`):**

- Every parameter accepted is a parameter used. No `temperature: _`. If a provider cannot honor a
  parameter, the response says so rather than silently dropping it.
- API keys are `SecretString` — `Debug`/`Display` redact, `Serialize` refuses.
- Absent usage data stays absent. Ollama does not report token counts; `TokenUsage` is `Option`,
  never `unwrap_or(0)`. A zero token count is a claim that zero tokens were used.
- Cost is `Option<Cost>` and only present when a price is *configured* for that model. Orbit-RS
  does not ship a guessed price table that silently goes stale.
- Every fallback that fires is logged and counted. A silent failover is an outage you cannot see.
- Illegal states unrepresentable: a `ModelProfile` cannot exist without a resolvable provider.

### 4.4 Post-implementation position

After M1–M5 (see `AI_LLM_ROADMAP.md`), the row-by-row standing versus LiteLLM — the reference
implementation for this category:

| Capability | LiteLLM | Orbit-RS after M5 |
|---|---|---|
| Provider count | 100+ | 4 native shapes covering ~15 named services |
| Unified API | Yes | Yes |
| Fallback chains | Yes | Yes |
| Retries + backoff | Yes | Yes (+ jitter) |
| Circuit breaker | Yes | Yes |
| Timeouts | Yes | Yes |
| Streaming | Yes | M6 |
| Embeddings | Yes | Yes |
| Cost tracking | Yes (bundled price map) | Yes (configured prices only — honest, not automatic) |
| Semantic caching | Bolt-on Redis + vector store | M7 — **native, over the vector index already in-process** |
| Per-tenant budgets | Yes | M7 |
| **Runs inside the database** | No | **Yes — the whole point** |

The last row is the strategic claim. LiteLLM is a proxy you deploy next to your database.
Orbit-RS's version is a subsystem of the database, which means retrieval, embedding, cache, and
generation share one process, one transaction boundary, and one security boundary. That is the
same argument Oracle and SingleStore are making about inference, applied to the generation half.

---

## 5. Sources

- [SurrealDB 3.0 GA / $23M raise — Tech.eu](https://tech.eu/2026/02/17/surrealdb-secures-23m-and-launches-surrealdb-3-0-to-address-ai-agent-memory-challenges/)
- [SurrealDB 3.0 replaces the five-database RAG stack — VentureBeat](https://venturebeat.com/data/surrealdb-3-0-wants-to-replace-your-five-database-rag-stack-with-one)
- [SurrealDB AI-native multi-model — SiliconANGLE](https://siliconangle.com/2026/02/17/surrealdb-raises-23m-expand-ai-native-multi-model-database/)
- [Loading embedding models into Oracle AI Database (2026)](https://blogs.oracle.com/developers/how-to-load-embedding-models-into-oracle-ai-database-in-2026)
- [Redis vector benchmark vs Aurora pgvector and MongoDB Atlas](https://redis.io/blog/benchmarking-results-for-vector-databases/)
- [Vector database comparison — Zilliz](https://zilliz.com/comparison)
- [Best vector databases 2026 — DataCamp](https://www.datacamp.com/blog/the-top-5-vector-databases)
- [LiteLLM vs Portkey vs OpenRouter — Developers Digest](https://www.developersdigest.tech/blog/llm-router-comparison-2026)
- [Building an LLM router gateway: fallbacks, semantic caching, per-tenant keys, cost tracking](https://www.codersarts.com/post/how-to-build-an-llm-router-gateway-with-litellm-fallbacks-semantic-caching-per-tenant-keys-and-c)
- [Best LLM gateways 2026 — Contabo](https://contabo.com/blog/best-llm-gateways/)
- [rig — Rust LLM/agent framework](https://rig.rs/)
- [genai — Rust multi-provider generative AI client](https://github.com/jeremychone/rust-genai)
- [llm-connector — crates.io](https://crates.io/crates/llm-connector/0.1.0)
