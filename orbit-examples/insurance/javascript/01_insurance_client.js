/**
 * =============================================================================
 * OrbitRS Insurance Example: JavaScript Client for Insurance Operations
 * =============================================================================
 * Demonstrates multi-protocol access from JavaScript for insurance data.
 *
 * Prerequisites:
 *   - OrbitRS server running
 *   - Node.js 18+ installed
 *   - npm packages: pg, redis, mongodb, axios
 *
 * Install dependencies:
 *   npm install pg redis mongodb axios
 *
 * Run:
 *   node 01_insurance_client.js
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
        database: 'insurance'
    },
    rest: {
        baseUrl: 'http://localhost:8080/api/v1'
    }
};

// =============================================================================
// PostgreSQL Operations - Policy and Claims Data
// =============================================================================

async function postgresOperations() {
    console.log('\n========================================');
    console.log('PostgreSQL Operations - Policy Management');
    console.log('========================================\n');

    const client = new Client(config.postgres);

    try {
        await client.connect();
        console.log('Connected to OrbitRS PostgreSQL');

        // Create schema and tables
        await client.query('CREATE SCHEMA IF NOT EXISTS insurance');

        await client.query(`
            CREATE TABLE IF NOT EXISTS insurance.policies (
                policy_id VARCHAR(50) PRIMARY KEY,
                policy_number VARCHAR(30) UNIQUE NOT NULL,
                policy_type VARCHAR(20) NOT NULL,
                customer_id VARCHAR(50) NOT NULL,
                customer_name VARCHAR(200),
                effective_date DATE NOT NULL,
                expiration_date DATE NOT NULL,
                premium DECIMAL(10,2) NOT NULL,
                coverage_limit DECIMAL(14,2),
                deductible DECIMAL(10,2),
                status VARCHAR(20) DEFAULT 'ACTIVE',
                underwriter_id VARCHAR(50),
                created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
            )
        `);
        console.log('Created policies table');

        await client.query(`
            CREATE TABLE IF NOT EXISTS insurance.claims (
                claim_id VARCHAR(50) PRIMARY KEY,
                claim_number VARCHAR(30) UNIQUE NOT NULL,
                policy_id VARCHAR(50),
                claimant_name VARCHAR(200),
                loss_date TIMESTAMP NOT NULL,
                report_date TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                loss_type VARCHAR(50),
                loss_description TEXT,
                claimed_amount DECIMAL(12,2),
                approved_amount DECIMAL(12,2) DEFAULT 0,
                paid_amount DECIMAL(12,2) DEFAULT 0,
                deductible_applied DECIMAL(10,2) DEFAULT 0,
                status VARCHAR(20) DEFAULT 'OPEN',
                adjuster_id VARCHAR(50),
                created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
            )
        `);
        console.log('Created claims table');

        await client.query(`
            CREATE TABLE IF NOT EXISTS insurance.customers (
                customer_id VARCHAR(50) PRIMARY KEY,
                first_name VARCHAR(100) NOT NULL,
                last_name VARCHAR(100) NOT NULL,
                email VARCHAR(200) UNIQUE,
                phone VARCHAR(20),
                date_of_birth DATE,
                address_street VARCHAR(200),
                address_city VARCHAR(100),
                address_state VARCHAR(2),
                address_zip VARCHAR(10),
                risk_score INTEGER DEFAULT 50,
                customer_since DATE DEFAULT CURRENT_DATE,
                total_premium_paid DECIMAL(12,2) DEFAULT 0,
                total_claims_filed INTEGER DEFAULT 0
            )
        `);
        console.log('Created customers table');

        // Insert sample data
        const customers = [
            ['CUST-INS-001', 'Michael', 'Anderson', 'michael.a@email.com', '+1-555-1001', '1982-05-20', 'Hartford', 'CT', 72],
            ['CUST-INS-002', 'Sarah', 'Williams', 'sarah.w@email.com', '+1-555-1002', '1975-09-12', 'Boston', 'MA', 88],
            ['CUST-INS-003', 'David', 'Martinez', 'david.m@email.com', '+1-555-1003', '1990-03-08', 'Miami', 'FL', 65]
        ];

        for (const customer of customers) {
            await client.query(`
                INSERT INTO insurance.customers
                (customer_id, first_name, last_name, email, phone, date_of_birth, address_city, address_state, risk_score)
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
                ON CONFLICT (customer_id) DO UPDATE SET
                    email = EXCLUDED.email,
                    risk_score = EXCLUDED.risk_score
            `, customer);
        }
        console.log(`Inserted/updated ${customers.length} customers`);

        const policies = [
            ['POL-INS-001', 'AUTO-2024-001', 'AUTO', 'CUST-INS-001', 'Michael Anderson', '2024-01-01', '2025-01-01', 1250.00, 100000.00, 500.00],
            ['POL-INS-002', 'HOME-2024-001', 'HOME', 'CUST-INS-001', 'Michael Anderson', '2024-01-01', '2025-01-01', 1850.00, 450000.00, 1000.00],
            ['POL-INS-003', 'LIFE-2024-001', 'LIFE', 'CUST-INS-002', 'Sarah Williams', '2024-02-01', '2044-02-01', 950.00, 750000.00, 0.00],
            ['POL-INS-004', 'AUTO-2024-002', 'AUTO', 'CUST-INS-003', 'David Martinez', '2024-03-01', '2025-03-01', 1680.00, 150000.00, 750.00],
            ['POL-INS-005', 'UMBRELLA-2024-001', 'UMBRELLA', 'CUST-INS-001', 'Michael Anderson', '2024-01-01', '2025-01-01', 450.00, 2000000.00, 0.00]
        ];

        for (const policy of policies) {
            await client.query(`
                INSERT INTO insurance.policies
                (policy_id, policy_number, policy_type, customer_id, customer_name, effective_date, expiration_date, premium, coverage_limit, deductible)
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
                ON CONFLICT (policy_id) DO UPDATE SET
                    premium = EXCLUDED.premium,
                    status = 'ACTIVE',
                    updated_at = CURRENT_TIMESTAMP
            `, policy);
        }
        console.log(`Inserted/updated ${policies.length} policies`);

        const claims = [
            ['CLM-INS-001', 'CLM-2024-001', 'POL-INS-001', 'Michael Anderson', '2024-06-15 14:30:00', 'COLLISION', 'Rear-end collision at intersection', 4500.00, 4000.00, 3500.00, 'CLOSED'],
            ['CLM-INS-002', 'CLM-2024-002', 'POL-INS-002', 'Michael Anderson', '2024-07-20 08:00:00', 'WATER_DAMAGE', 'Pipe burst in basement', 12000.00, 11000.00, 0.00, 'APPROVED'],
            ['CLM-INS-003', 'CLM-2024-003', 'POL-INS-004', 'David Martinez', '2024-08-10 16:45:00', 'THEFT', 'Vehicle break-in, laptop stolen', 2500.00, 0.00, 0.00, 'UNDER_REVIEW']
        ];

        for (const claim of claims) {
            await client.query(`
                INSERT INTO insurance.claims
                (claim_id, claim_number, policy_id, claimant_name, loss_date, loss_type, loss_description, claimed_amount, approved_amount, paid_amount, status)
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
                ON CONFLICT (claim_id) DO UPDATE SET
                    status = EXCLUDED.status,
                    approved_amount = EXCLUDED.approved_amount
            `, claim);
        }
        console.log(`Inserted/updated ${claims.length} claims`);

        // Query: Policy portfolio summary
        const portfolioSummary = await client.query(`
            SELECT
                policy_type,
                COUNT(*) as policy_count,
                SUM(premium) as total_premium,
                SUM(coverage_limit) as total_exposure,
                AVG(premium) as avg_premium
            FROM insurance.policies
            WHERE status = 'ACTIVE'
            GROUP BY policy_type
            ORDER BY total_premium DESC
        `);

        console.log('\nPolicy Portfolio Summary:');
        console.table(portfolioSummary.rows);

        // Query: Claims loss ratio
        const lossRatio = await client.query(`
            SELECT
                p.policy_type,
                COUNT(DISTINCT p.policy_id) as policies,
                SUM(p.premium) as earned_premium,
                COUNT(c.claim_id) as claim_count,
                COALESCE(SUM(c.paid_amount), 0) as paid_losses,
                ROUND(COALESCE(SUM(c.paid_amount), 0) / NULLIF(SUM(p.premium), 0) * 100, 2) as loss_ratio_percent
            FROM insurance.policies p
            LEFT JOIN insurance.claims c ON p.policy_id = c.policy_id
            WHERE p.status = 'ACTIVE'
            GROUP BY p.policy_type
            ORDER BY loss_ratio_percent DESC NULLS LAST
        `);

        console.log('\nLoss Ratio by Policy Type:');
        console.table(lossRatio.rows);

        // Query: Customer value analysis
        const customerValue = await client.query(`
            SELECT
                c.customer_id,
                c.first_name || ' ' || c.last_name as customer_name,
                c.risk_score,
                COUNT(DISTINCT p.policy_id) as policy_count,
                SUM(p.premium) as total_premium,
                COUNT(cl.claim_id) as claim_count,
                COALESCE(SUM(cl.claimed_amount), 0) as total_claims
            FROM insurance.customers c
            LEFT JOIN insurance.policies p ON c.customer_id = p.customer_id
            LEFT JOIN insurance.claims cl ON p.policy_id = cl.policy_id
            GROUP BY c.customer_id, c.first_name, c.last_name, c.risk_score
            ORDER BY total_premium DESC
        `);

        console.log('\nCustomer Value Analysis:');
        console.table(customerValue.rows);

    } finally {
        await client.end();
        console.log('PostgreSQL connection closed');
    }
}

// =============================================================================
// Redis Operations - Real-time Insurance Data
// =============================================================================

async function redisOperations() {
    console.log('\n========================================');
    console.log('Redis Operations - Real-time Data');
    console.log('========================================\n');

    const client = createClient({ url: config.redis.url });

    try {
        await client.connect();
        console.log('Connected to OrbitRS Redis');

        // Cache policy data for quick lookups
        const policies = [
            { id: 'POL-INS-001', type: 'AUTO', customer: 'Michael Anderson', premium: 1250.00, status: 'ACTIVE' },
            { id: 'POL-INS-002', type: 'HOME', customer: 'Michael Anderson', premium: 1850.00, status: 'ACTIVE' },
            { id: 'POL-INS-003', type: 'LIFE', customer: 'Sarah Williams', premium: 950.00, status: 'ACTIVE' }
        ];

        for (const policy of policies) {
            await client.hSet(`policy:${policy.id}`, {
                type: policy.type,
                customer: policy.customer,
                premium: policy.premium.toString(),
                status: policy.status,
                cached_at: new Date().toISOString()
            });
            await client.expire(`policy:${policy.id}`, 3600); // 1 hour cache
        }
        console.log('Cached policy data');

        // Track claims processing queue
        await client.lPush('claims:queue:pending', 'CLM-INS-003');
        await client.lPush('claims:queue:approved', 'CLM-INS-002');
        console.log('Updated claims processing queues');

        // Real-time premium counters by type
        await client.hSet('premium:daily:2024-08-15', {
            AUTO: '5250.00',
            HOME: '3850.00',
            LIFE: '2450.00',
            UMBRELLA: '850.00'
        });
        console.log('Set daily premium counters');

        // Claims metrics counters
        await client.incrBy('claims:count:2024', 3);
        await client.incrByFloat('claims:amount:2024', 19000.00);
        console.log('Updated claims metrics');

        // Policy expiration alerts (sorted set by expiration timestamp)
        const expirations = [
            { policy: 'POL-INS-001', timestamp: new Date('2025-01-01').getTime() / 1000 },
            { policy: 'POL-INS-002', timestamp: new Date('2025-01-01').getTime() / 1000 },
            { policy: 'POL-INS-004', timestamp: new Date('2025-03-01').getTime() / 1000 }
        ];
        for (const exp of expirations) {
            await client.zAdd('policy:expirations', { score: exp.timestamp, value: exp.policy });
        }
        console.log('Set policy expiration alerts');

        // Get policies expiring in next 90 days
        const now = Date.now() / 1000;
        const ninetyDaysLater = now + (90 * 24 * 60 * 60);
        const expiringSoon = await client.zRangeByScore('policy:expirations', now, ninetyDaysLater);
        console.log('\nPolicies expiring in next 90 days:', expiringSoon);

        // Get cached policy
        const cachedPolicy = await client.hGetAll('policy:POL-INS-001');
        console.log('Cached policy POL-INS-001:', cachedPolicy);

        // Get daily premium totals
        const dailyPremium = await client.hGetAll('premium:daily:2024-08-15');
        console.log('Daily premium totals:', dailyPremium);

        // Claim status tracking
        const pendingClaims = await client.lRange('claims:queue:pending', 0, -1);
        console.log('Pending claims in queue:', pendingClaims);

        // Real-time notifications (pub/sub example - publish only)
        await client.publish('insurance:events', JSON.stringify({
            event_type: 'CLAIM_FILED',
            claim_id: 'CLM-INS-003',
            policy_id: 'POL-INS-004',
            amount: 2500.00,
            timestamp: new Date().toISOString()
        }));
        console.log('Published claim event notification');

    } finally {
        await client.quit();
        console.log('Redis connection closed');
    }
}

// =============================================================================
// MongoDB Operations - Rich Document Storage
// =============================================================================

async function mongodbOperations() {
    console.log('\n========================================');
    console.log('MongoDB Operations - Document Storage');
    console.log('========================================\n');

    const client = new MongoClient(config.mongodb.url);

    try {
        await client.connect();
        console.log('Connected to OrbitRS MongoDB');

        const db = client.db(config.mongodb.database);
        const policiesCol = db.collection('policies');
        const claimsCol = db.collection('claims');
        const customersCol = db.collection('customers');

        // Insert rich policy documents
        const policies = [
            {
                policy_id: 'POL-INS-001',
                policy_number: 'AUTO-2024-001',
                policy_type: 'AUTO',
                customer: {
                    customer_id: 'CUST-INS-001',
                    name: 'Michael Anderson',
                    email: 'michael.a@email.com'
                },
                vehicle: {
                    vin: '1HGCM82633A123456',
                    year: 2023,
                    make: 'Honda',
                    model: 'Accord',
                    color: 'Blue'
                },
                coverages: [
                    { type: 'LIABILITY_BI', limit: 100000, deductible: 0 },
                    { type: 'LIABILITY_PD', limit: 50000, deductible: 0 },
                    { type: 'COLLISION', limit: 35000, deductible: 500 },
                    { type: 'COMPREHENSIVE', limit: 35000, deductible: 250 }
                ],
                drivers: [
                    { name: 'Michael Anderson', relation: 'INSURED', license: 'D1234567' }
                ],
                premium: {
                    annual: 1250.00,
                    payment_plan: 'MONTHLY',
                    next_due: new Date('2024-09-01')
                },
                effective_date: new Date('2024-01-01'),
                expiration_date: new Date('2025-01-01'),
                status: 'ACTIVE',
                created_at: new Date(),
                updated_at: new Date()
            },
            {
                policy_id: 'POL-INS-002',
                policy_number: 'HOME-2024-001',
                policy_type: 'HOME',
                customer: {
                    customer_id: 'CUST-INS-001',
                    name: 'Michael Anderson',
                    email: 'michael.a@email.com'
                },
                property: {
                    address: '123 Oak Street, Hartford, CT 06101',
                    type: 'SINGLE_FAMILY',
                    year_built: 1985,
                    square_footage: 2400,
                    construction: 'FRAME',
                    roof_type: 'ASPHALT_SHINGLE'
                },
                coverages: [
                    { type: 'DWELLING', limit: 350000, deductible: 1000 },
                    { type: 'OTHER_STRUCTURES', limit: 35000, deductible: 1000 },
                    { type: 'PERSONAL_PROPERTY', limit: 175000, deductible: 1000 },
                    { type: 'LIABILITY', limit: 300000, deductible: 0 }
                ],
                discounts: [
                    { type: 'MULTI_POLICY', percent: 10 },
                    { type: 'CLAIMS_FREE', percent: 5 },
                    { type: 'SECURITY_SYSTEM', percent: 3 }
                ],
                premium: {
                    annual: 1850.00,
                    payment_plan: 'ANNUAL',
                    paid_through: new Date('2025-01-01')
                },
                effective_date: new Date('2024-01-01'),
                expiration_date: new Date('2025-01-01'),
                status: 'ACTIVE',
                created_at: new Date(),
                updated_at: new Date()
            }
        ];

        for (const policy of policies) {
            await policiesCol.updateOne(
                { policy_id: policy.policy_id },
                { $set: policy },
                { upsert: true }
            );
        }
        console.log(`Upserted ${policies.length} policy documents`);

        // Insert claims with full history
        const claims = [
            {
                claim_id: 'CLM-INS-001',
                claim_number: 'CLM-2024-001',
                policy_id: 'POL-INS-001',
                policy_type: 'AUTO',
                claimant: {
                    name: 'Michael Anderson',
                    phone: '+1-555-1001',
                    email: 'michael.a@email.com'
                },
                loss: {
                    date: new Date('2024-06-15T14:30:00'),
                    type: 'COLLISION',
                    description: 'Rear-end collision at traffic light on Main St.',
                    location: 'Main St & Oak Ave, Hartford, CT',
                    police_report: 'HPD-2024-12345'
                },
                vehicle_damage: {
                    description: 'Rear bumper, trunk lid, tail lights',
                    repair_estimate: 4500.00,
                    shop_name: 'Hartford Auto Body',
                    photos: ['photo1.jpg', 'photo2.jpg', 'photo3.jpg']
                },
                financials: {
                    claimed_amount: 4500.00,
                    approved_amount: 4000.00,
                    deductible: 500.00,
                    paid_amount: 3500.00
                },
                timeline: [
                    { date: new Date('2024-06-15'), action: 'CLAIM_FILED', notes: 'Initial claim submission' },
                    { date: new Date('2024-06-17'), action: 'ADJUSTER_ASSIGNED', notes: 'Assigned to ADJ-001' },
                    { date: new Date('2024-06-20'), action: 'INSPECTION_COMPLETED', notes: 'Vehicle inspected at shop' },
                    { date: new Date('2024-06-22'), action: 'APPROVED', notes: 'Claim approved for $4000' },
                    { date: new Date('2024-06-25'), action: 'PAYMENT_ISSUED', notes: 'Payment of $3500 sent' }
                ],
                status: 'CLOSED',
                closed_date: new Date('2024-06-25'),
                created_at: new Date()
            }
        ];

        for (const claim of claims) {
            await claimsCol.updateOne(
                { claim_id: claim.claim_id },
                { $set: claim },
                { upsert: true }
            );
        }
        console.log(`Upserted ${claims.length} claim documents`);

        // Aggregation: Policy portfolio analysis
        const portfolioAnalysis = await policiesCol.aggregate([
            { $match: { status: 'ACTIVE' } },
            {
                $group: {
                    _id: '$policy_type',
                    count: { $sum: 1 },
                    total_premium: { $sum: '$premium.annual' },
                    avg_premium: { $avg: '$premium.annual' }
                }
            },
            { $sort: { total_premium: -1 } }
        ]).toArray();

        console.log('\nPortfolio Analysis:');
        console.log(JSON.stringify(portfolioAnalysis, null, 2));

        // Aggregation: Claims with full policy context
        const claimsWithPolicy = await claimsCol.aggregate([
            { $match: { status: 'CLOSED' } },
            {
                $lookup: {
                    from: 'policies',
                    localField: 'policy_id',
                    foreignField: 'policy_id',
                    as: 'policy'
                }
            },
            { $unwind: '$policy' },
            {
                $project: {
                    claim_number: 1,
                    'loss.type': 1,
                    'financials.paid_amount': 1,
                    'policy.policy_number': 1,
                    'policy.customer.name': 1,
                    processing_days: {
                        $dateDiff: {
                            startDate: '$loss.date',
                            endDate: '$closed_date',
                            unit: 'day'
                        }
                    }
                }
            }
        ]).toArray();

        console.log('\nClosed Claims Summary:');
        console.log(JSON.stringify(claimsWithPolicy, null, 2));

        // Text search (requires text index)
        await policiesCol.createIndex({
            'customer.name': 'text',
            'policy_number': 'text',
            'vehicle.make': 'text',
            'vehicle.model': 'text'
        });

        const searchResults = await policiesCol.find(
            { $text: { $search: 'Michael Honda' } }
        ).toArray();

        console.log('\nText Search Results:');
        searchResults.forEach(p => console.log(`  - ${p.policy_number}: ${p.customer.name}`));

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

        // Query active policies via REST
        const policiesResult = await api.post('/sql', {
            query: "SELECT policy_id, policy_number, policy_type, premium FROM insurance.policies WHERE status = 'ACTIVE' ORDER BY premium DESC LIMIT 5"
        });
        console.log('\nTop 5 Policies by Premium:');
        console.log(JSON.stringify(policiesResult.data, null, 2));

        // Get claims summary
        const claimsSummary = await api.post('/sql', {
            query: "SELECT status, COUNT(*) as count, SUM(claimed_amount) as total FROM insurance.claims GROUP BY status"
        });
        console.log('\nClaims Summary:');
        console.log(JSON.stringify(claimsSummary.data, null, 2));

        // Server statistics
        const stats = await api.get('/stats');
        console.log('\nServer Statistics:');
        console.log(JSON.stringify(stats.data, null, 2));

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
    console.log('OrbitRS Insurance JavaScript Client Examples');
    console.log('===========================================');

    try {
        await postgresOperations();
        await redisOperations();
        await mongodbOperations();
        await restApiOperations();

        console.log('\n===========================================');
        console.log('All insurance examples completed successfully!');
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
