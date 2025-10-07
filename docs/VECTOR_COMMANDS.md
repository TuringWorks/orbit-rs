# Vector Commands Documentation

This document provides comprehensive documentation for all vector storage and search commands implemented in Orbit-RS. The implementation provides both **VECTOR.*** commands and **FT.*** (RedisSearch-compatible) commands for maximum compatibility.

## 🚀 Features

- **Full Redis Protocol Compatibility**: All commands work with standard Redis clients
- **Multiple Similarity Metrics**: Cosine, Euclidean, Dot Product, and Manhattan distance
- **Metadata Support**: Store and filter vectors by key-value metadata
- **High-Dimensional Vectors**: Support for vectors of any dimension
- **Efficient Search**: K-nearest neighbors and similarity search with thresholds
- **RedisSearch Compatibility**: FT.* commands for existing RedisSearch applications

## 📋 VECTOR.* Commands

### Basic Vector Operations

#### VECTOR.ADD
Add a vector to an index with optional metadata.

**Syntax:**
```
VECTOR.ADD <index> <id> <vector> [key value ...]
```

**Parameters:**
- `index`: Name of the vector index
- `id`: Unique identifier for the vector
- `vector`: Comma-separated float values (e.g., "1.0,2.0,3.0")
- `key value`: Optional metadata key-value pairs

**Examples:**
```redis
# Add a simple vector
VECTOR.ADD my-index doc1 "0.1,0.2,0.3,0.4"

# Add vector with metadata
VECTOR.ADD my-index doc2 "0.5,0.6,0.7,0.8" title "Document 2" category "AI" author "John Doe"
```

**Returns:** `OK` on success

---

#### VECTOR.GET
Retrieve a vector and its metadata by ID.

**Syntax:**
```
VECTOR.GET <index> <id>
```

**Examples:**
```redis
VECTOR.GET my-index doc1
```

**Returns:** Array containing `[id, vector_data, key1, value1, key2, value2, ...]` or `null` if not found

---

#### VECTOR.DEL
Delete a vector from an index.

**Syntax:**
```
VECTOR.DEL <index> <id>
```

**Examples:**
```redis
VECTOR.DEL my-index doc1
```

**Returns:** `1` if vector was deleted, `0` if not found

---

#### VECTOR.COUNT
Get the number of vectors in an index.

**Syntax:**
```
VECTOR.COUNT <index>
```

**Examples:**
```redis
VECTOR.COUNT my-index
```

**Returns:** Integer count of vectors

---

#### VECTOR.LIST
List all vector IDs in an index.

**Syntax:**
```
VECTOR.LIST <index>
```

**Examples:**
```redis
VECTOR.LIST my-index
```

**Returns:** Array of vector IDs

---

#### VECTOR.STATS
Get statistics about a vector index.

**Syntax:**
```
VECTOR.STATS <index>
```

**Examples:**
```redis
VECTOR.STATS my-index
```

**Returns:** Array with statistics:
```
[
  "vector_count", 150,
  "index_count", 1,
  "avg_dimension", "384.00",
  "min_dimension", 384,
  "max_dimension", 384,
  "avg_metadata_keys", "2.50"
]
```

### Vector Search Operations

#### VECTOR.SEARCH
Perform similarity search with advanced filtering options.

**Syntax:**
```
VECTOR.SEARCH <index> <vector> <limit> [METRIC <metric>] [THRESHOLD <threshold>] [key value ...]
```

**Parameters:**
- `index`: Name of the vector index
- `vector`: Query vector as comma-separated floats
- `limit`: Maximum number of results to return
- `METRIC`: Similarity metric (COSINE, EUCLIDEAN, DOT, MANHATTAN)
- `THRESHOLD`: Minimum similarity threshold
- `key value`: Metadata filters (only vectors matching all filters are returned)

**Examples:**
```redis
# Basic similarity search
VECTOR.SEARCH my-index "0.1,0.2,0.3,0.4" 5

# Search with cosine similarity
VECTOR.SEARCH my-index "0.1,0.2,0.3,0.4" 10 METRIC COSINE

# Search with threshold
VECTOR.SEARCH my-index "0.1,0.2,0.3,0.4" 5 METRIC COSINE THRESHOLD 0.8

# Search with metadata filters
VECTOR.SEARCH my-index "0.1,0.2,0.3,0.4" 5 METRIC COSINE category AI author "John Doe"
```

