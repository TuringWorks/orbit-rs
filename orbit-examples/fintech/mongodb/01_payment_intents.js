/**
 * FinTech Use Case: Payment Processor Core
 * Purpose: Manage complex payment lifecycles (Intents) and audit logs/webhooks.
 * Document model allows storing varied metadata from upstream gateways (Stripe/PayPal)
 * without strict schema changes.
 */

// 1. Payment Intent
// The state of a payment session.
db.payment_intents.insertOne({
    "intent_id": "pi_123456789",
    "merchant_id": "mer_acme_inc",
    "amount": 5000, // in cents (50.00)
    "currency": "usd",
    "status": "requires_payment_method", // -> processing -> succeeded
    "payment_method_types": ["card", "apple_pay"],
    "created_at": ISODate("2024-12-09T12:00:00Z"),

    // Rich metadata from checkout context
    "metadata": {
        "order_id": "ord_555",
        "customer_segment": "enterprise"
    },

    // Detailed breakdown often required for invoices
    "charges": [],

    // Gateway specific debug info (Polymorphic)
    "gateway_data": {
        "provider": "stripe",
        "client_secret": "pi_..._secret_...",
        "risk_score": 45
    }
});

// 2. State Transition (Atomic Update)
// User submits card -> transition to 'processing'
db.payment_intents.updateOne(
    { "intent_id": "pi_123456789", "status": "requires_payment_method" },
    {
        $set: {
            "status": "processing",
            "updated_at": new Date()
        },
        $push: {
            "timeline": {
                "status": "processing",
                "ts": new Date(),
                "note": "Card confirmed by frontend"
            }
        }
    }
);

// 3. Webhook Event Log
// Immutable log of every event sent to the merchant's webhook URL.
// Crucial for debugging "Why didn't my server get the notification?"
db.webhook_events.insertOne({
    "event_id": "evt_999",
    "intent_id": "pi_123456789",
    "type": "payment_intent.succeeded",
    "payload": {
        "id": "pi_123456789",
        "amount": 5000,
        "status": "succeeded"
    },
    // Delivery attempts array
    "delivery_attempts": [
        {
            "url": "https://api.merchant.com/webhooks",
            "ts": ISODate("2024-12-09T12:05:00Z"),
            "response_code": 200,
            "latency_ms": 145,
            "success": true
        }
    ]
});

// 4. Reconciliation Query
// Find payments that succeeded 3 days ago but haven't been marked as 'settled' in our internal ledger.
// (Cross-reference logic usually done in App code, but query helps)
db.payment_intents.find({
    "status": "succeeded",
    "created_at": { $lt: new Date(Date.now() - 3 * 24 * 60 * 60 * 1000) },
    "metadata.ledger_settled": { $ne: true }
});
