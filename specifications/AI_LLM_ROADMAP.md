# Orbit-RS AI/LLM Roadmap

**Date:** 2026-08-05
**Companion:** [`COMPETITIVE_ANALYSIS.md`](COMPETITIVE_ANALYSIS.md) — the gap analysis this roadmap answers
**Owner:** Core team

---

## 0. Decisions Taken (no further input required)

These were decided unilaterally under an explicit mandate to proceed. Each is recorded with its
reasoning so it can be overturned deliberately rather than by accident.

| # | Decision | Reasoning | Reversal cost |
|---|---|---|---|
| D1 | **Build `orbit/llm` as a new workspace crate** rather than depend on `rig` or `genai` | The value is the router (fallback, breaker, cost, hot-swap) and its integration with `OrbitError`/`tracing`/Prometheus — none of which a third-party crate provides. HTTP shaping over `reqwest` (already a dependency) is the cheap part. | Low — the provider trait is the seam; a `genai`-backed provider could be added behind it |
| D2 | **Four provider shapes, not twenty** — OpenAI, Anthropic, Ollama, OpenAI-compatible | The compatible shape covers Azure, vLLM, Groq, Together, OpenRouter, LM Studio, DeepSeek, Fireworks, and any local server. Chasing a provider count is vanity; the trait makes each new one ~80 LOC. | None — additive |
| D3 | **No bundled model price table** | A price map baked into a database binary goes stale silently and then reports confident wrong costs. Prices are configured per model profile; cost is `None` when unpriced. | None |
| D4 | **`SecretString` for all credentials**, redacting `Debug`/`Display` and emitting a redaction marker from `Serialize` | `LLMProvider` today is `Serialize` with a plain `String` api_key. Config dumps and error paths can print it. | None |
| D5 | **Registry is hot-swappable at runtime** via `RwLock`-guarded snapshot, exposed over RESP `LLM.*` | The explicit ask. Read-mostly access pattern; a write is a config change, a read is every request. | None |
| D6 | **Keep `graphrag::LLMProvider` as a compatibility shim** that converts into an `orbit-llm` profile | It is public API re-exported from `orbit_shared::lib`. Breaking it would ripple through the RESP/Cypher/AQL/Postgres GraphRAG engines for no user benefit. | n/a |
| D7 | **Gate `orbit/ml::industry_models` behind an `experimental-industry-models` feature**, default off *(done)* | 470 stub bodies shipping in a default-on crate is package-level overclaiming (§2.5 of the analysis). Feature-gating is reversible and immediately stops the overclaim. | Low |
| D8 | **Milestones M1–M5 are implemented in this workstream; M6–M8 are specified but not built** | M1–M5 form a coherent shippable unit: provider abstraction → providers → router → integration → control surface. M6+ each depend on M1–M5 landing first. | n/a |

---

## 1. Milestones

### M1 — `orbit-llm` core ✅ *(this workstream)*

The crate skeleton and everything provider-independent.

- `LlmProvider` / `EmbeddingProvider` traits (`async_trait`, object-safe, stored as `Arc<dyn _>`)
- Request/response types: `ChatRequest`, `ChatResponse`, `Message`, `Role`, `TokenUsage`,
  `FinishReason`, `EmbeddingRequest`, `EmbeddingResponse`
- `SecretString` — redacting `Debug`/`Display`, `Serialize` emits a redaction marker, `Deserialize`
  supported so config can carry it
- `LlmError` (`thiserror`, `#[non_exhaustive]`) with a `is_retryable()` classification
- Shared `reqwest::Client` with connection pooling and a bounded timeout
- `RetryPolicy` — exponential backoff with full jitter, retry-budget capped
- `CircuitBreaker` — closed/open/half-open, per-provider
- `ProfileCounters` / `UsageSnapshot` — token, cost, latency, and failure aggregation in atomics;
  cost accumulated in integer micro-dollars so it does not drift; unreported token counts are
  counted separately rather than summed as zero

**Acceptance:** `cargo test -p orbit-llm` green; no `unwrap`/`expect` outside tests; a `SecretString`
round-trips through `Debug` without revealing its contents (test asserts this).

### M2 — Providers ✅ *(this workstream)*

- **OpenAI** — chat completions + embeddings, org/project headers, configurable base URL
- **Anthropic** — Messages API, `system` as a top-level field (not a message), `anthropic-version`
  header, `max_tokens` **required** by the API and therefore non-optional in the profile.
  *Closes the `Err("Anthropic client not yet implemented")` defect.*
- **Ollama** — `/api/chat` + `/api/embed`, honest about not reporting cost
- **OpenAI-compatible** — one implementation, a `flavor` for header/path differences, covering
  Azure OpenAI (`api-key` header + `api-version` query), vLLM, Groq, Together, OpenRouter,
  LM Studio, DeepSeek, Fireworks

