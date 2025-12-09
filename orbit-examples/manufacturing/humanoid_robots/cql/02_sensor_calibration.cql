-- ============================================================================
-- OrbitRS Humanoid Robot Manufacturing - Calibration Data (CQL)
-- ============================================================================
-- Storing terabytes of calibration test data for joint actuators
-- ============================================================================

CREATE KEYSPACE IF NOT EXISTS robot_calibration 
WITH replication = {'class': 'SimpleStrategy', 'replication_factor': 1};

USE robot_calibration;

-- During 'burn-in' or calibration, joints are moved through full range of motion.
-- Sensors record position, detailed torque, and temperature at 1000Hz.
CREATE TABLE joint_calibration_logs (
    robot_serial text,
    joint_id text, -- 'R_KNEE', 'L_ELBOW'
    test_run_id uuid,
    timestamp timestamp,
    position_rad float,
    torque_nm float,
    current_amps float,
    temp_c float,
    PRIMARY KEY ((robot_serial, joint_id), test_run_id, timestamp)
) WITH CLUSTERING ORDER BY (test_run_id ASC, timestamp ASC);

-- Insert high-speed sample
INSERT INTO joint_calibration_logs (
    robot_serial, joint_id, test_run_id, timestamp, 
    position_rad, torque_nm, current_amps, temp_c
) VALUES (
    'RBT-9000-001', 'R_KNEE', uuid(), toTimestamp(now()), 
    1.57, 120.5, 4.2, 55.1
);
