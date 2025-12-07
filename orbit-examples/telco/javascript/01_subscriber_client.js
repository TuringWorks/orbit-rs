/**
 * =============================================================================
 * OrbitRS Telco Example: JavaScript Client for Subscriber Management
 * =============================================================================
 * Demonstrates multi-protocol access for telecom applications.
 *
 * Prerequisites:
 *   - OrbitRS server running
 *   - Node.js 18+ installed
 *
 * Install dependencies:
 *   npm install pg redis mongodb axios ws
 *
 * Run:
 *   node 01_subscriber_client.js
 * =============================================================================
 */

const { Client } = require('pg');
const { createClient } = require('redis');
const { MongoClient } = require('mongodb');
const axios = require('axios');
const WebSocket = require('ws');

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
        database: 'telco'
    },
    rest: {
        baseUrl: 'http://localhost:8080/api/v1'
    },
    websocket: {
        url: 'ws://localhost:8080/api/v1/ws/events'
    }
};

// =============================================================================
// PostgreSQL - Subscriber Account Data
// =============================================================================

async function subscriberAccountOperations() {
    console.log('\n========================================');
    console.log('Subscriber Account Operations (PostgreSQL)');
    console.log('========================================\n');

    const client = new Client(config.postgres);

    try {
        await client.connect();
        console.log('Connected to OrbitRS PostgreSQL');

        // Create schema
        await client.query('CREATE SCHEMA IF NOT EXISTS telco');

        // Subscribers table
        await client.query(`
            CREATE TABLE IF NOT EXISTS telco.subscribers (
                subscriber_id VARCHAR(50) PRIMARY KEY,
                phone_number VARCHAR(20) UNIQUE NOT NULL,
                first_name VARCHAR(100),
                last_name VARCHAR(100),
                email VARCHAR(255),
                plan_id VARCHAR(50),
                plan_name VARCHAR(100),
                monthly_price DECIMAL(10,2),
                status VARCHAR(20) DEFAULT 'ACTIVE',
                signup_date DATE DEFAULT CURRENT_DATE,
                last_activity TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                data_limit_gb INTEGER,
                data_used_gb DECIMAL(10,2) DEFAULT 0,
                loyalty_points INTEGER DEFAULT 0
            )
        `);

        // Usage records table
        await client.query(`
            CREATE TABLE IF NOT EXISTS telco.usage_records (
                record_id SERIAL PRIMARY KEY,
                subscriber_id VARCHAR(50) REFERENCES telco.subscribers(subscriber_id),
                usage_type VARCHAR(20),
                quantity DECIMAL(10,2),
                unit VARCHAR(20),
                timestamp TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                tower_id VARCHAR(50),
                session_id VARCHAR(100)
            )
        `);

        console.log('Created subscriber tables');

        // Insert subscribers
        const subscribers = [
            ['SUB-001', '+1-555-0101', 'John', 'Smith', 'john.smith@email.com', 'PLAN-UNL-5G', 'Unlimited 5G', 89.99, 'ACTIVE', null, 45.2, 15420],
            ['SUB-002', '+1-555-0102', 'Jane', 'Doe', 'jane.doe@email.com', 'PLAN-FAM', 'Family Share', 149.99, 'ACTIVE', 100, 78.5, 28500],
            ['SUB-003', '+1-555-0103', 'Bob', 'Johnson', 'bob.j@email.com', 'PLAN-BASIC', 'Basic 4G', 45.00, 'ACTIVE', 10, 8.2, 2150],
            ['SUB-004', '+1-555-0104', 'Alice', 'Williams', 'alice.w@email.com', 'PLAN-UNL-5G', 'Unlimited 5G', 89.99, 'SUSPENDED', null, 0, 8900]
        ];

        for (const sub of subscribers) {
            await client.query(`
                INSERT INTO telco.subscribers
                (subscriber_id, phone_number, first_name, last_name, email, plan_id, plan_name, monthly_price, status, data_limit_gb, data_used_gb, loyalty_points)
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12)
                ON CONFLICT (subscriber_id) DO UPDATE SET
                    status = EXCLUDED.status,
                    data_used_gb = EXCLUDED.data_used_gb,
                    last_activity = CURRENT_TIMESTAMP
            `, sub);
        }
        console.log(`Upserted ${subscribers.length} subscribers`);

        // Query: Active subscribers with usage
        const activeSubscribers = await client.query(`
            SELECT subscriber_id, first_name || ' ' || last_name as full_name,
                   phone_number, plan_name, monthly_price,
                   CASE WHEN data_limit_gb IS NULL THEN 'Unlimited'
                        ELSE ROUND((data_used_gb / data_limit_gb * 100)::numeric, 1) || '%'
                   END as data_usage,
                   loyalty_points
            FROM telco.subscribers
            WHERE status = 'ACTIVE'
            ORDER BY monthly_price DESC
        `);

        console.log('\nActive Subscribers:');
        console.table(activeSubscribers.rows);

        // Revenue summary
        const revenue = await client.query(`
            SELECT plan_name,
                   COUNT(*) as subscriber_count,
                   SUM(monthly_price) as monthly_revenue,
                   AVG(loyalty_points)::integer as avg_loyalty_points
            FROM telco.subscribers
            WHERE status = 'ACTIVE'
            GROUP BY plan_name
            ORDER BY monthly_revenue DESC
        `);

        console.log('\nRevenue by Plan:');
        console.table(revenue.rows);

    } finally {
        await client.end();
        console.log('PostgreSQL connection closed');
    }
}