**Returns:** Array of results, each containing `[id, score, vector_data, metadata...]`

---

#### VECTOR.KNN
Perform k-nearest neighbors search.

**Syntax:**
```
VECTOR.KNN <index> <vector> <k> [METRIC <metric>]
```

**Parameters:**
- `index`: Name of the vector index
- `vector`: Query vector as comma-separated floats
- `k`: Number of nearest neighbors to return
- `METRIC`: Similarity metric (optional, defaults to COSINE)

**Examples:**
```redis
# Find 5 nearest neighbors
VECTOR.KNN my-index "0.1,0.2,0.3,0.4" 5

# KNN with specific metric
VECTOR.KNN my-index "0.1,0.2,0.3,0.4" 3 EUCLIDEAN
```

**Returns:** Array of results, each containing `[id, score]`

## 🚀 FT.* Commands (RedisSearch Compatible)

### Index Management

#### FT.CREATE
Create a vector index with specified dimension and distance metric.

**Syntax:**
```
FT.CREATE <index> DIM <dimension> [DISTANCE_METRIC <metric>]
```

**Parameters:**
- `index`: Name of the index to create
- `DIM`: Vector dimension (required)
- `DISTANCE_METRIC`: Similarity metric (COSINE, EUCLIDEAN, DOT, MANHATTAN)

**Examples:**
```redis
# Create index with default cosine similarity
FT.CREATE my-ft-index DIM 384

# Create index with Euclidean distance
FT.CREATE my-ft-index DIM 384 DISTANCE_METRIC EUCLIDEAN
```

**Returns:** `OK` on success

---

#### FT.INFO
Get information about a vector index.

**Syntax:**
```
FT.INFO <index>
```

**Examples:**
```redis
FT.INFO my-ft-index
```

**Returns:** Array with index information including vector count, dimensions, and index configurations

### Vector Operations

#### FT.ADD
Add a vector to an index (equivalent to VECTOR.ADD).

**Syntax:**
```
FT.ADD <index> <id> <vector> [key value ...]
```

**Examples:**
```redis
FT.ADD my-ft-index doc1 "0.1,0.2,0.3" title "Document 1"
```

**Returns:** `OK` on success

---

#### FT.DEL
Delete a vector from an index (equivalent to VECTOR.DEL).

**Syntax:**
```
FT.DEL <index> <id>
```

**Examples:**
```redis
FT.DEL my-ft-index doc1
```

**Returns:** `1` if deleted, `0` if not found

---

#### FT.SEARCH
Search for similar vectors (equivalent to VECTOR.SEARCH with DISTANCE_METRIC parameter).

**Syntax:**
```
FT.SEARCH <index> <vector> <limit> [DISTANCE_METRIC <metric>] [key value ...]
```

**Examples:**
```redis
FT.SEARCH my-ft-index "0.1,0.2,0.3" 5 DISTANCE_METRIC COSINE
```

**Returns:** Array of search results

## 📊 Similarity Metrics

| Metric | Aliases | Description | Use Case |
|--------|---------|-------------|----------|
| `COSINE` | `COS` | Cosine similarity (normalized dot product) | Text embeddings, semantic similarity |
| `EUCLIDEAN` | `L2`, `EUCL` | Euclidean distance (L2 norm) | Spatial data, image features |
| `DOT` | `DOTPRODUCT`, `IP`, `INNER` | Dot product similarity | When magnitude matters |
| `MANHATTAN` | `L1`, `MAN` | Manhattan distance (L1 norm) | High-dimensional sparse data |

## 🔄 Vector Data Formats

Vectors can be provided in multiple formats:

### String Format (Recommended)
```redis
VECTOR.ADD my-index doc1 "0.1,0.2,0.3,0.4,0.5"
```

### Array Format
```redis
VECTOR.ADD my-index doc1 ["0.1", "0.2", "0.3", "0.4", "0.5"]
```

## 📈 Performance Tips

1. **Use appropriate dimensions**: Higher dimensions provide more precision but slower search
2. **Choose the right metric**: 
   - Use COSINE for normalized vectors (embeddings)
   - Use EUCLIDEAN for coordinate data
   - Use DOT when magnitude is important
3. **Leverage metadata filtering**: Pre-filter by metadata before vector similarity
4. **Set reasonable thresholds**: Avoid returning too many low-similarity results

## 🛠️ Usage Examples

