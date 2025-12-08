/**
 * =============================================================================
 * OrbitRS Retail Example: JavaScript Client for Product Catalog
 * =============================================================================
 * Demonstrates multi-protocol access from JavaScript using OrbitRS.
 *
 * Prerequisites:
 *   - OrbitRS server running
 *   - Node.js 18+ installed
 *   - npm packages: pg, redis, mongodb
 *
 * Install dependencies:
 *   npm install pg redis mongodb axios
 *
 * Run:
 *   node 01_product_catalog.js
 * =============================================================================
 */

const { Client } = require('pg');
const { createClient } = require('redis');
const { MongoClient } = require('mongodb');
const axios = require('axios');

// OrbitRS connection configuration
const config = {
    postgres: {
        host: 'localhost',
        port: 5432,
        user: 'orbit',
        password: 'orbit',
        database: 'orbit'
    },
    redis: {
        url: 'redis://localhost:6379'
    },
    mongodb: {
        url: 'mongodb://localhost:27017',
        database: 'retail'
    },
    rest: {
        baseUrl: 'http://localhost:8080/api/v1'
    }
};

// =============================================================================
// PostgreSQL Operations - Relational Data
// =============================================================================

async function postgresOperations() {
    console.log('\n========================================');
    console.log('PostgreSQL Operations');
    console.log('========================================\n');

    const client = new Client(config.postgres);

    try {
        await client.connect();
        console.log('Connected to OrbitRS PostgreSQL');

        // Create schema and tables
        await client.query('CREATE SCHEMA IF NOT EXISTS retail');

        await client.query(`
            CREATE TABLE IF NOT EXISTS retail.products (
                product_id VARCHAR(50) PRIMARY KEY,
                name VARCHAR(255) NOT NULL,
                description TEXT,
                category VARCHAR(100),
                subcategory VARCHAR(100),
                price DECIMAL(10,2) NOT NULL,
                cost DECIMAL(10,2),
                stock_quantity INTEGER DEFAULT 0,
                sku VARCHAR(50) UNIQUE,
                brand VARCHAR(100),
                weight_kg DECIMAL(5,2),
                dimensions_cm VARCHAR(50),
                is_active BOOLEAN DEFAULT true,
                created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
            )
        `);
        console.log('Created products table');

        await client.query(`
            CREATE TABLE IF NOT EXISTS retail.categories (
                category_id VARCHAR(50) PRIMARY KEY,
                name VARCHAR(100) NOT NULL,
                parent_id VARCHAR(50),
                description TEXT,
                display_order INTEGER DEFAULT 0
            )
        `);
        console.log('Created categories table');

        // Insert sample data
        const products = [
            ['PROD-001', 'Wireless Bluetooth Headphones', 'Premium noise-canceling wireless headphones', 'Electronics', 'Audio', 149.99, 75.00, 250, 'SKU-WBH-001', 'SoundMax', 0.28, '20x18x8'],
            ['PROD-002', 'Smart Fitness Tracker', '24/7 health monitoring with GPS', 'Electronics', 'Wearables', 89.99, 35.00, 500, 'SKU-SFT-001', 'FitPro', 0.05, '4x2x1'],
            ['PROD-003', 'Organic Coffee Beans', 'Single-origin Arabica beans, 1kg bag', 'Food & Beverage', 'Coffee', 24.99, 12.00, 1000, 'SKU-OCB-001', 'GreenBean', 1.00, '25x15x10'],
            ['PROD-004', 'Yoga Mat Premium', 'Non-slip eco-friendly yoga mat', 'Sports', 'Yoga', 45.99, 18.00, 300, 'SKU-YMP-001', 'ZenFit', 1.50, '180x60x0.6'],
            ['PROD-005', 'Stainless Steel Water Bottle', 'Insulated 750ml bottle', 'Home & Kitchen', 'Drinkware', 29.99, 8.00, 800, 'SKU-SSW-001', 'HydroLife', 0.35, '26x8x8']
        ];

        for (const product of products) {
            await client.query(`
                INSERT INTO retail.products
                (product_id, name, description, category, subcategory, price, cost, stock_quantity, sku, brand, weight_kg, dimensions_cm)
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
                ON CONFLICT (product_id) DO UPDATE SET
                    name = EXCLUDED.name,
                    price = EXCLUDED.price,
                    stock_quantity = EXCLUDED.stock_quantity,
                    updated_at = CURRENT_TIMESTAMP
            `, product);
        }
        console.log(`Inserted/updated ${products.length} products`);

        // Query products
        const result = await client.query(`
            SELECT product_id, name, category, price, stock_quantity,
                   ROUND((price - cost) / price * 100, 2) as margin_percent
            FROM retail.products
            WHERE is_active = true
            ORDER BY category, name
        `);

        console.log('\nActive Products:');
        console.table(result.rows);

        // Inventory summary
        const summary = await client.query(`
            SELECT category,
                   COUNT(*) as product_count,
                   SUM(stock_quantity) as total_stock,
                   ROUND(AVG(price)::numeric, 2) as avg_price,
                   ROUND(SUM(price * stock_quantity)::numeric, 2) as inventory_value
            FROM retail.products
            WHERE is_active = true
            GROUP BY category
            ORDER BY inventory_value DESC
        `);

        console.log('\nInventory Summary by Category:');
        console.table(summary.rows);

    } finally {
        await client.end();
        console.log('PostgreSQL connection closed');
    }
}

