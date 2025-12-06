CREATE TABLE IF NOT EXISTS ts_entities (
    entity_id SERIAL PRIMARY KEY,
    entity_name VARCHAR(100)
);
INSERT INTO ts_entities (entity_name) VALUES ('Turbine-A') ON CONFLICT DO NOTHING;

CREATE TABLE IF NOT EXISTS ts_metrics (
    entity_id INTEGER REFERENCES ts_entities(entity_id),
    ts TIMESTAMP NOT NULL,
    metric DOUBLE PRECISION NOT NULL,
    tag VARCHAR(50)
);
INSERT INTO ts_metrics (entity_id, ts, metric, tag)
SELECT 1,
       NOW() - (INTERVAL '1 minute' * n),
       50 + (RANDOM() * 10),
       'vibration'
FROM GENERATE_SERIES(0, 180) AS n;

SELECT ts,
       metric,
       AVG(metric) OVER (
           ORDER BY ts
           ROWS BETWEEN 5 PRECEDING AND CURRENT ROW
       ) AS ma_6min,
       STDDEV(metric) OVER (
           ORDER BY ts
           ROWS BETWEEN 5 PRECEDING AND CURRENT ROW
       ) AS std_6min,
       CASE WHEN metric > AVG(metric) OVER (
                    ORDER BY ts ROWS BETWEEN 5 PRECEDING AND CURRENT ROW
                ) + 3 * STDDEV(metric) OVER (
                    ORDER BY ts ROWS BETWEEN 5 PRECEDING AND CURRENT ROW
                ) THEN 'ANOMALY' ELSE 'NORMAL' END AS status
FROM ts_metrics
WHERE entity_id = 1 AND tag = 'vibration'
ORDER BY ts DESC
LIMIT 60;

WITH deltas AS (
    SELECT ts,
           metric,
           metric - LAG(metric) OVER (ORDER BY ts) AS delta
    FROM ts_metrics
    WHERE entity_id = 1 AND tag = 'vibration'
), trend AS (
    SELECT ts,
           metric,
           AVG(delta) OVER (
               ORDER BY ts ROWS BETWEEN 29 PRECEDING AND CURRENT ROW
           ) AS slope_30min
    FROM deltas
)
SELECT ts, metric, slope_30min,
       CASE WHEN slope_30min > 0.2 THEN 'UPTREND'
            WHEN slope_30min < -0.2 THEN 'DOWNTREND'
            ELSE 'STABLE' END AS trend
FROM trend
ORDER BY ts DESC
LIMIT 60;

CREATE TABLE IF NOT EXISTS ts_metrics_secondary (
    entity_id INTEGER REFERENCES ts_entities(entity_id),
    ts TIMESTAMP NOT NULL,
    metric DOUBLE PRECISION NOT NULL,
    tag VARCHAR(50)
);
INSERT INTO ts_metrics_secondary (entity_id, ts, metric, tag)
SELECT 1,
       NOW() - (INTERVAL '1 minute' * n),
       70 + (RANDOM() * 5),
       'temperature'
FROM GENERATE_SERIES(0, 180) AS n;

WITH aligned AS (
    SELECT a.ts,
           a.metric AS vib,
           b.metric AS temp
    FROM ts_metrics a
    JOIN ts_metrics_secondary b
      ON a.entity_id = b.entity_id AND a.ts = b.ts
    WHERE a.entity_id = 1 AND a.tag = 'vibration' AND b.tag = 'temperature'
), corr_win AS (
    SELECT ts, vib, temp,
           AVG(vib) OVER (ORDER BY ts ROWS BETWEEN 29 PRECEDING AND CURRENT ROW) AS avg_vib,
           AVG(temp) OVER (ORDER BY ts ROWS BETWEEN 29 PRECEDING AND CURRENT ROW) AS avg_temp
    FROM aligned
), cov_calc AS (
    SELECT ts,
           vib,
           temp,
           (vib - avg_vib) * (temp - avg_temp) AS prod_dev
    FROM corr_win
)
SELECT ts,
       AVG(prod_dev) OVER (ORDER BY ts ROWS BETWEEN 29 PRECEDING AND CURRENT ROW) AS rolling_cov
FROM cov_calc
ORDER BY ts DESC
LIMIT 60;
