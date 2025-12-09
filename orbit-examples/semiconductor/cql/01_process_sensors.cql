-- Semiconductor FDC / SPC Data (CQL)
-- Fault Detection & Classification: High-res tool sensor data.

CREATE KEYSPACE IF NOT EXISTS semiconductor_fdc
WITH replication = {'class': 'SimpleStrategy', 'replication_factor': 1};

USE semiconductor_fdc;

-- 1. Recipe Trace Data
-- Storing 10Hz data for RF Power, Gas Flow, Pressure during a process run.
CREATE TABLE IF NOT EXISTS tool_traces (
    tool_id text,
    run_id text, -- Unique ID for the process run (e.g. "RUN_L1000_STEP5")
    ts timestamp,
    chamber_id text,
    rf_power double,
    gas_flow_sccm double,
    pressure_torr double,
    end_point_signal double,
    PRIMARY KEY ((tool_id, run_id), ts)
) WITH CLUSTERING ORDER BY (ts ASC);

-- 2. Metrology Data (Measurements)
-- Thickness, Critical Dimension (CD) measurements after processing.
CREATE TABLE IF NOT EXISTS metrology (
    lot_id text,
    wafer_id text,
    step_id text,
    measure_id text,
    value double,
    lcl double, -- Lower Control Limit
    ucl double, -- Upper Control Limit
    ts timestamp,
    PRIMARY KEY (lot_id, wafer_id, step_id, measure_id)
);

-- Sample Query: Retrieve trace for Fault Analysis
-- SELECT * FROM tool_traces WHERE tool_id='ETCH-LAM-01' AND run_id='RUN_5512' LIMIT 1000;
