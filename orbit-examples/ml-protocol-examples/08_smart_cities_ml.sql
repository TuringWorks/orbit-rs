CREATE TABLE IF NOT EXISTS traffic_sensors (
    sensor_id SERIAL PRIMARY KEY,
    location_name VARCHAR(100),
    latitude DOUBLE PRECISION,
    longitude DOUBLE PRECISION
);
INSERT INTO traffic_sensors (location_name, latitude, longitude)
VALUES ('Downtown-1', 37.7749, -122.4194) ON CONFLICT DO NOTHING;

CREATE TABLE IF NOT EXISTS traffic_flow (
    sensor_id INTEGER REFERENCES traffic_sensors(sensor_id),
    ts TIMESTAMP NOT NULL,
    vehicles_per_minute INTEGER,
    speed_avg DOUBLE PRECISION
);
INSERT INTO traffic_flow (sensor_id, ts, vehicles_per_minute, speed_avg)
SELECT 1,
       NOW() - (INTERVAL '1 minute' * n),
       (RANDOM() * 60)::INTEGER,
       20 + (RANDOM() * 30)
FROM GENERATE_SERIES(0, 240) AS n;

CREATE TABLE IF NOT EXISTS air_quality (
    station_id SERIAL PRIMARY KEY,
    name VARCHAR(100),
    latitude DOUBLE PRECISION,
    longitude DOUBLE PRECISION
);
INSERT INTO air_quality (name, latitude, longitude)
VALUES ('AQ-Station-1', 37.7750, -122.4183) ON CONFLICT DO NOTHING;

CREATE TABLE IF NOT EXISTS air_quality_readings (
    station_id INTEGER REFERENCES air_quality(station_id),
    ts TIMESTAMP NOT NULL,
    pm25 DOUBLE PRECISION,
    no2 DOUBLE PRECISION,
    o3 DOUBLE PRECISION
);
INSERT INTO air_quality_readings (station_id, ts, pm25, no2, o3)
SELECT 1,
       NOW() - (INTERVAL '1 minute' * n),
       10 + (RANDOM() * 40),
       5 + (RANDOM() * 30),
       10 + (RANDOM() * 20)
FROM GENERATE_SERIES(0, 240) AS n;

CREATE TABLE IF NOT EXISTS building_energy (
    building_id SERIAL PRIMARY KEY,
    name VARCHAR(100)
);
INSERT INTO building_energy (name) VALUES ('CityHall') ON CONFLICT DO NOTHING;

CREATE TABLE IF NOT EXISTS building_energy_usage (
    building_id INTEGER REFERENCES building_energy(building_id),
    ts TIMESTAMP NOT NULL,
    kwh DOUBLE PRECISION
);
INSERT INTO building_energy_usage (building_id, ts, kwh)
SELECT 1,
       NOW() - (INTERVAL '15 minutes' * n),
       100 + (RANDOM() * 50)
FROM GENERATE_SERIES(0, 192) AS n;

SELECT tf.ts,
       tf.vehicles_per_minute,
        tf.speed_avg,
       AVG(tf.speed_avg) OVER (
           ORDER BY tf.ts ROWS BETWEEN 29 PRECEDING AND CURRENT ROW
       ) AS speed_ma_30min,
       CASE WHEN tf.speed_avg < 15 AND tf.vehicles_per_minute > 45 THEN 'CONGESTION'
            WHEN tf.speed_avg < AVG(tf.speed_avg) OVER (
                    ORDER BY tf.ts ROWS BETWEEN 29 PRECEDING AND CURRENT ROW
                 ) * 0.7 THEN 'LIKELY_CONGESTION'
            ELSE 'NORMAL' END AS traffic_state
FROM traffic_flow tf
WHERE tf.sensor_id = 1
ORDER BY tf.ts DESC
LIMIT 60;

SELECT aq.ts,
       aq.pm25,
       aq.no2,
       aq.o3,
       AVG(aq.pm25) OVER (
           ORDER BY aq.ts ROWS BETWEEN 59 PRECEDING AND CURRENT ROW
       ) AS pm25_ma_60min,
       CASE WHEN aq.pm25 > 35 OR aq.no2 > 40 OR aq.o3 > 60 THEN 'POOR'
            WHEN aq.pm25_ma_60min > 30 THEN 'DETERIORATING'
            ELSE 'GOOD' END AS air_quality_state
FROM air_quality_readings aq
WHERE aq.station_id = 1
ORDER BY aq.ts DESC
LIMIT 60;

WITH aligned AS (
    SELECT tf.ts,
           tf.vehicles_per_minute,
           tf.speed_avg,
           aq.pm25
    FROM traffic_flow tf
    JOIN air_quality_readings aq
      ON aq.ts = tf.ts
    WHERE tf.sensor_id = 1 AND aq.station_id = 1
)
SELECT ts,
       vehicles_per_minute,
       speed_avg,
       pm25,
       CASE WHEN vehicles_per_minute > 50 AND speed_avg < 15 AND pm25 > 30 THEN 'TRAFFIC_AQI_ALERT'
            ELSE 'NORMAL' END AS alert_state
FROM aligned
ORDER BY ts DESC
LIMIT 60;
