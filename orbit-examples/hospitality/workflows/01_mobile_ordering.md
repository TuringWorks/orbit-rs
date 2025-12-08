# Hospitality Workflow: Mobile Order Ahead

## Overview
Mobile order ahead with loyalty rewards and kitchen operations.

## Workflow Steps

### 1. Customer Login (PostgreSQL + Redis)
```sql
SELECT customer_id, loyalty_tier, points_balance
FROM customers
WHERE email = 'customer@email.com';
```

```redis
SETEX session:customer-123 3600 '{"customer_id": "customer-123"}'
```

### 2. Browse Menu (PostgreSQL + MongoDB)
```sql
SELECT item_id, name, price, category
FROM menu_items
WHERE available = TRUE AND store_id = 'store-nyc';
```

```javascript
// Get item images and details
db.menu_media.find({item_id: "item-456"});
```

### 3. Place Order (PostgreSQL + Redis)
```sql
INSERT INTO orders (order_id, customer_id, store_id, total_amount, order_type)
VALUES (uuid_generate_v4(), 'customer-123', 'store-nyc', 12.50, 'MOBILE');
```

```redis
# Add to order queue
ZADD orders:store-nyc:pending [timestamp] "order-uuid"

# Estimated wait time
GET wait_time:store-nyc
# Returns: "15 minutes"
```

### 4. Kitchen Display (Redis Pub/Sub)
```redis
PUBLISH kitchen:store-nyc '{
  "order_id": "order-uuid",
  "items": ["Latte", "Croissant"],
  "priority": "NORMAL"
}'
```

### 5. Loyalty Points (PostgreSQL + Redis)
```sql
UPDATE customers
SET points_balance = points_balance + 125
WHERE customer_id = 'customer-123';
```

```redis
INCRBY loyalty:points:customer-123 125
```

### 6. Order Ready Notification (Redis Pub/Sub)
```redis
PUBLISH notifications:customer-123 '{
  "type": "ORDER_READY",
  "order_id": "order-uuid",
  "pickup_code": "4567"
}'
```

**Performance**: <2s order placement, real-time kitchen updates
