# Vector Store Example

This example demonstrates using Orbit-RS as a **Redis-compatible vector database** with high-performance similarity search capabilities. The example implements both Redis FT.* (full-text search) commands and simplified VECTOR.* commands for vector operations.

## Features

- 🧮 **High-dimensional vector storage and retrieval**
- 🔍 **Multiple similarity metrics** (Cosine, Euclidean, Dot Product, Manhattan)
- 📊 **Metadata filtering** for advanced search
- ⚡ **Redis protocol compatibility** - use with `redis-cli` or any Redis client
- 🎯 **K-nearest neighbors (KNN) search**
- 📈 **Real-time statistics** and monitoring

## Quick Start

1. **Start the Orbit server** (in another terminal):
   ```bash
   cargo run --bin orbit-server
   ```

2. **Run the vector store example**:
   ```bash
   cargo run --example vector-store
   ```

3. **Connect with redis-cli**:
   ```bash
   redis-cli -h 127.0.0.1 -p 6381
   ```

## Supported Commands

### Basic Vector Operations (VECTOR.* commands)

```redis

# Add a vector with ID
VECTOR.ADD my-vectors doc6 "0.1,0.2,0.3,0.4,..."

# Get a vector by ID
VECTOR.GET my-vectors doc1

# Delete a vector
VECTOR.DEL my-vectors doc1

# Get vector store statistics
VECTOR.STATS my-vectors

# Search for similar vectors (returns top 5 matches)
VECTOR.SEARCH ml-embeddings "0.1,0.2,0.3,..." 5

# K-nearest neighbors search
VECTOR.KNN ml-embeddings "0.1,0.2,0.3,..." 3
```

### Advanced Redis FT.* Commands

```redis

# Create a vector index
FT.CREATE my-index DIM 384 DISTANCE_METRIC COSINE

# Add vector with metadata
FT.ADD my-index vec1 "0.1,0.2,0.3,..." title "My Document" category "AI"

# Advanced search with metadata filtering
FT.SEARCH my-index "0.1,0.2,0.3,..." 10 DISTANCE_METRIC COSINE

# Get index information
FT.INFO my-index

# Delete a vector from index
FT.DEL my-index vec1
```

## Sample Data

The example includes pre-loaded sample vectors representing ML document embeddings:

- **doc1**: "Introduction to Machine Learning" (AI category)
- **doc2**: "Deep Learning Fundamentals" (AI category) 
- **doc3**: "Neural Network Architectures" (AI category)
- **doc4**: "Computer Vision Applications" (CV category)
- **doc5**: "Natural Language Processing" (NLP category)

Each vector is 384-dimensional (typical for sentence transformers) with metadata including title, category, and author.

## Similarity Metrics

| Metric | Description | Use Case |
|--------|-------------|----------|
| **COSINE** | Cosine similarity (default) | Best for normalized embeddings, semantic similarity |
| **L2** | Euclidean distance | Good for continuous features |
| **IP** | Inner product (dot product) | Fast similarity for normalized vectors |
| **L1** | Manhattan distance | Robust to outliers |

## Example Queries

```redis

# Try this with the pre-loaded data
127.0.0.1:6381> VECTOR.SEARCH ml-embeddings "0.150,0.152,0.148,..." 3
1) 1) "doc2"
   2) "0.98543"
2) 1) "doc1" 
   2) "0.97621"
3) 1) "doc3"
   2) "0.94328"

127.0.0.1:6381> VECTOR.STATS ml-embeddings
1) "vector_count"
2) (integer) 5
3) "index_count"  
4) (integer) 1
5) "avg_dimension"
6) "384.0"
```

## Architecture

- **VectorActor**: Core actor managing vector storage and similarity calculations
- **RESP Server**: Redis protocol compatibility layer
- **Command Handler**: Processes Redis commands and routes to vector operations
- **Similarity Engine**: Optimized vector distance calculations

## Use Cases

- **Semantic Search**: Find documents similar to a query
- **Recommendation Systems**: Product/content recommendations
- **RAG (Retrieval-Augmented Generation)**: Vector database for LLM applications
- **Image Search**: Visual similarity matching
- **Anomaly Detection**: Find outliers in high-dimensional data

## Performance

The vector store is optimized for:
- ✅ **Fast similarity calculations** using optimized distance functions
- ✅ **Concurrent operations** via Orbit's actor system
- ✅ **Metadata filtering** with HashMap-based indexing
- ✅ **Memory-efficient** storage with f32 precision

## Integration

Use with popular embedding models:
- **Sentence Transformers** (384/768 dim)
- **OpenAI Embeddings** (1536 dim) 
- **Cohere Embeddings** (4096 dim)
- **Custom neural network embeddings**

Connect from any Redis client library:
- Python: `redis-py`
- Node.js: `node_redis`
- Java: `Jedis`
- Go: `go-redis`
- Rust: `redis-rs`