-- Healthcare Core: EHR (Electronic Health Record) - Relational Schema
-- Purpose: Store rigid, structured patient demographic and encounter data.
-- Compliance: Designed with audit trails and exact types for HIPAA/GDPR support.
-- 1. Patients Table
-- The master patient index.
CREATE TABLE patients (
    patient_id UUID PRIMARY KEY,
    mrn VARCHAR(50) UNIQUE NOT NULL,
    -- Medical Record Number (Internal)
    first_name VARCHAR(100) NOT NULL,
    last_name VARCHAR(100) NOT NULL,
    dob DATE NOT NULL,
    gender VARCHAR(20),
    ssn_hash VARCHAR(256),
    -- Store only hashed PII if possible
    status VARCHAR(20) DEFAULT 'ACTIVE',
    -- ACTIVE, DECEASED, MERGED
    created_at TIMESTAMP DEFAULT NOW(),
    updated_at TIMESTAMP DEFAULT NOW()
);
-- 2. Providers (Doctors, Nurses)
CREATE TABLE providers (
    provider_id UUID PRIMARY KEY,
    npi VARCHAR(20) UNIQUE NOT NULL,
    -- National Provider Identifier
    full_name VARCHAR(200) NOT NULL,
    specialty VARCHAR(100),
    is_active BOOLEAN DEFAULT TRUE
);
-- 3. Encounters (Visits)
-- Links a patient to a provider for a specific interaction.
CREATE TABLE encounters (
    encounter_id UUID PRIMARY KEY,
    patient_id UUID REFERENCES patients(patient_id),
    provider_id UUID REFERENCES providers(provider_id),
    visit_type VARCHAR(50),
    -- INPATIENT, OUTPATIENT, TELEHEALTH
    start_time TIMESTAMP NOT NULL,
    end_time TIMESTAMP,
    status VARCHAR(20),
    -- SCHEDULED, IN_PROGRESS, COMPLETED, CANCELLED
    reason_for_visit TEXT,
    -- Audit columns
    created_by UUID,
    -- System User ID
    created_at TIMESTAMP DEFAULT NOW()
);
CREATE INDEX idx_encounters_patient ON encounters(patient_id);
CREATE INDEX idx_encounters_date ON encounters(start_time);
-- 4. Audit Log (Immutable)
-- Essential for compliance. Who accessed what and when.
CREATE TABLE access_logs (
    log_id UUID PRIMARY KEY,
    user_id UUID,
    resource_type VARCHAR(50),
    -- 'PATIENT', 'ENCOUNTER'
    resource_id UUID,
    action_type VARCHAR(20),
    -- 'VIEW', 'UPDATE', 'EXPORT'
    accessed_at TIMESTAMP DEFAULT NOW(),
    ip_address VARCHAR(45)
);
-- Example Usage
-- Register New Patient
INSERT INTO patients (
        patient_id,
        mrn,
        first_name,
        last_name,
        dob,
        gender
    )
VALUES (
        gen_random_uuid(),
        'MRN-2024-001',
        'John',
        'Doe',
        '1980-01-15',
        'MALE'
    );
-- Schedule Telehealth Visit
INSERT INTO encounters (
        encounter_id,
        patient_id,
        provider_id,
        visit_type,
        start_time,
        status
    )
VALUES (
        gen_random_uuid(),
        (
            SELECT patient_id
            FROM patients
            WHERE mrn = 'MRN-2024-001'
        ),
        NULL,
        -- Provider assigned later
        'TELEHEALTH',
        NOW() + INTERVAL '1 DAY',
        'SCHEDULED'
    );