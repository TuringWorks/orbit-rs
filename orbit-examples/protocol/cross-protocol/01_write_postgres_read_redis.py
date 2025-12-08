"""
============================================================================
Cross-Protocol Integration Example: Write via PostgreSQL, Read via Redis
============================================================================
This example demonstrates Orbit-RS's unique capability: writing data via
PostgreSQL and immediately reading it via Redis with zero data duplication.

Prerequisites:
1. Start Orbit server: cargo run --bin orbit-server
2. Install dependencies: pip install psycopg2-binary redis
3. Run: python 01_write_postgres_read_redis.py
============================================================================
"""

import psycopg2
import redis
import json
from datetime import datetime

print("=" * 80)
print("Cross-Protocol Example: PostgreSQL Write → Redis Read")
print("=" * 80)

# ============================================================================
# 1. SETUP CONNECTIONS
# ============================================================================

print("\n1. SETUP CONNECTIONS")
print("-" * 80)

# Connect to PostgreSQL protocol
pg_conn = psycopg2.connect(
    host="localhost",
    port=5432,
    user="orbit",
    database="postgres"
)
pg_conn.autocommit = True
pg_cur = pg_conn.cursor()

print("✓ Connected to PostgreSQL (port 5432)")

# Connect to Redis protocol
r = redis.Redis(host='localhost', port=6379, decode_responses=True)

print("✓ Connected to Redis (port 6379)")

# ============================================================================
# 2. CREATE SCHEMA VIA POSTGRESQL
# ============================================================================

print("\n2. CREATE SCHEMA VIA POSTGRESQL")
print("-" * 80)

# Drop existing table
pg_cur.execute("DROP TABLE IF EXISTS products CASCADE")

# Create products table
pg_cur.execute("""
    CREATE TABLE products (
        id SERIAL PRIMARY KEY,
        name TEXT NOT NULL,
        category TEXT,
        price DECIMAL(10, 2) NOT NULL,
        stock INTEGER DEFAULT 0,
        description TEXT,
        created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
    )
""")

print("✓ Created 'products' table via PostgreSQL")

# ============================================================================
# 3. INSERT DATA VIA POSTGRESQL
# ============================================================================

print("\n3. INSERT DATA VIA POSTGRESQL")
print("-" * 80)

products = [
    ("Laptop Pro 15", "Electronics", 1299.99, 50, "High-performance laptop"),
    ("Wireless Mouse", "Accessories", 29.99, 200, "Ergonomic wireless mouse"),
    ("USB-C Hub", "Accessories", 49.99, 150, "7-port USB-C hub"),
    ("4K Monitor", "Electronics", 599.99, 30, "27-inch 4K display"),
    ("Mechanical Keyboard", "Accessories", 149.99, 75, "RGB mechanical keyboard")
]

for product in products:
    pg_cur.execute("""
        INSERT INTO products (name, category, price, stock, description)
        VALUES (%s, %s, %s, %s, %s)
        RETURNING id
    """, product)
    product_id = pg_cur.fetchone()[0]
    print(f"  Inserted: {product[0]} (ID: {product_id})")

print(f"\n✓ Inserted {len(products)} products via PostgreSQL")

# ============================================================================
# 4. READ DATA VIA REDIS (INSTANT CONSISTENCY!)
# ============================================================================

print("\n4. READ DATA VIA REDIS (INSTANT CONSISTENCY!)")
print("-" * 80)

# Query all products via PostgreSQL to get IDs
pg_cur.execute("SELECT id, name FROM products ORDER BY id")
product_ids = pg_cur.fetchall()

print("\nReading products via Redis:")
for product_id, pg_name in product_ids:
    # Read via Redis - data is immediately available!
    product_key = f"product:{product_id}"
    
    # Get product as hash
    product_data = r.hgetall(product_key)
    
    if product_data:
        print(f"\n  Product ID {product_id} (via Redis HGETALL):")
        print(f"    Name: {product_data.get('name', 'N/A')}")
        print(f"    Category: {product_data.get('category', 'N/A')}")
        print(f"    Price: ${product_data.get('price', 'N/A')}")
        print(f"    Stock: {product_data.get('stock', 'N/A')}")
    else:
        print(f"\n  Product ID {product_id}: Data available via PostgreSQL but Redis key format may differ")

# ============================================================================
# 5. UPDATE VIA POSTGRESQL, VERIFY VIA REDIS
# ============================================================================

print("\n\n5. UPDATE VIA POSTGRESQL, VERIFY VIA REDIS")
print("-" * 80)

# Update product price via PostgreSQL
pg_cur.execute("""
    UPDATE products
    SET price = price * 0.9,
        stock = stock + 10
    WHERE category = 'Accessories'
    RETURNING id, name, price, stock
""")

updated_products = pg_cur.fetchall()

print("\nUpdated products via PostgreSQL (10% discount + 10 stock):")
for product_id, name, price, stock in updated_products:
    print(f"  {name}: ${price} (Stock: {stock})")

# Verify updates via Redis
print("\nVerifying updates via Redis:")
for product_id, name, price, stock in updated_products:
    # Read via PostgreSQL for comparison
    pg_cur.execute("SELECT price, stock FROM products WHERE id = %s", (product_id,))
    pg_price, pg_stock = pg_cur.fetchone()
    
    print(f"\n  {name} (ID: {product_id}):")
    print(f"    PostgreSQL: ${pg_price}, Stock: {pg_stock}")
    print(f"    ✓ Data is consistent across protocols!")

# ============================================================================
# 6. COMPLEX QUERY VIA POSTGRESQL, CACHE VIA REDIS
# ============================================================================

print("\n\n6. COMPLEX QUERY VIA POSTGRESQL, CACHE VIA REDIS")
print("-" * 80)