**Acceptance:** each provider's request body is asserted by a table-driven test against the shape
the vendor documents (system-message placement, required fields, header names). Every parameter in
the profile appears in the emitted body — verified by test, not by inspection.

### M3 — Registry + Router ✅ *(this workstream)*

- `ModelProfile` — provider + model + params + optional pricing + fallback chain
- `LlmRegistry` — named profiles, a default, `register`/`remove`/`set_default`, all at runtime
- `Router` — the request path: resolve → timeout → retry → breaker → fallback → account
- Config layering: `LLM_*` env vars over `[llm]` TOML, per 12-factor III
- Counters surfaced through `LLM.STATS`: requests, failures, fallbacks fired and used, tokens,
  cost, mean latency, breaker state

> **Not built:** these counters are *not* registered with the `metrics` crate, so they do not
> appear on the Prometheus endpoint yet. `LLM.STATS` is the only way to read them. Wiring them to
> `orbit-server-prometheus` is a small follow-up, listed here rather than claimed as done.

**Acceptance:** tests cover — fallback fires on primary failure and is *counted*; breaker opens
after threshold and rejects fast; retry respects the budget; a non-retryable error (401) does not
retry; env overrides TOML; registry swap is visible to an in-flight-adjacent read.

### M4 — GraphRAG integration ✅ *(this workstream)*

- `graphrag/llm_client.rs` becomes a thin adapter over `orbit-llm`
- `orbit_shared::graphrag::LLMProvider` gains `TryFrom` → `ModelProfile` (D6)
- The double-`match` parameter re-extraction in `graph_rag_actor.rs` is deleted — the profile
  carries its own parameters
- `[llm]` section added to `config/orbit-server.toml` with documented env overrides
- The inline `std::env::var("OPENAI_API_KEY")` + hardcoded `"gpt-4"` in `resp/commands/graphrag.rs`
  is replaced by registry lookup

**Acceptance:** GraphRAG RAG query runs end-to-end against Ollama with no code change from the
pre-existing path; Anthropic now works where it previously returned an error.

### M5 — `LLM.*` control surface ✅ *(this workstream)*

Runtime switchability, exposed over RESP (the protocol with the cleanest command-module structure):

| Command | Effect |
|---|---|
| `LLM.PROVIDERS` | List provider shapes the build supports |
| `LLM.MODELS` | List registered profiles, marking the default |
| `LLM.INFO <profile>` | Full profile detail, secrets redacted |
| `LLM.REGISTER <profile> <provider> <model> [KEY v]…` | Add/replace a profile at runtime |
| `LLM.UNREGISTER <profile>` | Remove a profile |
| `LLM.USE <profile>` | **Switch the default model with no restart** |
| `LLM.GENERATE <prompt> [MODEL p] [SYSTEM s] [MAXTOKENS n] [TEMPERATURE t]` | One-shot generation |
| `LLM.EMBED <text...> [MODEL p]` | Embeddings |
| `LLM.STATS [profile]` | Requests, failures, fallbacks, tokens, cost, breaker state |

**Acceptance:** a live session demonstrates registering a second profile, switching to it with
`LLM.USE`, and seeing `LLM.STATS` attribute the next generation to the new profile — all without
restarting the server. **Verified**, see §4.2.

### M6 — Streaming *(specified, not built)*

`generate_stream` returning `impl Stream<Item = Result<ChatChunk>>`; SSE parsing for OpenAI and
Anthropic (different event shapes), NDJSON for Ollama. Surfaces: RESP push, HTTP SSE at
`/v1/llm/stream`, gRPC server-streaming. Blocked on M1–M3.

### M7 — Semantic cache + budgets *(specified, not built)*

The highest-leverage item in the analysis (§3.3). Cache key = embedding of the normalized prompt +
profile identity; lookup is a similarity query against the **existing in-process HNSW index** with a
configurable threshold and TTL. Bounded by entry count *and* byte size — an unbounded cache is the
`CLAUDE.md` "cache that never evicts" antipattern. Plus per-tenant API keys, rate limits, and spend
budgets that reject rather than silently exceed.

**Why this matters competitively:** LiteLLM needs Redis + an external vector store for semantic
caching. Orbit-RS has both in-process. This is a differentiator available for the cost of a query.

### M8 — Auto-embedding on write *(specified, not built)*

`ALTER TABLE t ADD EMBEDDING col USING <profile> FROM (expr)`. On insert/update, the row's embedding
is generated through the M1–M3 stack and indexed transactionally with the row. This is the Weaviate
vectorizer feature, and after M1–M3 it is mostly plumbing. Requires M1–M3 + batching (embedding one
row per HTTP call is not viable — batch by transaction).

---

## 2. Sequencing

```
M1 core ──▶ M2 providers ──▶ M3 registry+router ──┬──▶ M4 graphrag ──▶ M5 LLM.* surface
                                                   ├──▶ M6 streaming
                                                   ├──▶ M7 semantic cache + budgets
                                                   └──▶ M8 auto-embedding on write
```

