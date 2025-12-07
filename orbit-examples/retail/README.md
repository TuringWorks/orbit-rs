# Retail, CPG, Warehouse & Fast Fashion Examples - OrbitRS

## Overview

Comprehensive retail industry examples demonstrating OrbitRS's multi-protocol capabilities for e-commerce, inventory management, warehouse operations, fast fashion, CPG, marketing, and audience management.

## Architecture

```text
┌─────────────────────────────────────────────────────────────────────-──┐
│              Retail & E-Commerce Platform on OrbitRS                   │
├──────────────────────────────────────────────────────────────────────-─┤
│                                                                        │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐  ┌────────────┐  │
│  │  PostgreSQL  │  │    Redis     │  │   MongoDB    │  │   Neo4j    │  │
│  │   :5432      │  │    :6379     │  │   :27017     │  │   :7687    │  │
│  ├──────────────┤  ├──────────────┤  ├──────────────┤  ├────────────┤  │
│  │ Products     │  │ Shopping Cart│  │ Images       │  │ Recommend. │  │
│  │ Inventory    │  │ Sessions     │  │ Reviews      │  │ Purchased  │  │
│  │ Orders       │  │ Flash Sales  │  │ Receipts     │  │ Together   │  │
│  │ Customers    │  │ Real-time    │  │ Marketing    │  │ Influencer │  │
│  │ Pricing      │  │ Inventory    │  │ Analytics    │  │ Networks   │  │
│  │ Stores       │  │ Segments     │  │ Content      │  │ Supply     │  │
│  └──────────────┘  └──────────────┘  └──────────────┘  └────────────┘  │
│                                                                        │
│  ┌──────────────┐  ┌──────────────────────────────────────────────┐    │
│  │  Cassandra   │  │          Workflows & Integration             │    │
│  │   :9042      │  ├──────────────────────────────────────────────┤    │
│  ├──────────────┤  │ • Order Processing Pipeline                  │    │
│  │ Sales Trends │  │ • Inventory Replenishment                    │    │
│  │ Inventory    │  │ • Fast Fashion Collection Launch             │    │
│  │ Turnover     │  │ • Personalized Marketing                     │    │
│  │ Customer     │  │ • Omnichannel Fulfillment                    │    │
│  │ Behavior     │  │ • Recommendation Engine                      │    │
│  │ Campaign KPIs│  │ • Dynamic Pricing                            │    │
│  └──────────────┘  └──────────────────────────────────────────────┘    │
│                                                                        │
└────────────────────────────────────────────────────────────────────────┘
```

## Protocol Usage by Domain

| Domain | PostgreSQL | Redis | MongoDB | Neo4j | Cassandra |
|--------|-----------|-------|---------|-------|-----------|
| **Products** | ✓ Catalog | ✓ Cache | ✓ Images | ✓ Related | ✓ Views |
| **Inventory** | ✓ Stock levels | ✓ Real-time | - | - | ✓ Turnover |
| **Orders** | ✓ Transactions | ✓ Cart | ✓ Receipts | - | ✓ History |
| **Customers** | ✓ Profiles | ✓ Sessions | ✓ Reviews | ✓ Social | ✓ Behavior |
| **Pricing** | ✓ Base prices | ✓ Dynamic | - | - | ✓ History |
| **Marketing** | ✓ Campaigns | ✓ Segments | ✓ Content | ✓ Influence | ✓ Analytics |
| **Warehouse** | ✓ Locations | ✓ Picking | ✓ Logs | ✓ Routes | ✓ Efficiency |
| **Fast Fashion** | ✓ Collections | ✓ Drops | ✓ Lookbooks | ✓ Trends | ✓ Velocity |

## Key Use Cases

### 1. Order Processing Pipeline
**Workflow**: Customer places order → inventory check → payment → fulfillment

**Protocols**:
- **PostgreSQL**: Order record, inventory reservation
- **Redis**: Cart management, real-time stock
- **MongoDB**: Order receipt, shipping label
- **Cassandra**: Order history, analytics

**Performance**: <100ms order placement

### 2. Fast Fashion Collection Launch
**Workflow**: New collection drop with limited inventory

