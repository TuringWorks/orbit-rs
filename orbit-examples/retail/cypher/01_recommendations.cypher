// ============================================================================
// OrbitRS Retail Examples - Neo4j Recommendation Graphs
// ============================================================================
// Product recommendations, customer journey, influencer networks
// ============================================================================

// ============================================================================
// CREATE PRODUCT NODES
// ============================================================================

CREATE (p1:Product {
    sku: 'SKU-001',
    name: 'Premium Winter Jacket',
    category: 'Outerwear',
    price: 99.99,
    brand: 'WinterWear'
});

CREATE (p2:Product {
    sku: 'SKU-010',
    name: 'Wool Scarf',
    category: 'Accessories',
    price: 29.99,
    brand: 'WinterWear'
});

CREATE (p3:Product {
    sku: 'SKU-015',
    name: 'Leather Boots',
    category: 'Footwear',
    price: 149.99,
    brand: 'UrbanStyle'
});

// ============================================================================
// CREATE CUSTOMER NODES
// ============================================================================

CREATE (c1:Customer {
    customer_id: 'cust-001',
    email: 'customer1@example.com',
    loyalty_tier: 'GOLD',
    total_spent: 1250.00
});

CREATE (c2:Customer {
    customer_id: 'cust-002',
    email: 'customer2@example.com',
    loyalty_tier: 'SILVER',
    total_spent: 450.00
});

// ============================================================================
// PURCHASE RELATIONSHIPS
// ============================================================================

// Customer purchases
MATCH (c:Customer {customer_id: 'cust-001'})
MATCH (p:Product {sku: 'SKU-001'})
CREATE (c)-[:PURCHASED {
    order_id: 'order-001',
    purchase_date: date('2024-12-01'),
    quantity: 1,
    price_paid: 99.99
}]->(p);

MATCH (c:Customer {customer_id: 'cust-001'})
MATCH (p:Product {sku: 'SKU-010'})
CREATE (c)-[:PURCHASED {
    order_id: 'order-001',
    purchase_date: date('2024-12-01'),
    quantity: 1,
    price_paid: 29.99
}]->(p);

// ============================================================================
// FREQUENTLY BOUGHT TOGETHER
// ============================================================================

MATCH (p1:Product {sku: 'SKU-001'})
MATCH (p2:Product {sku: 'SKU-010'})
CREATE (p1)-[:BOUGHT_WITH {
    frequency: 145,
    confidence: 0.85,
    lift: 2.3
}]->(p2);

MATCH (p1:Product {sku: 'SKU-001'})
MATCH (p3:Product {sku: 'SKU-015'})
CREATE (p1)-[:BOUGHT_WITH {
    frequency: 98,
    confidence: 0.72,
    lift: 1.8
}]->(p3);

// ============================================================================
// SIMILAR PRODUCTS
// ============================================================================

CREATE (p4:Product {
    sku: 'SKU-002',
    name: 'Insulated Parka',
    category: 'Outerwear',
    price: 129.99
});

MATCH (p1:Product {sku: 'SKU-001'})
MATCH (p4:Product {sku: 'SKU-002'})
CREATE (p1)-[:SIMILAR_TO {
    similarity_score: 0.92,
    reason: 'SAME_CATEGORY'
}]->(p4);

// ============================================================================
// CUSTOMER JOURNEY
// ============================================================================

// Viewed products
MATCH (c:Customer {customer_id: 'cust-001'})
MATCH (p:Product {sku: 'SKU-001'})
CREATE (c)-[:VIEWED {
    viewed_at: datetime('2024-11-28T10:30:00Z'),
    session_id: 'sess-abc123',
    duration_seconds: 45
}]->(p);

// Added to cart
MATCH (c:Customer {customer_id: 'cust-001'})
MATCH (p:Product {sku: 'SKU-001'})
CREATE (c)-[:ADDED_TO_CART {
    added_at: datetime('2024-11-28T10:32:00Z'),
    session_id: 'sess-abc123'
}]->(p);

// Wishlisted
MATCH (c:Customer {customer_id: 'cust-002'})
MATCH (p:Product {sku: 'SKU-015'})
CREATE (c)-[:WISHLISTED {
    added_at: datetime('2024-12-05T14:20:00Z')
}]->(p);

// ============================================================================
// INFLUENCER NETWORK
// ============================================================================

CREATE (inf1:Influencer {
    influencer_id: 'inf-001',
    name: 'Fashion Blogger',
    platform: 'INSTAGRAM',
    followers: 150000,
    engagement_rate: 4.5
});

MATCH (inf:Influencer {influencer_id: 'inf-001'})
MATCH (p:Product {sku: 'SKU-001'})
CREATE (inf)-[:PROMOTED {
    post_date: date('2024-11-20'),
    post_url: 'https://instagram.com/p/xyz',
    impressions: 45000,
    clicks: 2250,
    conversions: 87
}]->(p);

