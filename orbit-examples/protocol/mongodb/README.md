# MongoDB Protocol Examples for Orbit-RS

This directory contains comprehensive examples demonstrating MongoDB wire protocol support in Orbit-RS.

## Overview

Orbit-RS implements the MongoDB wire protocol, allowing you to use standard MongoDB clients and drivers to interact with the Orbit database. All MongoDB operations are backed by Orbit's unified storage engine with RocksDB persistence.

## Quick Start

### Prerequisites

1. **Start Orbit Server** with MongoDB protocol enabled (default port 27017):
   ```bash
   cargo run --bin orbit-server
   ```

2. **Install MongoDB Client Tools**:
   ```bash
   # MongoDB Shell (mongosh)
   brew install mongosh  # macOS
   
   # Python driver
   pip install pymongo
   
   # Node.js driver
   npm install mongodb
   ```

### Basic Connection

```bash
# Connect with mongosh
mongosh mongodb://localhost:27017

# Or specify database
mongosh mongodb://localhost:27017/mydb
```

## Examples Structure

```
mongodb/
├── README.md                          # This file
├── 01_basic_crud.js                   # Basic CRUD operations
├── 02_aggregation_pipeline.js         # Aggregation framework
├── 03_ml_integration.js               # ML functions via MongoDB
├── 04_transactions.js                 # Multi-document transactions
├── 05_indexes.js                      # Index creation and management
├── 06_change_streams.js               # Real-time change streams
├── python/
│   ├── mongodb_examples.py            # PyMongo basic examples
│   ├── mongodb_ml.py                  # ML integration with PyMongo
│   └── mongodb_advanced.py            # Advanced patterns
├── scenarios/
│   ├── healthcare_scenario.js         # Healthcare use case
│   ├── ecommerce_scenario.js          # E-commerce use case
│   └── iot_scenario.js                # IoT sensor data use case
└── QUICK_START.md                     # Quick start guide

```

## Running Examples

### JavaScript Examples (mongosh)

```bash
# Run with mongosh
mongosh mongodb://localhost:27017 --file 01_basic_crud.js

# Or load in interactive shell
mongosh mongodb://localhost:27017
> load('01_basic_crud.js')
```

### Python Examples

```bash
cd python
python mongodb_examples.py
python mongodb_ml.py
```

## Key Features Demonstrated

### 1. Document CRUD Operations
- Insert documents (insertOne, insertMany)
- Query documents (find, findOne)
- Update documents (updateOne, updateMany)
- Delete documents (deleteOne, deleteMany)
- Upsert operations

### 2. Aggregation Pipeline
- $match, $group, $project stages
- $lookup for joins
- $unwind for array processing
- $sort, $limit, $skip
- Aggregation expressions (34+ operators)

### 3. ML Integration
- ML_PREDICT() for model inference
- ML_EMBED_TEXT() for text embeddings
- ML_TRAIN_MODEL() for training
- Vector similarity search

### 4. Transactions
- Multi-document ACID transactions
- Session management
- Commit and abort operations

### 5. Indexes
- Single field indexes
- Compound indexes
- Text indexes
- Geospatial indexes
- Vector indexes for similarity search

### 6. Change Streams
- Real-time change notifications
- Watch collections for updates
- Resume tokens for reliability

## MongoDB vs Other Protocols

**When to use MongoDB protocol:**
- Working with document-oriented data
- Need flexible schema
- Using existing MongoDB applications
- Require aggregation pipeline
- Building with JavaScript/Node.js ecosystem

**Cross-Protocol Access:**
```javascript
// Write via MongoDB
db.products.insertOne({
  name: "Laptop",
  price: 999,
  embedding: [0.1, 0.2, 0.3]
})

// Read via PostgreSQL
psql> SELECT * FROM products WHERE name = 'Laptop';

// Read via Redis
redis-cli> HGETALL product:1

// Query via REST
curl http://localhost:8080/api/products?name=Laptop
```

## ML Function Examples

### Text Embeddings
```javascript
db.documents.insertOne({
  content: "Machine learning with Orbit-RS",
  embedding: ML_EMBED_TEXT("Machine learning with Orbit-RS", "sentence-transformers")
})
```

### Model Prediction
```javascript
db.customers.aggregate([
  {
    $project: {
      customer_id: 1,
      churn_risk: {
        $function: {
          body: "ML_PREDICT('churn_model', [tenure, monthly_charges])",
          args: ["$tenure", "$monthly_charges"],
          lang: "sql"
        }
      }
    }
  }
])
```

### Vector Similarity Search
```javascript
const queryEmbedding = [0.1, 0.2, 0.3, 0.4];

db.products.aggregate([
  {
    $addFields: {
      similarity: {
        $vectorDistance: {
          vector1: "$embedding",
          vector2: queryEmbedding,
          metric: "cosine"
        }
      }
    }
  },
  { $sort: { similarity: 1 } },
  { $limit: 5 }
])
```

## Performance Tips

1. **Create Indexes**: Index frequently queried fields
   ```javascript
   db.products.createIndex({ name: 1 })
   db.products.createIndex({ category: 1, price: -1 })
   ```

2. **Use Projection**: Only fetch needed fields
   ```javascript
   db.products.find({}, { name: 1, price: 1, _id: 0 })
   ```

3. **Batch Operations**: Use insertMany for bulk inserts
   ```javascript
   db.products.insertMany(documents, { ordered: false })
   ```

4. **Aggregation Pipeline**: Optimize pipeline order
   ```javascript
   // Good: Filter early
   db.products.aggregate([
     { $match: { price: { $gt: 100 } } },
     { $group: { _id: "$category", avg: { $avg: "$price" } } }
   ])
   ```

## Troubleshooting

### Connection Issues
```bash
# Check if MongoDB port is listening
lsof -i :27017

# Test connection
mongosh mongodb://localhost:27017 --eval "db.runCommand({ ping: 1 })"
```

### Authentication
```javascript
// Connect with authentication (if enabled)
mongosh mongodb://username:password@localhost:27017/mydb
```

### Error Handling
```javascript
try {
  db.products.insertOne({ name: "Test" })
} catch (e) {
  print("Error:", e.message)
}
```

## See Also

- [MongoDB Wire Protocol Implementation](../../orbit/server/src/protocols/mongodb/)
- [Cross-Protocol Examples](../cross-protocol/)
- [ML Protocol Integration](../ml-protocol-examples/)
- [MongoDB Official Documentation](https://docs.mongodb.com/)

## Contributing

To add new MongoDB examples:
1. Follow the existing file naming convention
2. Include comprehensive comments
3. Add error handling
4. Test against running Orbit server
5. Update this README
