# Retail Workflow: Order Processing with Recommendations

## Overview
E-commerce order processing with ML-powered recommendations and inventory management.

## Workflow Steps

### 1. Product Browsing (MongoDB + Redis)
```javascript
// Get product catalog
db.products.find({category: "Electronics"}).limit(20);
```

```redis
# Real-time inventory
GET inventory:product-456:warehouse-nyc
```

### 2. ML Recommendations (Neo4j + Redis)
```cypher
// Collaborative filtering
MATCH (u:User {id: 'user-123'})-[:PURCHASED]->(p:Product)
MATCH (p)<-[:PURCHASED]-(other:User)-[:PURCHASED]->(rec:Product)
WHERE NOT (u)-[:PURCHASED]->(rec)
RETURN rec ORDER BY COUNT(*) DESC LIMIT 10;
```

```redis
GET ml:recommendations:user-123
# Returns: Personalized product recommendations
```

### 3. Add to Cart (Redis)
```redis
HSET cart:user-123 product-456 2
EXPIRE cart:user-123 86400
```

### 4. Checkout & Payment (PostgreSQL)
```sql
INSERT INTO orders (order_id, customer_id, total_amount, status)
VALUES (uuid_generate_v4(), 'cust-123', 299.99, 'PENDING_PAYMENT');
```

### 5. Inventory Update (PostgreSQL + Redis)
```sql
UPDATE inventory
SET quantity = quantity - 2
WHERE product_id = 'product-456' AND warehouse_id = 'wh-nyc';
```

```redis
DECRBY inventory:product-456:warehouse-nyc 2
```

### 6. Order Fulfillment (PostgreSQL)
```sql
INSERT INTO shipments (shipment_id, order_id, carrier, tracking_number)
VALUES (uuid_generate_v4(), 'order-uuid', 'UPS', 'TRACK-123');
```

### 7. Analytics (Cassandra)
```cql
INSERT INTO sales_analytics (date, product_id, quantity_sold, revenue)
VALUES ('2024-12-07', 'product-456', 2, 299.99);
```

**Performance**: <100ms checkout, real-time inventory updates