**Protocols**:
- **PostgreSQL**: Product catalog, initial stock
- **Redis**: Flash sale countdown, real-time inventory
- **MongoDB**: Lookbook images, styling guides
- **Neo4j**: Trending items, influencer recommendations
- **Cassandra**: Sales velocity tracking

**Scale**: Handle 100K+ concurrent shoppers

### 3. Personalized Marketing
**Workflow**: Targeted campaigns based on behavior

**Protocols**:
- **PostgreSQL**: Customer segments, campaign rules
- **Redis**: Real-time audience targeting
- **MongoDB**: Email templates, creative assets
- **Neo4j**: Purchase patterns, lookalike audiences
- **Cassandra**: Campaign performance metrics

### 4. Omnichannel Fulfillment
**Workflow**: Buy online, pickup in store (BOPIS)

**Protocols**:
- **PostgreSQL**: Order, store inventory
- **Redis**: Store availability cache
- **MongoDB**: Pickup instructions
- **Neo4j**: Store network, optimal pickup location

## Data Models

### Product (PostgreSQL)
```sql
CREATE TABLE products (
    product_id UUID PRIMARY KEY,
    sku VARCHAR(50) UNIQUE,
    name VARCHAR(500),
    category_id UUID,
    brand VARCHAR(100),
    price DECIMAL(10, 2),
    cost DECIMAL(10, 2),
    status VARCHAR(20) -- ACTIVE, DISCONTINUED, SEASONAL
);
```

### Shopping Cart (Redis)
```redis
HSET cart:user-123 product-001 2
HSET cart:user-123 product-005 1
EXPIRE cart:user-123 86400  # 24 hours
```

### Product Images (MongoDB)
```javascript
{
    product_id: "prod-001",
    images: [
        {url: "s3://...", type: "main", order: 1},
        {url: "s3://...", type: "detail", order: 2}
    ],
    videos: [{url: "s3://...", duration: 30}]
}
```

### Recommendations (Neo4j)
```cypher
CREATE (p1:Product {sku: 'SKU-001'})
CREATE (p2:Product {sku: 'SKU-002'})
CREATE (p1)-[:FREQUENTLY_BOUGHT_WITH {score: 0.85}]->(p2)
```

### Sales Trends (Cassandra)
```cql
CREATE TABLE sales_by_day (
    product_id UUID,
    sale_date DATE,
    units_sold INT,
    revenue DECIMAL,
    PRIMARY KEY (product_id, sale_date)
);
```

## Performance Benchmarks

| Operation | Latency | Throughput |
|-----------|---------|------------|
| Product Search | <50ms | 10K/sec |
| Add to Cart | <10ms | 50K/sec |
| Order Placement | <100ms | 5K/sec |
| Inventory Check | <5ms | 100K/sec |
| Recommendation | <50ms | 20K/sec |
| Price Lookup | <5ms | 100K/sec |

## Quick Start

```bash
# Set up schemas
psql -h localhost -p 5432 -U orbit -d retail < sql/01_schema_products.sql
psql -h localhost -p 5432 -U orbit -d retail < sql/02_schema_inventory.sql
psql -h localhost -p 5432 -U orbit -d retail < sql/03_schema_orders.sql

# Initialize Redis
redis-cli -h localhost -p 6379 < redis/01_retail_operations.redis

# Set up MongoDB
mongosh mongodb://localhost:27017/retail mongodb/01_product_catalog.js

# Create Neo4j graph
cypher-shell -a bolt://localhost:7687 < cypher/01_recommendations.cypher
```

## Running Examples

### Python - Order Processing
```bash
cd python
python3 01_order_processing.py --order-id ORD-12345
```

### JavaScript - Real-time Inventory
```bash
cd javascript
node 01_inventory_dashboard.js --port 3000
```

## Directory Structure

```text
retail/
├── sql/                    # PostgreSQL schemas
├── redis/                  # Redis operations
├── mongodb/                # MongoDB collections
├── cypher/                 # Neo4j graphs
├── cql/                    # Cassandra tables
├── python/                 # Python workflows
├── javascript/             # JavaScript integration
└── workflows/              # Documentation
```