### Complete Workflow Example

```redis
# 1. Create vectors with metadata
VECTOR.ADD ml-embeddings doc1 "0.1,0.2,0.3,0.4" title "Machine Learning Basics" category "AI" difficulty "beginner"
VECTOR.ADD ml-embeddings doc2 "0.2,0.3,0.4,0.5" title "Deep Learning Advanced" category "AI" difficulty "advanced"  
VECTOR.ADD ml-embeddings doc3 "0.8,0.7,0.6,0.5" title "Statistics Fundamentals" category "Math" difficulty "intermediate"

# 2. Get vector count and statistics
VECTOR.COUNT ml-embeddings
VECTOR.STATS ml-embeddings

# 3. Search for similar vectors
VECTOR.SEARCH ml-embeddings "0.15,0.25,0.35,0.45" 5 METRIC COSINE THRESHOLD 0.7

# 4. Search with metadata filtering (only AI category)
VECTOR.SEARCH ml-embeddings "0.15,0.25,0.35,0.45" 3 METRIC COSINE category AI

# 5. Find nearest neighbors
VECTOR.KNN ml-embeddings "0.15,0.25,0.35,0.45" 3

# 6. Get specific vector
VECTOR.GET ml-embeddings doc1

# 7. List all vectors
VECTOR.LIST ml-embeddings

# 8. Delete vector
VECTOR.DEL ml-embeddings doc3
```

### FT Commands Example

```redis
# 1. Create FT index
FT.CREATE semantic-search DIM 384 DISTANCE_METRIC COSINE

# 2. Add vectors
FT.ADD semantic-search embedding1 "0.1,0.2,..." title "Document Title"

# 3. Search 
FT.SEARCH semantic-search "0.1,0.2,..." 10 DISTANCE_METRIC COSINE

# 4. Get index info
FT.INFO semantic-search
```

## 🔧 Integration with Applications

### Python Example
```python
import redis

# Connect to Orbit vector store
r = redis.Redis(host='127.0.0.1', port=6381)

# Add vector with metadata
r.execute_command('VECTOR.ADD', 'my-index', 'doc1', 
                  '0.1,0.2,0.3,0.4', 
                  'title', 'My Document',
                  'category', 'AI')

# Search for similar vectors
results = r.execute_command('VECTOR.SEARCH', 'my-index', 
                           '0.1,0.2,0.3,0.4', 5, 
                           'METRIC', 'COSINE',
                           'THRESHOLD', '0.8')
```

### Node.js Example
```javascript
const redis = require('redis');
const client = redis.createClient({host: '127.0.0.1', port: 6381});

// Add vector
await client.sendCommand(['VECTOR.ADD', 'my-index', 'doc1', 
                         '0.1,0.2,0.3,0.4', 
                         'title', 'My Document']);

// Search vectors
const results = await client.sendCommand([
  'VECTOR.SEARCH', 'my-index', '0.1,0.2,0.3,0.4', '5'
]);
```

## 🚨 Error Handling

Common error responses:

- `ERR wrong number of arguments`: Incorrect command syntax
- `ERR invalid vector format`: Vector data not properly formatted
- `ERR unknown similarity metric`: Invalid metric name
- `ERR limit must be positive`: Invalid limit value
- `ERR actor error`: Internal server error

## 🎯 Advanced Use Cases

### Semantic Search
```redis
# Store document embeddings
VECTOR.ADD docs doc1 "..." title "Introduction to AI" tags "ai,ml,intro"
VECTOR.ADD docs doc2 "..." title "Deep Learning" tags "ai,dl,advanced"

# Find semantically similar documents
VECTOR.SEARCH docs "query_embedding..." 10 METRIC COSINE THRESHOLD 0.7
```

### Image Similarity
```redis
# Store image feature vectors
VECTOR.ADD images img1 "..." filename "photo1.jpg" category "landscape"
VECTOR.ADD images img2 "..." filename "photo2.jpg" category "portrait"

# Find similar images
VECTOR.SEARCH images "query_features..." 5 METRIC EUCLIDEAN
```

### Recommendation System
```redis
# Store user preference vectors
VECTOR.ADD users user123 "..." age "25" location "NYC"

# Find similar users
VECTOR.KNN users "user_profile..." 10 COSINE
```

This implementation provides production-ready vector storage and search capabilities with full Redis protocol compatibility, making it easy to integrate into existing applications and workflows.