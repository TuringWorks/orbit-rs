CREATE TABLE IF NOT EXISTS smart_meters (
    meter_id SERIAL PRIMARY KEY,
    customer_id INTEGER,
    timestamp BIGINT NOT NULL,
    voltage FLOAT,
    current FLOAT,
    power_kw FLOAT,
    outage BOOLEAN DEFAULT FALSE,
    outage_risk FLOAT
);

INSERT INTO smart_meters (customer_id, timestamp, voltage, current, power_kw, outage)
VALUES
    (101, extract(epoch from now())::bigint * 1000, 230.0, 5.0, 1.15, false),
    (102, extract(epoch from now())::bigint * 1000, 220.0, 12.0, 2.64, false),
    (103, extract(epoch from now())::bigint * 1000, 190.0, 18.0, 3.42, true);

SELECT ML_TRAIN_MODEL(
    'grid_outage_rf',
    'random_forest',
    ARRAY[
        voltage,
        current,
        power_kw
    ],
    outage
) FROM smart_meters;

SELECT ML_EVALUATE_MODEL(
    'grid_outage_rf',
    ARRAY[
        voltage,
        current,
        power_kw
    ],
    outage
) FROM smart_meters;

UPDATE smart_meters
SET outage_risk = ML_PREDICT(
    'grid_outage_rf',
    ARRAY[
        voltage,
        current,
        power_kw
    ]
);
