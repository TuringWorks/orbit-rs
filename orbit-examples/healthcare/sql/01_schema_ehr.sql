-- ============================================================================
-- OrbitRS Healthcare Examples - Electronic Health Records Schema
-- ============================================================================
-- Patients, appointments, prescriptions, medical history with HIPAA compliance
-- ============================================================================
CREATE EXTENSION IF NOT EXISTS "uuid-ossp";
-- ============================================================================
-- PATIENTS
-- ============================================================================
CREATE TABLE patients (
    patient_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    mrn VARCHAR(20) UNIQUE NOT NULL,
    -- Medical Record Number
    -- Personal Info (PHI - Protected Health Information)
    first_name VARCHAR(100) NOT NULL,
    last_name VARCHAR(100) NOT NULL,
    date_of_birth DATE NOT NULL,
    ssn_encrypted VARCHAR(255),
    -- Encrypted
    -- Contact
    email VARCHAR(255),
    phone VARCHAR(20),
    address_line1 VARCHAR(255),
    city VARCHAR(100),
    state VARCHAR(2),
    zip_code VARCHAR(10),
    -- Demographics
    gender VARCHAR(20),
    race VARCHAR(50),
    ethnicity VARCHAR(50),
    preferred_language VARCHAR(50),
    -- Emergency Contact
    emergency_contact_name VARCHAR(200),
    emergency_contact_phone VARCHAR(20),
    emergency_contact_relationship VARCHAR(50),
    -- Insurance
    insurance_provider VARCHAR(200),
    insurance_policy_number VARCHAR(100),
    -- ML Risk Scores
    readmission_risk_score DECIMAL(5, 4),
    -- 0.0000 to 1.0000
    chronic_disease_risk DECIMAL(5, 4),
    -- Status
    status VARCHAR(20) DEFAULT 'ACTIVE' CHECK (status IN ('ACTIVE', 'INACTIVE', 'DECEASED')),
    -- HIPAA Audit
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    created_by UUID,
    updated_by UUID
);
CREATE INDEX idx_patients_mrn ON patients(mrn);
CREATE INDEX idx_patients_dob ON patients(date_of_birth);
CREATE INDEX idx_patients_status ON patients(status);
-- ============================================================================
-- PROVIDERS (Doctors, Nurses, etc.)
-- ============================================================================
CREATE TABLE providers (
    provider_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    npi VARCHAR(10) UNIQUE NOT NULL,
    -- National Provider Identifier
    -- Personal Info
    first_name VARCHAR(100) NOT NULL,
    last_name VARCHAR(100) NOT NULL,
    -- Professional
    specialty VARCHAR(100),
    license_number VARCHAR(50),
    license_state VARCHAR(2),
    -- Contact
    email VARCHAR(255),
    phone VARCHAR(20),
    -- Status
    is_active BOOLEAN DEFAULT TRUE,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_providers_specialty ON providers(specialty);
-- ============================================================================
-- APPOINTMENTS
-- ============================================================================
CREATE TABLE appointments (
    appointment_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    patient_id UUID NOT NULL REFERENCES patients(patient_id),
    provider_id UUID NOT NULL REFERENCES providers(provider_id),
    -- Appointment Details
    appointment_type VARCHAR(50) CHECK (
        appointment_type IN (
            'OFFICE_VISIT',
            'TELEMEDICINE',
            'PROCEDURE',
            'FOLLOW_UP',
            'EMERGENCY',
            'CONSULTATION'
        )
    ),
    -- Scheduling
    scheduled_start TIMESTAMP NOT NULL,
    scheduled_end TIMESTAMP NOT NULL,
    actual_start TIMESTAMP,
    actual_end TIMESTAMP,
    -- Status
    status VARCHAR(20) DEFAULT 'SCHEDULED' CHECK (
        status IN (
            'SCHEDULED',
            'CONFIRMED',
            'CHECKED_IN',
            'IN_PROGRESS',
            'COMPLETED',
            'CANCELLED',
            'NO_SHOW'
        )
    ),
    -- Reason
    chief_complaint TEXT,
    notes TEXT,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_appointments_patient ON appointments(patient_id);
CREATE INDEX idx_appointments_provider ON appointments(provider_id);
CREATE INDEX idx_appointments_scheduled ON appointments(scheduled_start);
CREATE INDEX idx_appointments_status ON appointments(status);
-- ============================================================================
-- DIAGNOSES (ICD-10 Codes)
-- ============================================================================
CREATE TABLE diagnoses (
    diagnosis_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    patient_id UUID NOT NULL REFERENCES patients(patient_id),
    provider_id UUID REFERENCES providers(provider_id),
    appointment_id UUID REFERENCES appointments(appointment_id),
    -- ICD-10 Code
    icd10_code VARCHAR(10) NOT NULL,
    description TEXT NOT NULL,
    -- Classification
    diagnosis_type VARCHAR(20) CHECK (
        diagnosis_type IN ('PRIMARY', 'SECONDARY', 'DIFFERENTIAL')
    ),
    -- Status
    status VARCHAR(20) DEFAULT 'ACTIVE' CHECK (status IN ('ACTIVE', 'RESOLVED', 'CHRONIC')),
    -- Dates
    diagnosed_date DATE NOT NULL,
    resolved_date DATE,
    -- ML Confidence
    ml_confidence DECIMAL(5, 4),
    -- If AI-assisted diagnosis
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_diagnoses_patient ON diagnoses(patient_id);
CREATE INDEX idx_diagnoses_icd10 ON diagnoses(icd10_code);
CREATE INDEX idx_diagnoses_status ON diagnoses(status);
-- ============================================================================
-- PRESCRIPTIONS
-- ============================================================================
CREATE TABLE prescriptions (
    prescription_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    patient_id UUID NOT NULL REFERENCES patients(patient_id),
    provider_id UUID NOT NULL REFERENCES providers(provider_id),
    -- Medication
    medication_name VARCHAR(200) NOT NULL,
    ndc_code VARCHAR(11),
    -- National Drug Code
    -- Dosage
    dosage VARCHAR(100) NOT NULL,
    frequency VARCHAR(100) NOT NULL,
    route VARCHAR(50),
    -- oral, IV, topical, etc.
    -- Quantity
    quantity INTEGER,
    refills INTEGER DEFAULT 0,
    -- Dates
    prescribed_date DATE NOT NULL,
    start_date DATE,
    end_date DATE,
    -- Instructions
    instructions TEXT,
    -- Drug Interaction Check
    interaction_checked BOOLEAN DEFAULT FALSE,
    interaction_warnings JSONB,
    -- Status
    status VARCHAR(20) DEFAULT 'ACTIVE' CHECK (
        status IN (
            'ACTIVE',
            'COMPLETED',
            'DISCONTINUED',
            'CANCELLED'
        )
    ),
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_prescriptions_patient ON prescriptions(patient_id);
CREATE INDEX idx_prescriptions_medication ON prescriptions(medication_name);
CREATE INDEX idx_prescriptions_status ON prescriptions(status);
-- ============================================================================
-- VITAL SIGNS
-- ============================================================================
CREATE TABLE vital_signs (
    vital_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    patient_id UUID NOT NULL REFERENCES patients(patient_id),
    appointment_id UUID REFERENCES appointments(appointment_id),
    -- Measurements
    temperature DECIMAL(4, 1),
    -- Fahrenheit
    blood_pressure_systolic INTEGER,
    blood_pressure_diastolic INTEGER,
    heart_rate INTEGER,
    respiratory_rate INTEGER,
    oxygen_saturation DECIMAL(5, 2),
    -- SpO2 percentage
    weight DECIMAL(5, 2),
    -- pounds
    height DECIMAL(5, 2),
    -- inches
    bmi DECIMAL(4, 2),
    -- Timestamp
    measured_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    -- Recorded By
    recorded_by UUID REFERENCES providers(provider_id),
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_vital_signs_patient ON vital_signs(patient_id);
CREATE INDEX idx_vital_signs_measured ON vital_signs(measured_at);
-- ============================================================================
-- LAB RESULTS
-- ============================================================================
CREATE TABLE lab_results (
    lab_result_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    patient_id UUID NOT NULL REFERENCES patients(patient_id),
    provider_id UUID REFERENCES providers(provider_id),
    -- Test Info
    test_name VARCHAR(200) NOT NULL,
    loinc_code VARCHAR(10),
    -- Logical Observation Identifiers Names and Codes
    -- Results
    result_value VARCHAR(100),
    result_unit VARCHAR(50),
    reference_range VARCHAR(100),
    -- Status
    status VARCHAR(20) DEFAULT 'PENDING' CHECK (
        status IN (
            'PENDING',
            'PRELIMINARY',
            'FINAL',
            'CORRECTED',
            'CANCELLED'
        )
    ),
    -- Abnormal Flag
    is_abnormal BOOLEAN DEFAULT FALSE,
    abnormal_flag VARCHAR(20),
    -- HIGH, LOW, CRITICAL
    -- Dates
    ordered_date TIMESTAMP,
    collected_date TIMESTAMP,
    resulted_date TIMESTAMP,
    -- Notes
    notes TEXT,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX idx_lab_results_patient ON lab_results(patient_id);
CREATE INDEX idx_lab_results_test ON lab_results(test_name);
CREATE INDEX idx_lab_results_status ON lab_results(status);
-- ============================================================================
-- AUDIT LOG (HIPAA Compliance)
-- ============================================================================
CREATE TABLE audit_log (
    audit_id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    -- Who
    user_id UUID,
    user_role VARCHAR(50),
    -- What
    action VARCHAR(50) NOT NULL,
    -- CREATE, READ, UPDATE, DELETE
    table_name VARCHAR(100) NOT NULL,
    record_id UUID,
    -- When
    timestamp TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    -- Where
    ip_address VARCHAR(45),
    -- Details
    changes JSONB,
    -- Patient Context
    patient_id UUID REFERENCES patients(patient_id)
);
CREATE INDEX idx_audit_log_user ON audit_log(user_id);
CREATE INDEX idx_audit_log_timestamp ON audit_log(timestamp);
CREATE INDEX idx_audit_log_patient ON audit_log(patient_id);
CREATE INDEX idx_audit_log_action ON audit_log(action);
-- ============================================================================
-- TRIGGERS
-- ============================================================================
CREATE OR REPLACE FUNCTION update_updated_at_column() RETURNS TRIGGER AS $$ BEGIN NEW.updated_at = CURRENT_TIMESTAMP;
RETURN NEW;
END;
$$ LANGUAGE plpgsql;
CREATE TRIGGER update_patients_updated_at BEFORE
UPDATE ON patients FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
CREATE TRIGGER update_appointments_updated_at BEFORE
UPDATE ON appointments FOR EACH ROW EXECUTE FUNCTION update_updated_at_column();
-- ============================================================================
-- VIEWS
-- ============================================================================
-- Patient summary
CREATE VIEW v_patient_summary AS
SELECT p.patient_id,
    p.mrn,
    p.first_name,
    p.last_name,
    p.date_of_birth,
    EXTRACT(
        YEAR
        FROM AGE(p.date_of_birth)
    ) AS age,
    p.gender,
    p.readmission_risk_score,
    COUNT(DISTINCT a.appointment_id) AS total_appointments,
    COUNT(DISTINCT d.diagnosis_id) AS active_diagnoses,
    COUNT(DISTINCT pr.prescription_id) AS active_prescriptions
FROM patients p
    LEFT JOIN appointments a ON p.patient_id = a.patient_id
    LEFT JOIN diagnoses d ON p.patient_id = d.patient_id
    AND d.status = 'ACTIVE'
    LEFT JOIN prescriptions pr ON p.patient_id = pr.patient_id
    AND pr.status = 'ACTIVE'
WHERE p.status = 'ACTIVE'
GROUP BY p.patient_id,
    p.mrn,
    p.first_name,
    p.last_name,
    p.date_of_birth,
    p.gender,
    p.readmission_risk_score;
-- ============================================================================
-- COMMENTS
-- ============================================================================
COMMENT ON TABLE patients IS 'Patient demographic and contact information (PHI)';
COMMENT ON TABLE appointments IS 'Patient appointments and visits';
COMMENT ON TABLE diagnoses IS 'Patient diagnoses with ICD-10 codes';
COMMENT ON TABLE prescriptions IS 'Medication prescriptions with drug interaction checking';
COMMENT ON TABLE vital_signs IS 'Patient vital signs measurements';
COMMENT ON TABLE lab_results IS 'Laboratory test results';
COMMENT ON TABLE audit_log IS 'HIPAA-compliant audit trail for all data access';