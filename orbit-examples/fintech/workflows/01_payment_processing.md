# Workflow: Online Payment Processing

## Overview
A high-reliability flow for processing online payments, mirroring a modern Payment Gateway architecture (Stripe/Adyen).

## Workflow Steps

### 1. Intent Creation (MongoDB)
**Actor**: Checkout Service  
**Action**: Create a `PaymentIntent` document to track the lifecycle.
```js
db.payment_intents.insertOne({ status: "requires_payment_method", ... })
```

### 2. Risk Evaluation (Redis)
**System**: Anti-Fraud Service  
**Action**: Check velocity limits and idempotency.
-   Key: `velocity:ip:1.2.3.4`
-   Key: `idempotency:req_abc`
-   **Decision**: If Risk Score > 80, Block.

### 3. Gateway Transaction (External)
**Action**: Call Visa/Mastercard/Bank network.
-   **Result**: `Approved` (Auth Code: 123456)

### 4. Ledger Capture (SQL)
**System**: Orbit SQL Engine  
**Action**: Once approved, money must be immutable. Record in the Double-Entry Ledger.
```sql
BEGIN;
INSERT INTO ledger_entries (...) VALUES (..., 'MERCHANT_CREDIT', 50.00);
INSERT INTO ledger_entries (...) VALUES (..., 'CUSTOMER_DEBIT', -50.00);
COMMIT;
```

### 5. Finalize State (MongoDB)
**Action**: Update Intent status to `succeeded` and log the Ledger Transaction ID.

### 6. Async Notification (Webhook)
**Action**: Push event to Merchant's URL so they can ship the goods.
```json
POST /webhooks
{ "type": "payment_intent.succeeded" }
```