// =============================================================================
// Redis - Real-time Session & Usage Tracking
// =============================================================================

async function realTimeSessionOperations() {
    console.log('\n========================================');
    console.log('Real-time Session Tracking (Redis)');
    console.log('========================================\n');

    const client = createClient({ url: config.redis.url });

    try {
        await client.connect();
        console.log('Connected to OrbitRS Redis');

        // Active session tracking
        const sessions = [
            { subscriberId: 'SUB-001', towerId: 'TWR-NYC-001', signalStrength: -72, connectionType: '5G_NR' },
            { subscriberId: 'SUB-002', towerId: 'TWR-NYC-002', signalStrength: -68, connectionType: '5G_NR' },
            { subscriberId: 'SUB-003', towerId: 'TWR-NYC-003', signalStrength: -82, connectionType: 'LTE' }
        ];

        // Store active sessions
        for (const session of sessions) {
            await client.hSet(`session:${session.subscriberId}`, {
                tower_id: session.towerId,
                signal_strength: session.signalStrength.toString(),
                connection_type: session.connectionType,
                started_at: new Date().toISOString(),
                last_activity: new Date().toISOString()
            });
            // Session expires after 30 minutes of inactivity
            await client.expire(`session:${session.subscriberId}`, 1800);

            // Add to tower's active subscribers set
            await client.sAdd(`tower:${session.towerId}:subscribers`, session.subscriberId);
        }
        console.log('Created active sessions');

        // Real-time data usage counters (per subscriber, per day)
        const today = new Date().toISOString().split('T')[0];
        await client.hIncrByFloat(`usage:${today}:SUB-001`, 'data_mb', 125.5);
        await client.hIncrBy(`usage:${today}:SUB-001`, 'voice_seconds', 180);
        await client.hIncrBy(`usage:${today}:SUB-001`, 'sms_count', 5);

        await client.hIncrByFloat(`usage:${today}:SUB-002`, 'data_mb', 2048.0);
        await client.hIncrBy(`usage:${today}:SUB-002`, 'voice_seconds', 420);
        await client.hIncrBy(`usage:${today}:SUB-002`, 'sms_count', 12);

        console.log('Updated real-time usage counters');

        // Get session info
        const session1 = await client.hGetAll('session:SUB-001');
        console.log('\nActive Session for SUB-001:');
        console.log(session1);

        // Get tower load
        const towerSubscribers = await client.sMembers('tower:TWR-NYC-001:subscribers');
        console.log(`\nSubscribers on TWR-NYC-001: ${towerSubscribers.length}`);

        // Tower metrics (sorted set for ranking)
        await client.zAdd('tower:load', [
            { score: 1250, value: 'TWR-NYC-001' },
            { score: 2100, value: 'TWR-NYC-002' },
            { score: 1800, value: 'TWR-NYC-003' }
        ]);

        const towerRanking = await client.zRangeWithScores('tower:load', 0, -1, { REV: true });
        console.log('\nTower Load Ranking:');
        towerRanking.forEach((t, i) => {
            console.log(`  ${i + 1}. ${t.value}: ${t.score} connections`);
        });

        // Alerts queue (list as queue)
        await client.lPush('alerts:queue', JSON.stringify({
            type: 'HIGH_LATENCY',
            tower_id: 'TWR-NYC-003',
            severity: 'WARNING',
            message: 'Latency > 20ms',
            timestamp: new Date().toISOString()
        }));

        const alert = await client.rPop('alerts:queue');
        console.log('\nProcessed Alert:');
        console.log(JSON.parse(alert));

        // Subscriber rate limiting
        const rateLimitKey = 'ratelimit:api:SUB-001';
        const current = await client.incr(rateLimitKey);
        if (current === 1) {
            await client.expire(rateLimitKey, 60); // 1 minute window
        }
        console.log(`\nAPI calls this minute for SUB-001: ${current}`);

    } finally {
        await client.quit();
        console.log('Redis connection closed');
    }
}

