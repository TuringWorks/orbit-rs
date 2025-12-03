/**
 * Redis ML Example - Orbit Database
 *
 * This example demonstrates using ML commands through Orbit's Redis (RESP) protocol.
 * Requires: npm install redis
 */

const redis = require('redis');

const REDIS_URL = 'redis://localhost:6379';

async function main() {
    console.log('='.repeat(60));
    console.log('Orbit ML Examples - Redis (RESP) Protocol');
    console.log('='.repeat(60));

    const client = redis.createClient({ url: REDIS_URL });

    client.on('error', (err) => {
        console.error('Redis Client Error:', err.message);
    });

    try {
        await client.connect();
        console.log(`Connected to Orbit Redis at ${REDIS_URL}`);

        // Run examples
        await createSampleData(client);
        await exampleModelManagement(client);
        await exampleModelTraining(client);
        await examplePredictions(client);
        await exampleVectorOperations(client);
        await exampleIndustryModels(client);
        await exampleTimeSeries(client);

        console.log('\n' + '='.repeat(60));
        console.log('All examples completed!');
        console.log('Note: Some commands may show errors if ML features');
        console.log('are not fully implemented in the current server version.');
        console.log('='.repeat(60));

    } catch (error) {
        if (error.code === 'ECONNREFUSED') {
            console.error('\nConnection Error: Cannot connect to Redis');
            console.error('Make sure Orbit server is running on port 6379');
        } else {
            console.error('Error:', error.message);
        }
    } finally {
        await client.quit();
    }
}

async function createSampleData(client) {
    console.log('\n=== Setting Up Sample Data ===');

    // Customer data
    const customers = [
        { id: '1', tenure: '12', monthly_charges: '29.85', churned: '0' },
        { id: '2', tenure: '72', monthly_charges: '109.70', churned: '0' },
        { id: '3', tenure: '2', monthly_charges: '53.85', churned: '1' },
        { id: '4', tenure: '45', monthly_charges: '42.30', churned: '0' },
        { id: '5', tenure: '3', monthly_charges: '70.70', churned: '1' },
    ];

    for (const customer of customers) {
        await client.hSet(`customer:${customer.id}`, customer);
    }

    // Transaction data
    const transactions = [
        { id: '1', amount: '25.50', merchant: 'groceries', hour: '10', fraud: '0' },
        { id: '2', amount: '1500.00', merchant: 'electronics', hour: '3', fraud: '1' },
        { id: '3', amount: '45.00', merchant: 'restaurant', hour: '19', fraud: '0' },
        { id: '4', amount: '2000.00', merchant: 'jewelry', hour: '2', fraud: '1' },
    ];

    for (const txn of transactions) {
        await client.hSet(`transaction:${txn.id}`, txn);
    }

    console.log(`Created ${customers.length} customer records`);
    console.log(`Created ${transactions.length} transaction records`);
}

async function exampleModelManagement(client) {
    console.log('\n=== ML Model Management ===');

    // Create a model
    console.log('\nCreating fraud detection model...');
    try {
        const result = await client.sendCommand([
            'ML.CREATE', 'fraud_detector', 'xgboost',
            'FEATURES', 'amount,hour',
            'LABEL', 'fraud'
        ]);
        console.log('ML.CREATE result:', result);
    } catch (e) {
        console.log('Note:', e.message);
    }

    // List models
    console.log('\nListing all models...');
    try {
        const models = await client.sendCommand(['ML.LIST']);
        console.log('Available models:', models);
    } catch (e) {
        console.log('ML.LIST:', e.message);
    }
}

async function exampleModelTraining(client) {
    console.log('\n=== ML Model Training ===');

    console.log('\nTraining fraud detection model...');
    try {
        const result = await client.sendCommand([
            'ML.TRAIN', 'fraud_detector', 'transactions:*',
            'EPOCHS', '100'
        ]);
        console.log('Training result:', result);
    } catch (e) {
        console.log('ML.TRAIN:', e.message);
    }
}