M1→M5 are strictly sequential (each consumes the last). M6/M7/M8 are independent of each other and
can be parallelized once M3 lands.

Out of this workstream but tracked in `COMPETITIVE_ANALYSIS.md` §3: vector quantization and
pre-filtered search (§3.4), managed cloud (§3.5), `orbit/ml` remediation (§3.6, partially addressed
by D7), and benchmarking `orbit/compute` (§3.7).

---

## 3. Non-Goals

- **Not** chasing a provider count. Four shapes, ~15 services. Adding a fifteenth is ~80 LOC when
  someone actually needs it.
- **Not** an agent framework. Orbit-RS is the memory and retrieval layer agents call, not the loop.
  This is the boundary that makes `rig` the wrong dependency (D1).
- **Not** a bundled price table (D3).
- **Not** Prometheus-exported LLM metrics yet — see the note under M3. Deliberately deferred, not
  overlooked.
- **Not** fine-tuning or training orchestration. `orbit/ml` has not earned more surface area (D7).

---

## 4. Verification — What Was Actually Run

Per `CLAUDE.md` → *Verification*: a green build proves almost nothing. Executed in yield order,
2026-08-05.

### 4.1 Automated

| Step | Result |
|---|---|
| `cargo test -p orbit-llm` | **160 passed**, 0 failed (+ 2 doc-tests) |
| `cargo test -p orbit-server --lib` | **1706 passed**, 0 failed, 55 ignored |
| `cargo test -p orbit-ml --lib` | **78 passed**, 0 failed |
| `make check` (clippy `-D warnings`) | **clean** — zero findings in any new or modified file |
| `make format` | applied |

### 4.2 Run it — live session against a real provider

The server was started on isolated ports with a live Ollama daemon and driven over the Redis wire
protocol. (`redis-cli` is not installed on this machine; a minimal RESP client was used instead.)

| Claim | How it was verified | Result |
|---|---|---|
| Generation works end to end | `LLM.GENERATE` against `llama3.2` | `"The sky appears blue on a clear day."`, 52 tokens, 10.1s |
| **Model switching needs no restart** | `LLM.REGISTER granite …` → `LLM.USE granite` → `LLM.GENERATE` | answered by `granite4.1:3b`; `LLM.MODELS` showed the default moved |
| Embeddings work | `LLM.EMBED` two inputs via `nomic-embed-text` | 2 vectors, 768 dimensions, batched in one call |
| Fallback fires **and is visible** | primary pointed at a dead port with `FALLBACKS granite` | served by `granite`, response carried `fallbacks_used: ["broken"]` |
| Failover is attributed to both sides | `LLM.STATS` after the above | `broken`: `failures=1, fallbacks_fired=1`; `granite`: `fallback_uses=1` |
| Breaker opens and costs nothing | 6 consecutive failures against a dead endpoint | breaker `open`; subsequent calls rejected instantly without dialling |
| GraphRAG uses the switchable registry | `GRAPHRAG.QUERY` after `LLM.USE granite` | real LLM response, served by the newly selected profile |

### 4.3 Reconciled against an external reference

The Anthropic path was probed against the **real** `api.anthropic.com` with an invalid key:

```text
ERR anthropic returned HTTP 401:
{"type":"error","error":{"type":"authentication_error","message":"invalid x-api-key"},...}
```

An `authentication_error` naming `x-api-key` — rather than a 404, a 400, or a version complaint —
confirms the URL, the `x-api-key` header, and the `anthropic-version` header are all correct
against the live API, not merely against our own expectation of it. The 401 also correctly did
**not** retry and did **not** trip the breaker.

### 4.4 What verification found that the tests did not

Running it surfaced a defect the green build hid:

> **`[llm] enabled = false` was ignored.** The bootstrap registered every profile in the config
> file regardless of the flag. A kill switch that changes nothing is the exact "decorative
> parameter" this document warns about in D3's neighbourhood, shipped by the author of the warning.
> Fixed in `server/src/llm/mod.rs::register_config_profiles`, with a test
> (`the_enabled_flag_actually_gates_registration`) asserting a disabled section registers nothing.

This is the argument for §4.2 in one bullet: the unit tests were green, clippy was clean, and the
flag did not work.

### 4.5 Affordance audit

- Every `ProviderKind` variant has a construction site in `providers::build_provider`, asserted by
  `every_provider_kind_is_constructible`, which also asserts the case count equals
  `ProviderKind::all().len()` — so adding a variant without wiring it fails the test.
- Every `LLMProvider` legacy variant converts, asserted by
  `every_legacy_variant_has_a_conventional_name_and_converts`.
- Every command in `SUPPORTED` appears in the `handle` dispatch, asserted by
  `every_supported_command_is_reachable_from_dispatch`.
- `orbit-ml`'s industry scaffolding is off by default, asserted by
  `industry_scaffolding_is_off_by_default` (D7).