// =============================================================================
// MongoDB - Rich Subscriber Profiles
// =============================================================================

async function subscriberProfileOperations() {
    console.log('\n========================================');
    console.log('Subscriber Profiles (MongoDB)');
    console.log('========================================\n');

    const client = new MongoClient(config.mongodb.url);

    try {
        await client.connect();
        console.log('Connected to OrbitRS MongoDB');

        const db = client.db(config.mongodb.database);
        const profiles = db.collection('subscriber_profiles');
        const interactions = db.collection('customer_interactions');

        // Insert rich subscriber profiles
        const subscriberProfiles = [
            {
                subscriber_id: 'SUB-001',
                personal: {
                    first_name: 'John',
                    last_name: 'Smith',
                    date_of_birth: new Date('1985-03-15'),
                    preferred_language: 'en'
                },
                contact: {
                    phone: '+1-555-0101',
                    email: 'john.smith@email.com',
                    address: {
                        street: '123 Main St',
                        city: 'New York',
                        state: 'NY',
                        zip: '10001'
                    }
                },
                plan: {
                    id: 'PLAN-UNL-5G',
                    name: 'Unlimited 5G',
                    monthly_price: 89.99,
                    features: ['Unlimited Data', 'Unlimited Talk', 'Unlimited Text', 'HD Streaming', '50GB Hotspot']
                },
                devices: [
                    {
                        imei: '353456789012345',
                        type: 'smartphone',
                        make: 'Apple',
                        model: 'iPhone 15 Pro',
                        activated: new Date('2024-01-15'),
                        status: 'active'
                    }
                ],
                preferences: {
                    paperless_billing: true,
                    autopay: true,
                    marketing_consent: true,
                    notification_channels: ['email', 'sms', 'push']
                },
                loyalty: {
                    tier: 'GOLD',
                    points: 15420,
                    member_since: new Date('2022-01-15'),
                    benefits: ['Priority Support', '10% Accessory Discount', 'Free Upgrade Every 2 Years']
                },
                usage_patterns: {
                    peak_hours: ['18:00-21:00'],
                    top_apps: ['YouTube', 'Netflix', 'Spotify'],
                    avg_daily_data_mb: 1500,
                    primary_tower: 'TWR-NYC-001'
                }
            },
            {
                subscriber_id: 'SUB-002',
                personal: {
                    first_name: 'Jane',
                    last_name: 'Doe',
                    date_of_birth: new Date('1990-07-22'),
                    preferred_language: 'en'
                },
                contact: {
                    phone: '+1-555-0102',
                    email: 'jane.doe@email.com',
                    address: {
                        street: '456 Oak Ave',
                        city: 'Brooklyn',
                        state: 'NY',
                        zip: '11201'
                    }
                },
                plan: {
                    id: 'PLAN-FAM',
                    name: 'Family Share',
                    monthly_price: 149.99,
                    lines: 4,
                    shared_data_gb: 100,
                    features: ['Shared Data Pool', 'Unlimited Talk', 'Unlimited Text', 'Family Locator']
                },
                family_members: [
                    { name: 'Mike Doe', relation: 'spouse', line: '+1-555-0110' },
                    { name: 'Emma Doe', relation: 'child', line: '+1-555-0111' },
                    { name: 'Tom Doe', relation: 'child', line: '+1-555-0112' }
                ],
                loyalty: {
                    tier: 'PLATINUM',
                    points: 45820,
                    member_since: new Date('2020-06-15')
                }
            }
        ];

        for (const profile of subscriberProfiles) {
            await profiles.updateOne(
                { subscriber_id: profile.subscriber_id },
                { $set: profile, $setOnInsert: { created_at: new Date() } },
                { upsert: true }
            );
        }
        console.log(`Upserted ${subscriberProfiles.length} subscriber profiles`);

        // Customer interactions
        const customerInteractions = [
            {
                subscriber_id: 'SUB-001',
                channel: 'PHONE',
                type: 'SUPPORT',
                category: 'BILLING',
                summary: 'Question about international charges',
                resolution: 'Explained roaming package options',
                agent_id: 'AGT-101',
                duration_minutes: 12,
                satisfaction_score: 5,
                timestamp: new Date()
            },
            {
                subscriber_id: 'SUB-002',
                channel: 'CHAT',
                type: 'SALES',
                category: 'UPGRADE',
                summary: 'Inquired about adding 5th line',
                resolution: 'Scheduled callback with sales team',
                agent_id: 'AGT-202',
                duration_minutes: 8,
                timestamp: new Date()
            }
        ];

        await interactions.insertMany(customerInteractions);
        console.log(`Inserted ${customerInteractions.length} interactions`);

        // Aggregation: Customer 360 view
        const customer360 = await profiles.aggregate([
            { $match: { subscriber_id: 'SUB-001' } },
            {
                $lookup: {
                    from: 'customer_interactions',
                    localField: 'subscriber_id',
                    foreignField: 'subscriber_id',
                    as: 'interactions'
                }
            },
            {
                $project: {
                    subscriber_id: 1,
                    name: { $concat: ['$personal.first_name', ' ', '$personal.last_name'] },
                    plan: '$plan.name',
                    loyalty_tier: '$loyalty.tier',
                    loyalty_points: '$loyalty.points',
                    device_count: { $size: '$devices' },
                    interaction_count: { $size: '$interactions' },
                    avg_satisfaction: { $avg: '$interactions.satisfaction_score' }
                }
            }
        ]).toArray();

        console.log('\nCustomer 360 View:');
        console.log(JSON.stringify(customer360[0], null, 2));

        // High-value subscribers
        const highValue = await profiles.aggregate([
            {
                $match: {
                    'loyalty.tier': { $in: ['GOLD', 'PLATINUM'] }
                }
            },
            {
                $project: {
                    subscriber_id: 1,
                    name: { $concat: ['$personal.first_name', ' ', '$personal.last_name'] },
                    tier: '$loyalty.tier',
                    points: '$loyalty.points',
                    monthly_revenue: '$plan.monthly_price'
                }
            },
            { $sort: { points: -1 } }
        ]).toArray();

        console.log('\nHigh-Value Subscribers:');
        console.table(highValue);

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
        console.log('Server Health:', health.data.status || 'OK');

        // Query subscribers via REST
        const subscribers = await api.post('/sql', {
            query: `SELECT subscriber_id, phone_number, plan_name, status
                    FROM telco.subscribers
                    WHERE status = 'ACTIVE'
                    ORDER BY subscriber_id`
        });
        console.log('\nActive Subscribers via REST:');
        console.log(JSON.stringify(subscribers.data, null, 2));

        // Cluster info
        const cluster = await api.get('/cluster/status');
        console.log('\nCluster Status:');
        console.log(JSON.stringify(cluster.data, null, 2));

    } catch (error) {
        if (error.response) {
            console.error('API Error:', error.response.status, error.response.data);
        } else if (error.code === 'ECONNREFUSED') {
            console.log('REST API not available (server may not be running)');
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
    console.log('OrbitRS Telco Multi-Protocol Examples');
    console.log('===========================================');

    try {
        await subscriberAccountOperations();
        await realTimeSessionOperations();
        await subscriberProfileOperations();
        await restApiOperations();

        console.log('\n===========================================');
        console.log('All examples completed successfully!');
        console.log('===========================================');
    } catch (error) {
        console.error('Error:', error.message);
        process.exit(1);
    }
}

if (require.main === module) {
    main();
}

module.exports = {
    subscriberAccountOperations,
    realTimeSessionOperations,
    subscriberProfileOperations,
    restApiOperations
};