// =============================================================================
// Redis Operations - Caching and Real-time Data
// =============================================================================

async function redisOperations() {
    console.log('\n========================================');
    console.log('Redis Operations');
    console.log('========================================\n');

    const client = createClient({ url: config.redis.url });

    try {
        await client.connect();
        console.log('Connected to OrbitRS Redis');

        // Cache product data
        const products = [
            { id: 'PROD-001', name: 'Wireless Bluetooth Headphones', price: 149.99, stock: 250 },
            { id: 'PROD-002', name: 'Smart Fitness Tracker', price: 89.99, stock: 500 },
            { id: 'PROD-003', name: 'Organic Coffee Beans', price: 24.99, stock: 1000 }
        ];

        // Store products as hashes
        for (const product of products) {
            await client.hSet(`product:${product.id}`, {
                name: product.name,
                price: product.price.toString(),
                stock: product.stock.toString(),
                cached_at: new Date().toISOString()
            });
            // Set expiration (1 hour cache)
            await client.expire(`product:${product.id}`, 3600);
        }
        console.log('Cached product data in Redis');

        // Store product views counter
        await client.zIncrBy('product:views', 150, 'PROD-001');
        await client.zIncrBy('product:views', 230, 'PROD-002');
        await client.zIncrBy('product:views', 89, 'PROD-003');
        console.log('Updated product view counters');

        // Get top viewed products
        const topProducts = await client.zRangeWithScores('product:views', 0, 2, { REV: true });
        console.log('\nTop Viewed Products:');
        topProducts.forEach((item, index) => {
            console.log(`  ${index + 1}. ${item.value}: ${item.score} views`);
        });

        // Real-time inventory tracking
        await client.hSet('inventory:realtime', {
            'PROD-001': '250',
            'PROD-002': '500',
            'PROD-003': '1000'
        });

        // Simulate inventory decrement (sale)
        const newStock = await client.hIncrBy('inventory:realtime', 'PROD-001', -1);
        console.log(`\nPROD-001 stock after sale: ${newStock}`);

        // Shopping cart session
        const cartKey = 'cart:session:user123';
        await client.hSet(cartKey, {
            'PROD-001': '2',
            'PROD-002': '1'
        });
        await client.expire(cartKey, 1800); // 30 min expiration
        console.log('Shopping cart session created');

        const cart = await client.hGetAll(cartKey);
        console.log('Cart contents:', cart);

        // Pub/Sub for real-time notifications (publish only, no subscriber in this example)
        await client.publish('inventory:updates', JSON.stringify({
            product_id: 'PROD-001',
            action: 'stock_updated',
            new_quantity: newStock,
            timestamp: new Date().toISOString()
        }));
        console.log('Published inventory update notification');

    } finally {
        await client.quit();
        console.log('Redis connection closed');
    }
}

// =============================================================================
// MongoDB Operations - Document Data
// =============================================================================

