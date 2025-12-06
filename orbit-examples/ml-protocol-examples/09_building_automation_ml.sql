CREATE TABLE IF NOT EXISTS buildings (
    building_id SERIAL PRIMARY KEY,
    name VARCHAR(100)
);
INSERT INTO buildings (name) VALUES ('HQ-1') ON CONFLICT DO NOTHING;

CREATE TABLE IF NOT EXISTS building_sensors (
    building_id INTEGER REFERENCES buildings(building_id),
    ts TIMESTAMP NOT NULL,
    temp_c DOUBLE PRECISION,
    humidity DOUBLE PRECISION,
    occupancy INTEGER,
    co2_ppm DOUBLE PRECISION
);
INSERT INTO building_sensors (building_id, ts, temp_c, humidity, occupancy, co2_ppm)
SELECT 1,
       NOW() - (INTERVAL '5 minutes' * n),
       20 + (RANDOM() * 6),
       30 + (RANDOM() * 30),
       (RANDOM() * 50)::INTEGER,
       400 + (RANDOM() * 800)
FROM GENERATE_SERIES(0, 288) AS n;

CREATE TABLE IF NOT EXISTS hvac_events (
    building_id INTEGER REFERENCES buildings(building_id),
    ts TIMESTAMP NOT NULL,
    mode VARCHAR(20),
    setpoint DOUBLE PRECISION
);
INSERT INTO hvac_events (building_id, ts, mode, setpoint)
SELECT 1,
       NOW() - (INTERVAL '30 minutes' * n),
       CASE WHEN (n % 2) = 0 THEN 'cool' ELSE 'heat' END,
       22
FROM GENERATE_SERIES(0, 48) AS n;

CREATE TABLE IF NOT EXISTS energy_usage (
    building_id INTEGER REFERENCES buildings(building_id),
    ts TIMESTAMP NOT NULL,
    kwh DOUBLE PRECISION
);
INSERT INTO energy_usage (building_id, ts, kwh)
SELECT 1,
       NOW() - (INTERVAL '15 minutes' * n),
       80 + (RANDOM() * 40)
FROM GENERATE_SERIES(0, 192) AS n;

SELECT bs.ts,
       bs.temp_c,
       bs.humidity,
       bs.occupancy,
       bs.co2_ppm,
       AVG(bs.temp_c) OVER (
           ORDER BY bs.ts ROWS BETWEEN 5 PRECEDING AND CURRENT ROW
       ) AS temp_ma,
       CASE WHEN bs.co2_ppm > 1000 AND bs.occupancy > 30 THEN 'VENTILATE'
            WHEN bs.temp_c > 26 AND bs.occupancy > 20 THEN 'COOL'
            WHEN bs.temp_c < 18 AND bs.occupancy > 20 THEN 'HEAT'
            ELSE 'OK' END AS action
FROM building_sensors bs
WHERE bs.building_id = 1
ORDER BY bs.ts DESC
LIMIT 60;

WITH daily AS (
    SELECT DATE_TRUNC('day', ts) AS day,
           AVG(temp_c) AS avg_temp,
           AVG(humidity) AS avg_humidity,
           AVG(occupancy) AS avg_occupancy,
           AVG(co2_ppm) AS avg_co2
    FROM building_sensors
    WHERE building_id = 1
    GROUP BY 1
)
SELECT day,
       avg_temp,
       avg_humidity,
       avg_occupancy,
       avg_co2,
       CASE WHEN avg_co2 > 900 THEN 'AIR_QUALITY_ATTENTION'
            WHEN avg_occupancy > 35 AND avg_temp > 24 THEN 'SCHEDULE_COOLING'
            ELSE 'NORMAL' END AS daily_recommendation
FROM daily
ORDER BY day DESC
LIMIT 7;

WITH joined AS (
    SELECT e.ts,
           e.kwh,
           s.occupancy,
           s.temp_c
    FROM energy_usage e
    JOIN building_sensors s
      ON s.ts = e.ts
    WHERE e.building_id = 1 AND s.building_id = 1
)
SELECT ts,
       kwh,
       occupancy,
       temp_c,
       AVG(kwh) OVER (
           ORDER BY ts ROWS BETWEEN 11 PRECEDING AND CURRENT ROW
       ) AS kwh_ma_3h,
       CASE WHEN occupancy = 0 AND kwh_ma_3h > 90 THEN 'BASELOAD_HIGH'
            ELSE 'BASELOAD_NORMAL' END AS baseload_state
FROM joined
ORDER BY ts DESC
LIMIT 60;