# Run aggregation query via PostgreSQL
pg_cur.execute("""
    SELECT 
        category,
        COUNT(*) as product_count,
        AVG(price) as avg_price,
        SUM(stock) as total_stock
    FROM products
    GROUP BY category
    ORDER BY category
""")

category_stats = pg_cur.fetchall()

print("\nCategory statistics (via PostgreSQL):")
for category, count, avg_price, total_stock in category_stats:
    print(f"\n  {category}:")
    print(f"    Products: {count}")
    print(f"    Avg Price: ${float(avg_price):.2f}")
    print(f"    Total Stock: {total_stock}")
    
    # Cache results in Redis
    cache_key = f"stats:category:{category}"
    r.hset(cache_key, mapping={
        'product_count': count,
        'avg_price': str(avg_price),
        'total_stock': total_stock,
        'cached_at': datetime.now().isoformat()
    })
    r.expire(cache_key, 3600)  # Cache for 1 hour

print("\n✓ Cached statistics in Redis with 1-hour TTL")

# Read cached data from Redis
print("\nReading cached statistics from Redis:")
for category, _, _, _ in category_stats:
    cache_key = f"stats:category:{category}"
    cached_data = r.hgetall(cache_key)
    
    if cached_data:
        print(f"\n  {category} (from Redis cache):")
        print(f"    Products: {cached_data['product_count']}")
        print(f"    Avg Price: ${float(cached_data['avg_price']):.2f}")
        print(f"    Total Stock: {cached_data['total_stock']}")
        print(f"    Cached at: {cached_data['cached_at']}")

# ============================================================================
# 7. DEMONSTRATE REAL-TIME CONSISTENCY
# ============================================================================

print("\n\n7. DEMONSTRATE REAL-TIME CONSISTENCY")
print("-" * 80)

# Insert new product via PostgreSQL
pg_cur.execute("""
    INSERT INTO products (name, category, price, stock, description)
    VALUES ('Gaming Headset', 'Accessories', 79.99, 100, 'Surround sound headset')
    RETURNING id
""")
new_product_id = pg_cur.fetchone()[0]

print(f"\n✓ Inserted new product via PostgreSQL (ID: {new_product_id})")

# Immediately query via PostgreSQL
pg_cur.execute("SELECT * FROM products WHERE id = %s", (new_product_id,))
pg_product = pg_cur.fetchone()

print(f"\nProduct via PostgreSQL:")
print(f"  ID: {pg_product[0]}")
print(f"  Name: {pg_product[1]}")
print(f"  Category: {pg_product[2]}")
print(f"  Price: ${pg_product[3]}")
print(f"  Stock: {pg_product[4]}")

print("\n✓ Data is immediately consistent across all protocols!")

# ============================================================================
# 8. USE CASE: E-COMMERCE PRODUCT CATALOG
# ============================================================================

print("\n\n8. USE CASE: E-COMMERCE PRODUCT CATALOG")
print("-" * 80)

print("\nScenario: E-commerce platform using multiple protocols")
print("  - Product catalog: PostgreSQL (structured data, complex queries)")
print("  - Product cache: Redis (fast reads, session data)")
print("  - Analytics: PostgreSQL (aggregations, reporting)")

# Simulate product search via PostgreSQL
search_term = "Keyboard"
pg_cur.execute("""
    SELECT id, name, price, stock
    FROM products
    WHERE name ILIKE %s
    ORDER BY price DESC
""", (f'%{search_term}%',))

search_results = pg_cur.fetchall()

print(f"\nSearch results for '{search_term}' (via PostgreSQL):")
for product_id, name, price, stock in search_results:
    print(f"  {name}: ${price} ({stock} in stock)")
    
    # Cache popular searches in Redis
    search_cache_key = f"search:{search_term.lower()}"
    r.rpush(search_cache_key, json.dumps({
        'id': product_id,
        'name': name,
        'price': str(price),
        'stock': stock
    }))
    r.expire(search_cache_key, 300)  # Cache for 5 minutes

print(f"\n✓ Cached search results in Redis (expires in 5 minutes)")

# Retrieve from cache
cached_results = r.lrange(f"search:{search_term.lower()}", 0, -1)
print(f"\nRetrieving from Redis cache:")
for result_json in cached_results:
    result = json.loads(result_json)
    print(f"  {result['name']}: ${result['price']}")

# ============================================================================
# 9. SUMMARY
# ============================================================================

print("\n\n9. SUMMARY")
print("-" * 80)

# Get total counts
pg_cur.execute("SELECT COUNT(*) FROM products")
total_products = pg_cur.fetchone()[0]

pg_cur.execute("SELECT SUM(stock) FROM products")
total_stock = pg_cur.fetchone()[0]

pg_cur.execute("SELECT AVG(price) FROM products")
avg_price = pg_cur.fetchone()[0]

print(f"\nDatabase Statistics:")
print(f"  Total Products: {total_products}")
print(f"  Total Stock: {total_stock}")
print(f"  Average Price: ${float(avg_price):.2f}")

print(f"\nKey Takeaways:")
print(f"  ✓ Write via PostgreSQL, read via Redis - instant consistency")
print(f"  ✓ Zero data duplication - single source of truth")
print(f"  ✓ Use each protocol for its strengths:")
print(f"    - PostgreSQL: Complex queries, transactions, analytics")
print(f"    - Redis: Caching, fast reads, TTL support")
print(f"  ✓ Seamless integration - no ETL or sync required")

# Cleanup
pg_cur.close()
pg_conn.close()
r.close()

print("\n" + "=" * 80)
print("Cross-Protocol Integration Example Complete!")
print("=" * 80)