async function mongodbOperations() {
    console.log('\n========================================');
    console.log('MongoDB Operations');
    console.log('========================================\n');

    const client = new MongoClient(config.mongodb.url);

    try {
        await client.connect();
        console.log('Connected to OrbitRS MongoDB');

        const db = client.db(config.mongodb.database);
        const productsCol = db.collection('products');
        const reviewsCol = db.collection('reviews');

        // Insert rich product documents
        const products = [
            {
                product_id: 'PROD-001',
                name: 'Wireless Bluetooth Headphones',
                brand: 'SoundMax',
                specifications: {
                    driver_size: '40mm',
                    frequency_response: '20Hz-20kHz',
                    battery_life_hours: 30,
                    bluetooth_version: '5.2',
                    noise_cancellation: true,
                    colors: ['Black', 'White', 'Navy Blue']
                },
                pricing: {
                    list_price: 199.99,
                    sale_price: 149.99,
                    discount_percent: 25,
                    currency: 'USD'
                },
                images: [
                    { url: '/images/prod-001-main.jpg', type: 'main' },
                    { url: '/images/prod-001-side.jpg', type: 'gallery' },
                    { url: '/images/prod-001-box.jpg', type: 'gallery' }
                ],
                seo: {
                    title: 'Premium Wireless Headphones with Active Noise Cancellation',
                    description: 'Experience superior sound quality with 30-hour battery life',
                    keywords: ['headphones', 'wireless', 'bluetooth', 'noise-canceling']
                },
                ratings: {
                    average: 4.5,
                    count: 1250
                },
                created_at: new Date(),
                updated_at: new Date()
            },
            {
                product_id: 'PROD-002',
                name: 'Smart Fitness Tracker',
                brand: 'FitPro',
                specifications: {
                    display: 'AMOLED 1.4"',
                    water_resistance: '5ATM',
                    battery_life_days: 14,
                    sensors: ['Heart Rate', 'SpO2', 'GPS', 'Accelerometer'],
                    compatibility: ['iOS 12+', 'Android 8+'],
                    colors: ['Black', 'Rose Gold', 'Olive Green']
                },
                pricing: {
                    list_price: 129.99,
                    sale_price: 89.99,
                    discount_percent: 31,
                    currency: 'USD'
                },
                features: [
                    '24/7 Heart Rate Monitoring',
                    'Sleep Tracking',
                    '100+ Workout Modes',
                    'Stress Management',
                    'Women\'s Health Tracking'
                ],
                ratings: {
                    average: 4.3,
                    count: 3420
                },
                created_at: new Date(),
                updated_at: new Date()
            }
        ];

        // Upsert products
        for (const product of products) {
            await productsCol.updateOne(
                { product_id: product.product_id },
                { $set: product },
                { upsert: true }
            );
        }
        console.log(`Upserted ${products.length} product documents`);

        // Insert reviews
        const reviews = [
            {
                product_id: 'PROD-001',
                user_id: 'user-123',
                rating: 5,
                title: 'Best headphones I\'ve ever owned',
                content: 'The noise cancellation is incredible. Battery lasts forever.',
                verified_purchase: true,
                helpful_votes: 45,
                created_at: new Date()
            },
            {
                product_id: 'PROD-001',
                user_id: 'user-456',
                rating: 4,
                title: 'Great sound, minor comfort issues',
                content: 'Sound quality is excellent but can be uncomfortable after 3+ hours.',
                verified_purchase: true,
                helpful_votes: 23,
                created_at: new Date()
            }
        ];

        await reviewsCol.insertMany(reviews);
        console.log(`Inserted ${reviews.length} reviews`);

        // Aggregation: Product with reviews
        const productWithReviews = await productsCol.aggregate([
            { $match: { product_id: 'PROD-001' } },
            {
                $lookup: {
                    from: 'reviews',
                    localField: 'product_id',
                    foreignField: 'product_id',
                    as: 'reviews'
                }
            },
            {
                $project: {
                    product_id: 1,
                    name: 1,
                    'pricing.sale_price': 1,
                    'ratings.average': 1,
                    review_count: { $size: '$reviews' },
                    recent_reviews: { $slice: ['$reviews', 2] }
                }
            }
        ]).toArray();

        console.log('\nProduct with Reviews:');
        console.log(JSON.stringify(productWithReviews[0], null, 2));

        // Text search (requires text index)
        await productsCol.createIndex({ name: 'text', 'seo.description': 'text' });

        const searchResults = await productsCol.find(
            { $text: { $search: 'wireless bluetooth' } },
            { score: { $meta: 'textScore' } }
        ).sort({ score: { $meta: 'textScore' } }).toArray();

        console.log('\nText Search Results:');
        searchResults.forEach(p => console.log(`  - ${p.name}`));

    } finally {
        await client.close();
        console.log('MongoDB connection closed');
    }
}

// =============================================================================
// REST API Operations
// =============================================================================

async function restApiOperations() {
    console.log('\n========================================');
    console.log('REST API Operations');
    console.log('========================================\n');

    const api = axios.create({
        baseURL: config.rest.baseUrl,
        headers: { 'Content-Type': 'application/json' }
    });

    try {
        // Health check
        const health = await axios.get('http://localhost:8080/health');
        console.log('Server Health:', health.data);

        // Execute SQL via REST
        const createResult = await api.post('/sql', {
            query: "SELECT product_id, name, price FROM retail.products WHERE price > 50 ORDER BY price DESC"
        });
        console.log('\nPremium Products (via REST API):');
        console.log(JSON.stringify(createResult.data, null, 2));

        // Get server stats
        const stats = await api.get('/stats');
        console.log('\nServer Statistics:');
        console.log(JSON.stringify(stats.data, null, 2));

        // Get cluster status
        const cluster = await api.get('/cluster/status');
        console.log('\nCluster Status:');
        console.log(JSON.stringify(cluster.data, null, 2));

    } catch (error) {
        if (error.response) {
            console.error('API Error:', error.response.status, error.response.data);
        } else {
            console.error('Request Error:', error.message);
        }
    }
}

// =============================================================================
// Main Execution
// =============================================================================

async function main() {
    console.log('===========================================');
    console.log('OrbitRS Multi-Protocol JavaScript Examples');
    console.log('===========================================');

    try {
        await postgresOperations();
        await redisOperations();
        await mongodbOperations();
        await restApiOperations();

        console.log('\n===========================================');
        console.log('All examples completed successfully!');
        console.log('===========================================');
    } catch (error) {
        console.error('Error:', error.message);
        process.exit(1);
    }
}

// Run if executed directly
if (require.main === module) {
    main();
}

module.exports = {
    postgresOperations,
    redisOperations,
    mongodbOperations,
    restApiOperations
};
