// ============================================================================
// OrbitRS Banking Examples - Audit Logs (MongoDB)
// ============================================================================
// Immutable log of user actions and system events
// ============================================================================

db = db.getSiblingDB('banking_audit');

db.createCollection("audit_logs", {
    capped: true,
    size: 5242880, // 5MB capped collection
    max: 5000
});

db.audit_logs.insertMany([
    {
        event_id: "EVT-1001",
        timestamp: new Date(),
        action: "LOGIN_ATTEMPT",
        user_id: "user_1001",
        ip_address: "192.168.1.50",
        status: "SUCCESS",
        details: {
            device: "iPhone 15",
            os: "iOS 17.4"
        }
    },
    {
        event_id: "EVT-1002",
        timestamp: new Date(),
        action: "TRANSFER_INITIATED",
        user_id: "user_1001",
        amount: 500.00,
        currency: "USD",
        recipient: "user_2002",
        status: "PENDING_FRAUD_CHECK"
    }
]);

// Find failed logins in last hour
db.audit_logs.find({
    action: "LOGIN_ATTEMPT",
    status: "FAILURE",
    timestamp: { $gt: new Date(Date.now() - 3600000) }
});
