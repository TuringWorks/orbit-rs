-- ============================================================================
-- OrbitRS Telco Examples - Billing Schema
-- ============================================================================
-- Billing, invoices, payments, charges, and credits
-- ============================================================================
-- ============================================================================
-- INVOICES
-- ============================================================================
CREATE TABLE invoices (
    invoice_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    invoice_number VARCHAR(50) UNIQUE NOT NULL,
    -- Account
    account_id UUID NOT NULL REFERENCES accounts(account_id),
    subscriber_id UUID NOT NULL REFERENCES subscribers(subscriber_id),
    -- Billing Period
    billing_period_start DATE NOT NULL,
    billing_period_end DATE NOT NULL,
    billing_cycle_day INTEGER,
    -- Amounts
    subtotal DECIMAL(12, 2) NOT NULL DEFAULT 0,
    tax_amount DECIMAL(12, 2) DEFAULT 0,
    fees_amount DECIMAL(12, 2) DEFAULT 0,
    adjustments_amount DECIMAL(12, 2) DEFAULT 0,
    credits_applied DECIMAL(12, 2) DEFAULT 0,
    total_amount DECIMAL(12, 2) NOT NULL,
    amount_due DECIMAL(12, 2) NOT NULL,
    previous_balance DECIMAL(12, 2) DEFAULT 0,
    -- Status
    status VARCHAR(20) DEFAULT 'DRAFT' CHECK (
        status IN (
            'DRAFT',
            'ISSUED',
            'SENT',
            'PAID',
            'PARTIAL_PAID',
            'OVERDUE',
            'CANCELLED',
            'REFUNDED'
        )
    ),
    -- Dates
    issue_date DATE NOT NULL,
    due_date DATE NOT NULL,
    paid_date DATE,
    -- Payment
    payment_method VARCHAR(20),
    payment_reference VARCHAR(100),
    -- Document
    document_url VARCHAR(500),
    -- PDF stored in MongoDB/S3
    -- Metadata
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_invoices_account ON invoices(account_id);
CREATE INDEX idx_invoices_subscriber ON invoices(subscriber_id);
CREATE INDEX idx_invoices_status ON invoices(status);
CREATE INDEX idx_invoices_due_date ON invoices(due_date);
CREATE INDEX idx_invoices_billing_period ON invoices(billing_period_start, billing_period_end);
-- ============================================================================
-- INVOICE ITEMS
-- ============================================================================
CREATE TABLE invoice_items (
    item_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    invoice_id UUID NOT NULL REFERENCES invoices(invoice_id) ON DELETE CASCADE,
    -- Item Details
    item_type VARCHAR(50) NOT NULL CHECK (
        item_type IN (
            'SUBSCRIPTION',
            'USAGE_VOICE',
            'USAGE_DATA',
            'USAGE_SMS',
            'USAGE_ROAMING',
            'ONE_TIME_CHARGE',
            'EQUIPMENT',
            'ACTIVATION_FEE',
            'LATE_FEE',
            'ADJUSTMENT',
            'CREDIT',
            'TAX',
            'SURCHARGE'
        )
    ),
    description TEXT NOT NULL,
    -- Service Period
    service_period_start DATE,
    service_period_end DATE,
    -- Quantity and Rate
    quantity DECIMAL(15, 4) DEFAULT 1,
    -- Can be minutes, MB, SMS count, etc.
    unit_of_measure VARCHAR(20),
    -- MINUTES, MB, GB, SMS, EACH
    unit_price DECIMAL(10, 6),
    -- Amount
    amount DECIMAL(12, 2) NOT NULL,
    tax_amount DECIMAL(12, 2) DEFAULT 0,
    total_amount DECIMAL(12, 2) NOT NULL,
    -- References
    plan_id UUID,
    usage_record_id UUID,
    -- Metadata
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_invoice_items_invoice ON invoice_items(invoice_id);
CREATE INDEX idx_invoice_items_type ON invoice_items(item_type);
-- ============================================================================
-- PAYMENTS
-- ============================================================================
CREATE TABLE payments (
    payment_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    payment_number VARCHAR(50) UNIQUE NOT NULL,
    -- Account
    account_id UUID NOT NULL REFERENCES accounts(account_id),
    subscriber_id UUID NOT NULL REFERENCES subscribers(subscriber_id),
    invoice_id UUID REFERENCES invoices(invoice_id),
    -- Amount
    amount DECIMAL(12, 2) NOT NULL,
    currency VARCHAR(3) DEFAULT 'USD',
    -- Payment Method
    payment_method VARCHAR(30) NOT NULL CHECK (
        payment_method IN (
            'CREDIT_CARD',
            'DEBIT_CARD',
            'ACH',
            'BANK_TRANSFER',
            'CHECK',
            'CASH',
            'PAYPAL',
            'APPLE_PAY',
            'GOOGLE_PAY',
            'CRYPTOCURRENCY'
        )
    ),
    payment_method_id UUID REFERENCES payment_methods(payment_method_id),
    -- Transaction Details
    transaction_id VARCHAR(100),
    authorization_code VARCHAR(50),
    confirmation_number VARCHAR(100),
    -- Payment Processor
    processor VARCHAR(50),
    -- Stripe, Square, PayPal, etc.
    processor_transaction_id VARCHAR(100),
    processor_fee DECIMAL(10, 2),
    -- Status
    status VARCHAR(20) DEFAULT 'PENDING' CHECK (
        status IN (
            'PENDING',
            'PROCESSING',
            'AUTHORIZED',
            'CAPTURED',
            'COMPLETED',
            'FAILED',
            'DECLINED',
            'REFUNDED',
            'CHARGEBACK',
            'CANCELLED'
        )
    ),
    failure_reason TEXT,
    -- Dates
    payment_date DATE NOT NULL,
    processed_date TIMESTAMP,
    cleared_date DATE,
    -- Refund
    refund_amount DECIMAL(12, 2) DEFAULT 0,
    refund_date DATE,
    refund_reason TEXT,
    -- Retry
    retry_count INTEGER DEFAULT 0,
    next_retry_date DATE,
    -- Metadata
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_payments_account ON payments(account_id);
CREATE INDEX idx_payments_subscriber ON payments(subscriber_id);
CREATE INDEX idx_payments_invoice ON payments(invoice_id);
CREATE INDEX idx_payments_status ON payments(status);
CREATE INDEX idx_payments_date ON payments(payment_date);
-- ============================================================================
-- PAYMENT METHODS
-- ============================================================================
CREATE TABLE payment_methods (
    payment_method_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    account_id UUID NOT NULL REFERENCES accounts(account_id),
    subscriber_id UUID NOT NULL REFERENCES subscribers(subscriber_id),
    -- Method Type
    method_type VARCHAR(30) NOT NULL CHECK (
        method_type IN (
            'CREDIT_CARD',
            'DEBIT_CARD',
            'BANK_ACCOUNT',
            'PAYPAL',
            'DIGITAL_WALLET'
        )
    ),
    -- Card Details (encrypted/tokenized)
    card_last_four VARCHAR(4),
    card_brand VARCHAR(20),
    -- VISA, MASTERCARD, AMEX, DISCOVER
    card_expiry_month INTEGER CHECK (
        card_expiry_month BETWEEN 1 AND 12
    ),
    card_expiry_year INTEGER,
    card_holder_name VARCHAR(200),
    -- Bank Account (encrypted/tokenized)
    bank_name VARCHAR(100),
    account_last_four VARCHAR(4),
    routing_number_last_four VARCHAR(4),
    account_type VARCHAR(20) CHECK (account_type IN ('CHECKING', 'SAVINGS')),
    -- Tokenization
    payment_token VARCHAR(255),
    -- Token from payment processor
    processor VARCHAR(50),
    -- Billing Address
    billing_address_id UUID REFERENCES addresses(address_id),
    -- Preferences
    is_default BOOLEAN DEFAULT FALSE,
    is_auto_pay BOOLEAN DEFAULT FALSE,
    -- Verification
    is_verified BOOLEAN DEFAULT FALSE,
    verified_date DATE,
    verification_amount DECIMAL(10, 2),
    -- Micro-deposit amount
    -- Status
    status VARCHAR(20) DEFAULT 'ACTIVE' CHECK (
        status IN ('ACTIVE', 'EXPIRED', 'INVALID', 'REMOVED')
    ),
    -- Metadata
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_payment_methods_account ON payment_methods(account_id);
CREATE INDEX idx_payment_methods_subscriber ON payment_methods(subscriber_id);
CREATE INDEX idx_payment_methods_default ON payment_methods(is_default)
WHERE is_default = TRUE;
-- ============================================================================
-- CHARGES
-- ============================================================================
CREATE TABLE charges (
    charge_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    -- Account
    account_id UUID NOT NULL REFERENCES accounts(account_id),
    subscriber_id UUID NOT NULL REFERENCES subscribers(subscriber_id),
    -- Charge Details
    charge_type VARCHAR(50) NOT NULL CHECK (
        charge_type IN (
            'SUBSCRIPTION',
            'USAGE',
            'ONE_TIME',
            'RECURRING',
            'PENALTY',
            'ADJUSTMENT'
        )
    ),
    charge_category VARCHAR(50),
    description TEXT NOT NULL,
    -- Amount
    amount DECIMAL(12, 2) NOT NULL,
    tax_amount DECIMAL(12, 2) DEFAULT 0,
    total_amount DECIMAL(12, 2) NOT NULL,
    -- Service Period
    service_period_start DATE,
    service_period_end DATE,
    -- Status
    status VARCHAR(20) DEFAULT 'PENDING' CHECK (
        status IN (
            'PENDING',
            'BILLED',
            'PAID',
            'DISPUTED',
            'WAIVED',
            'CANCELLED'
        )
    ),
    -- Billing
    invoice_id UUID REFERENCES invoices(invoice_id),
    billed_date DATE,
    -- References
    plan_id UUID,
    usage_record_id UUID,
    -- Metadata
    charge_date DATE DEFAULT CURRENT_DATE,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_charges_account ON charges(account_id);
CREATE INDEX idx_charges_subscriber ON charges(subscriber_id);
CREATE INDEX idx_charges_status ON charges(status);
CREATE INDEX idx_charges_date ON charges(charge_date);
CREATE INDEX idx_charges_invoice ON charges(invoice_id);
-- ============================================================================
-- CREDITS
-- ============================================================================
CREATE TABLE credits (
    credit_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    -- Account
    account_id UUID NOT NULL REFERENCES accounts(account_id),
    subscriber_id UUID NOT NULL REFERENCES subscribers(subscriber_id),
    -- Credit Details
    credit_type VARCHAR(50) NOT NULL CHECK (
        credit_type IN (
            'REFUND',
            'ADJUSTMENT',
            'PROMOTIONAL',
            'LOYALTY',
            'GOODWILL',
            'ERROR_CORRECTION'
        )
    ),
    description TEXT NOT NULL,
    reason_code VARCHAR(50),
    -- Amount
    amount DECIMAL(12, 2) NOT NULL,
    -- Application
    status VARCHAR(20) DEFAULT 'PENDING' CHECK (
        status IN (
            'PENDING',
            'APPROVED',
            'APPLIED',
            'EXPIRED',
            'CANCELLED'
        )
    ),
    applied_to_invoice_id UUID REFERENCES invoices(invoice_id),
    applied_date DATE,
    -- Expiration
    expiry_date DATE,
    -- Approval
    requires_approval BOOLEAN DEFAULT FALSE,
    approved_by VARCHAR(100),
    approved_date DATE,
    -- Metadata
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    created_by VARCHAR(100),
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_credits_account ON credits(account_id);
CREATE INDEX idx_credits_subscriber ON credits(subscriber_id);
CREATE INDEX idx_credits_status ON credits(status);
CREATE INDEX idx_credits_expiry ON credits(expiry_date);
-- ============================================================================
-- BILLING CYCLES
-- ============================================================================
CREATE TABLE billing_cycles (
    cycle_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    -- Cycle Definition
    cycle_name VARCHAR(50) NOT NULL,
    cycle_day INTEGER NOT NULL CHECK (
        cycle_day BETWEEN 1 AND 28
    ),
    -- Period
    period_start DATE NOT NULL,
    period_end DATE NOT NULL,
    -- Processing
    status VARCHAR(20) DEFAULT 'PENDING' CHECK (
        status IN (
            'PENDING',
            'PROCESSING',
            'COMPLETED',
            'FAILED'
        )
    ),
    -- Statistics
    total_accounts INTEGER DEFAULT 0,
    processed_accounts INTEGER DEFAULT 0,
    failed_accounts INTEGER DEFAULT 0,
    total_billed_amount DECIMAL(15, 2) DEFAULT 0,
    -- Dates
    processing_started_at TIMESTAMP,
    processing_completed_at TIMESTAMP,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_billing_cycles_period ON billing_cycles(period_start, period_end);
CREATE INDEX idx_billing_cycles_status ON billing_cycles(status);
-- ============================================================================
-- PAYMENT PLANS
-- ============================================================================
CREATE TABLE payment_plans (
    payment_plan_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    -- Account
    account_id UUID NOT NULL REFERENCES accounts(account_id),
    subscriber_id UUID NOT NULL REFERENCES subscribers(subscriber_id),
    -- Plan Details
    plan_type VARCHAR(20) CHECK (
        plan_type IN ('INSTALLMENT', 'DEFERRED', 'CUSTOM')
    ),
    total_amount DECIMAL(12, 2) NOT NULL,
    down_payment DECIMAL(12, 2) DEFAULT 0,
    remaining_balance DECIMAL(12, 2) NOT NULL,
    -- Schedule
    number_of_payments INTEGER NOT NULL,
    payment_frequency VARCHAR(20) CHECK (
        payment_frequency IN ('WEEKLY', 'BI_WEEKLY', 'MONTHLY')
    ),
    payment_amount DECIMAL(12, 2) NOT NULL,
    -- Dates
    start_date DATE NOT NULL,
    end_date DATE NOT NULL,
    next_payment_date DATE,
    -- Status
    status VARCHAR(20) DEFAULT 'ACTIVE' CHECK (
        status IN (
            'ACTIVE',
            'COMPLETED',
            'DEFAULTED',
            'CANCELLED'
        )
    ),
    -- Tracking
    payments_made INTEGER DEFAULT 0,
    payments_missed INTEGER DEFAULT 0,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_payment_plans_account ON payment_plans(account_id);
CREATE INDEX idx_payment_plans_status ON payment_plans(status);
-- ============================================================================
-- TRIGGERS
-- ============================================================================
CREATE TRIGGER update_invoices_updated_at BEFORE
UPDATE ON invoices FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
CREATE TRIGGER update_payments_updated_at BEFORE
UPDATE ON payments FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
CREATE TRIGGER update_payment_methods_updated_at BEFORE
UPDATE ON payment_methods FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
CREATE TRIGGER update_charges_updated_at BEFORE
UPDATE ON charges FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
CREATE TRIGGER update_credits_updated_at BEFORE
UPDATE ON credits FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
CREATE TRIGGER update_payment_plans_updated_at BEFORE
UPDATE ON payment_plans FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
-- ============================================================================
-- VIEWS
-- ============================================================================
-- Outstanding invoices
CREATE VIEW v_outstanding_invoices AS
SELECT i.invoice_id,
    i.invoice_number,
    i.account_id,
    a.account_number,
    s.msisdn,
    s.first_name,
    s.last_name,
    i.total_amount,
    i.amount_due,
    i.due_date,
    i.status,
    CURRENT_DATE - i.due_date AS days_overdue
FROM invoices i
    JOIN accounts a ON i.account_id = a.account_id
    JOIN subscribers s ON i.subscriber_id = s.subscriber_id
WHERE i.status IN ('ISSUED', 'SENT', 'PARTIAL_PAID', 'OVERDUE')
    AND i.amount_due > 0
ORDER BY i.due_date;
-- Account balance summary
CREATE VIEW v_account_balances AS
SELECT a.account_id,
    a.account_number,
    s.msisdn,
    SUM(
        CASE
            WHEN i.status IN ('ISSUED', 'SENT', 'PARTIAL_PAID', 'OVERDUE') THEN i.amount_due
            ELSE 0
        END
    ) AS outstanding_balance,
    SUM(
        CASE
            WHEN i.status = 'OVERDUE' THEN i.amount_due
            ELSE 0
        END
    ) AS overdue_balance,
    SUM(
        CASE
            WHEN p.status = 'COMPLETED' THEN p.amount
            ELSE 0
        END
    ) AS total_payments,
    COUNT(
        DISTINCT CASE
            WHEN i.status = 'OVERDUE' THEN i.invoice_id
        END
    ) AS overdue_invoice_count
FROM accounts a
    JOIN subscribers s ON a.primary_subscriber_id = s.subscriber_id
    LEFT JOIN invoices i ON a.account_id = i.account_id
    LEFT JOIN payments p ON a.account_id = p.account_id
GROUP BY a.account_id,
    a.account_number,
    s.msisdn;
-- ============================================================================
-- COMMENTS
-- ============================================================================
COMMENT ON TABLE invoices IS 'Monthly billing invoices';
COMMENT ON TABLE invoice_items IS 'Line items on invoices';
COMMENT ON TABLE payments IS 'Payment transactions';
COMMENT ON TABLE payment_methods IS 'Stored payment methods';
COMMENT ON TABLE charges IS 'Charges to be billed';
COMMENT ON TABLE credits IS 'Account credits and adjustments';
COMMENT ON TABLE billing_cycles IS 'Billing cycle processing tracking';
COMMENT ON TABLE payment_plans IS 'Installment payment arrangements';