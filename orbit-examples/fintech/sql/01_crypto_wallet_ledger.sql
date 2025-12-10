-- FinTech: Crypto/Digital Wallet Ledger (SQL)
-- Purpose: A rigid, double-entry bookkeeping system for a Digital Wallet or Exchange.
-- Ensures zero-sum consistency for all internal transfers and strict locking for withdrawals.
-- 1. Accounts Table
-- Represents a user's balance bucket for a specific currency.
CREATE TABLE accounts (
    account_id UUID PRIMARY KEY,
    user_id UUID NOT NULL,
    currency_code VARCHAR(10) NOT NULL,
    -- 'USD', 'BTC', 'ETH'
    balance DECIMAL(36, 18) DEFAULT 0,
    -- High precision for crypto
    hold_balance DECIMAL(36, 18) DEFAULT 0,
    -- Locked funds (e.g. open orders)
    created_at TIMESTAMP DEFAULT NOW(),
    version INT DEFAULT 0,
    -- Optimistic locking
    UNIQUE(user_id, currency_code)
);
-- 2. Ledger Entries (The Source of Truth)
-- Immutable record of every balance change.
-- Double Entry Rule: Sum of 'amount' for a given transaction_ref must be 0 (for internal transfers).
CREATE TABLE ledger_entries (
    entry_id UUID PRIMARY KEY,
    transaction_ref UUID NOT NULL,
    -- ID of the overarching transaction
    account_id UUID REFERENCES accounts(account_id),
    amount DECIMAL(36, 18) NOT NULL,
    -- Negative for Debit, Positive for Credit
    entry_type VARCHAR(50),
    -- 'DEPOSIT', 'WITHDRAWAL', 'TRADE', 'FEE'
    created_at TIMESTAMP DEFAULT NOW()
);
CREATE INDEX idx_ledger_account ON ledger_entries(account_id);
CREATE INDEX idx_ledger_tx_ref ON ledger_entries(transaction_ref);
-- 3. Stored Procedure: Atomic Transfer
-- Example of Orbit SQL PL/pgSQL capability for safety
CREATE OR REPLACE FUNCTION transfer_funds(
        p_from_account UUID,
        p_to_account UUID,
        p_amount DECIMAL,
        p_tx_ref UUID
    ) RETURNS VOID AS $$ BEGIN -- Debit Sender
UPDATE accounts
SET balance = balance - p_amount,
    version = version + 1
WHERE account_id = p_from_account
    AND balance >= p_amount;
IF NOT FOUND THEN RAISE EXCEPTION 'Insufficient funds or concurrent modification';
END IF;
-- Credit Receiver
UPDATE accounts
SET balance = balance + p_amount,
    version = version + 1
WHERE account_id = p_to_account;
-- Write Ledger
INSERT INTO ledger_entries (
        entry_id,
        transaction_ref,
        account_id,
        amount,
        entry_type
    )
VALUES (
        gen_random_uuid(),
        p_tx_ref,
        p_from_account,
        - p_amount,
        'INTERNAL_TRANSFER'
    ),
    (
        gen_random_uuid(),
        p_tx_ref,
        p_to_account,
        p_amount,
        'INTERNAL_TRANSFER'
    );
END;
$$ LANGUAGE plpgsql;
-- Example Usage
-- User A (BTC) -> User B (BTC)
INSERT INTO accounts (account_id, user_id, currency_code, balance)
VALUES ('aaa-111', 'u-alice', 'BTC', 1.50000000),
    ('bbb-222', 'u-bob', 'BTC', 0.00000000);
-- Execute Transfer
SELECT transfer_funds('aaa-111', 'bbb-222', 0.1, gen_random_uuid());