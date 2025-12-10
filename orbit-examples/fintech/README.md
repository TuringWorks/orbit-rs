# FinTech Industry Examples

This directory contains examples demonstrating Orbit-RS in the modern Financial Technology (FinTech) sector, focusing on Payment Processing, High-Frequency Trading (HFT), and Crypto/Digital Ledger Technology.

## Scenarios

### 1. Crypto & Digital Wallet Ledger (SQL)
- **File**: `sql/01_crypto_wallet_ledger.sql`
- **Description**: Robust Double-Entry Bookkeeping schema for managing user balances across multiple currencies.
- **Features**: ACID transactions, Optimistic Locking, and strict Zero-Sum internal transfer logic.

### 2. High-Frequency Trading (HFT) (Redis)
- **File**: `redis/01_hft_orderbook.redis`
- **Description**: Low-latency Order Book and Matching Engine structures.
- **Features**: 
    - **Order Book**: Sorted Sets for Price-Time priority.
    - **Pub/Sub**: Real-time ticker updates.
    - **Idempotency**: Preventing double-execution of trade commands.

### 3. Payment Intents & Gateways (MongoDB)
- **File**: `mongodb/01_payment_intents.js`
- **Description**: Flexible Payment state machine (similar to Stripe PaymentIntents).
- **Features**: Complex lifecycle tracking, Webhook event logging, and rich metadata storage.

### 4. P2P Social Payments (Cypher)
- **File**: `cypher/01_p2p_fraud_ring.cypher`
- **Description**: Graph analysis of Peer-to-Peer payment networks.
- **Features**: Detecting circular money flows ("cycling"), mule accounts, and social graph fraud rings.

## Workflows

### 01_payment_processing
- **File**: `workflows/01_payment_processing.md`
- **Description**: End-to-end flow of a Payment Transaction, from Intent creation to Ledger Capture and Webhook notification.