// Influencer followers who purchased
MATCH (c:Customer {customer_id: 'cust-001'})
MATCH (inf:Influencer {influencer_id: 'inf-001'})
CREATE (c)-[:FOLLOWS]->(inf);

// ============================================================================
// RECOMMENDATION QUERIES
// ============================================================================

// 1. Products frequently bought together
MATCH (p:Product {sku: 'SKU-001'})-[r:BOUGHT_WITH]->(recommended:Product)
RETURN recommended.sku, recommended.name, recommended.price, r.confidence
ORDER BY r.confidence DESC
LIMIT 5;

// 2. Similar products
MATCH (p:Product {sku: 'SKU-001'})-[r:SIMILAR_TO]->(similar:Product)
RETURN similar.sku, similar.name, similar.price, r.similarity_score
ORDER BY r.similarity_score DESC
LIMIT 5;

// 3. Customers who bought this also bought
MATCH (p:Product {sku: 'SKU-001'})<-[:PURCHASED]-(c:Customer)-[:PURCHASED]->(other:Product)
WHERE other.sku <> 'SKU-001'
RETURN other.sku, other.name, COUNT(*) AS purchase_count
ORDER BY purchase_count DESC
LIMIT 5;

// 4. Personalized recommendations based on purchase history
MATCH (c:Customer {customer_id: 'cust-001'})-[:PURCHASED]->(purchased:Product)
MATCH (purchased)-[:BOUGHT_WITH]->(recommended:Product)
WHERE NOT (c)-[:PURCHASED]->(recommended)
RETURN DISTINCT recommended.sku, recommended.name, recommended.price,
       COUNT(*) AS recommendation_strength
ORDER BY recommendation_strength DESC
LIMIT 10;

// 5. Trending products (most purchased recently)
MATCH (c:Customer)-[p:PURCHASED]->(product:Product)
WHERE p.purchase_date >= date() - duration({days: 7})
RETURN product.sku, product.name, COUNT(*) AS purchase_count
ORDER BY purchase_count DESC
LIMIT 10;

// 6. Customer segments by purchase patterns
MATCH (c:Customer)-[:PURCHASED]->(p:Product)
WITH c, COLLECT(DISTINCT p.category) AS categories
RETURN categories, COUNT(c) AS customer_count
ORDER BY customer_count DESC;

// 7. Influencer impact analysis
MATCH (inf:Influencer)-[promo:PROMOTED]->(p:Product)<-[:PURCHASED]-(c:Customer)-[:FOLLOWS]->(inf)
RETURN inf.name, p.name, COUNT(c) AS influenced_purchases,
       promo.conversions AS total_conversions
ORDER BY influenced_purchases DESC;

// 8. Cross-category recommendations
MATCH (c:Customer {customer_id: 'cust-001'})-[:PURCHASED]->(p1:Product)
MATCH (p1)<-[:PURCHASED]-(other:Customer)-[:PURCHASED]->(p2:Product)
WHERE p2.category <> p1.category
  AND NOT (c)-[:PURCHASED]->(p2)
RETURN DISTINCT p2.category, p2.sku, p2.name, COUNT(*) AS relevance
ORDER BY relevance DESC
LIMIT 5;

// 9. Customer journey funnel
MATCH path = (c:Customer {customer_id: 'cust-001'})-[:VIEWED]->(p:Product)
OPTIONAL MATCH (c)-[cart:ADDED_TO_CART]->(p)
OPTIONAL MATCH (c)-[purchase:PURCHASED]->(p)
RETURN p.sku, p.name,
       CASE WHEN cart IS NOT NULL THEN 'ADDED_TO_CART' ELSE 'VIEWED_ONLY' END AS cart_status,
       CASE WHEN purchase IS NOT NULL THEN 'PURCHASED' ELSE 'NOT_PURCHASED' END AS purchase_status;

// 10. Abandoned cart products (viewed + added but not purchased)
MATCH (c:Customer)-[:ADDED_TO_CART]->(p:Product)
WHERE NOT (c)-[:PURCHASED]->(p)
RETURN c.customer_id, c.email, p.sku, p.name, p.price
ORDER BY c.customer_id;

// ============================================================================
// GRAPH ANALYTICS (requires GDS library)
// ============================================================================

// PageRank for product importance
// CALL gds.pageRank.stream('product-graph')
// YIELD nodeId, score
// RETURN gds.util.asNode(nodeId).sku AS sku, score
// ORDER BY score DESC LIMIT 10;

// Community detection for customer segments
// CALL gds.louvain.stream('customer-product-graph')
// YIELD nodeId, communityId
// WITH gds.util.asNode(nodeId) AS node, communityId
// WHERE node:Customer
// RETURN communityId, COLLECT(node.customer_id) AS customers
// ORDER BY SIZE(customers) DESC;

// ============================================================================
// CLEANUP
// ============================================================================

// Delete all nodes and relationships (use with caution!)
// MATCH (n) DETACH DELETE n;