async function examplePredictions(client) {
    console.log('\n=== ML Predictions ===');

    // Single prediction
    console.log('\nPredicting fraud for new transaction...');
    try {
        const result = await client.sendCommand([
            'ML.PREDICT', 'fraud_detector', '[500.0, 2]'
        ]);
        console.log('Fraud prediction for $500 at 2am:', result);
    } catch (e) {
        console.log('ML.PREDICT:', e.message);
    }

    // Prediction with score
    console.log('\nPredicting with confidence score...');
    try {
        const result = await client.sendCommand([
            'ML.PREDICT.SCORE', 'fraud_detector', '[1500.0, 3]'
        ]);
        console.log('Prediction with score:', result);
    } catch (e) {
        console.log('ML.PREDICT.SCORE:', e.message);
    }

    // Batch predictions
    console.log('\nBatch predictions...');
    try {
        const result = await client.sendCommand([
            'ML.PREDICT.BATCH', 'fraud_detector', 'transaction:*',
            'LIMIT', '10'
        ]);
        console.log('Batch prediction results:', result);
    } catch (e) {
        console.log('ML.PREDICT.BATCH:', e.message);
    }
}

async function exampleVectorOperations(client) {
    console.log('\n=== Vector Operations ===');

    // Generate embedding
    console.log('\nGenerating text embedding...');
    try {
        const embedding = await client.sendCommand([
            'ML.EMBED', 'machine learning tutorial', 'sentence-transformers'
        ]);
        const firstFive = Array.isArray(embedding) ? embedding.slice(0, 5) : 'N/A';
        console.log('Embedding generated (first 5 dims):', firstFive, '...');
    } catch (e) {
        console.log('ML.EMBED:', e.message);
    }

    // Semantic search
    console.log('\nSemantic search...');
    try {
        const results = await client.sendCommand([
            'ML.SEARCH.SEMANTIC', 'documents', 'how to train models',
            'LIMIT', '5'
        ]);
        console.log('Semantic search results:', results);
    } catch (e) {
        console.log('ML.SEARCH.SEMANTIC:', e.message);
    }
}

async function exampleIndustryModels(client) {
    console.log('\n=== Industry Models ===');

    // Healthcare
    console.log('\nHealthcare - Disease Risk Prediction...');
    try {
        const patientData = JSON.stringify({
            age: 45,
            bmi: 28.5,
            blood_pressure: 140,
            glucose: 126
        });
        const result = await client.sendCommand([
            'ML.HEALTHCARE.PREDICT', 'diabetes_risk', patientData
        ]);
        console.log('Diabetes risk prediction:', result);
    } catch (e) {
        console.log('ML.HEALTHCARE.PREDICT:', e.message);
    }

    // Finance
    console.log('\nFinance - Credit Risk Assessment...');
    try {
        const customerData = JSON.stringify({
            income: 75000,
            debt_ratio: 0.35,
            credit_history_years: 8
        });
        const result = await client.sendCommand([
            'ML.FINANCE.PREDICT', 'credit_risk', customerData
        ]);
        console.log('Credit risk assessment:', result);
    } catch (e) {
        console.log('ML.FINANCE.PREDICT:', e.message);
    }

    // Retail
    console.log('\nRetail - Demand Forecast...');
    try {
        const productData = JSON.stringify({
            product_id: 'SKU-12345',
            historical_sales: [100, 120, 95, 140, 160],
            season: 'summer'
        });
        const result = await client.sendCommand([
            'ML.RETAIL.PREDICT', 'demand_forecast', productData
        ]);
        console.log('Demand forecast:', result);
    } catch (e) {
        console.log('ML.RETAIL.PREDICT:', e.message);
    }
}

async function exampleTimeSeries(client) {
    console.log('\n=== Time Series ML ===');

    // Store time series data
    console.log('\nStoring time series data...');
    const salesData = [100, 120, 95, 140, 160, 155, 180, 175, 190, 210];
    for (let i = 0; i < salesData.length; i++) {
        await client.zAdd('sales:daily', { score: salesData[i], value: `day:${i}` });
    }

    // Forecast
    console.log('\nForecasting future sales...');
    try {
        const result = await client.sendCommand([
            'ML.FORECAST', 'sales:daily', '7'
        ]);
        console.log('Sales forecast (next 7 days):', result);
    } catch (e) {
        console.log('ML.FORECAST:', e.message);
    }

    // Anomaly detection
    console.log('\nDetecting anomalies...');
    try {
        const result = await client.sendCommand([
            'ML.ANOMALY.DETECT', 'sales:daily'
        ]);
        console.log('Anomaly detection result:', result);
    } catch (e) {
        console.log('ML.ANOMALY.DETECT:', e.message);
    }
}

// Run main function
main().catch(console.error);
