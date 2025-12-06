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

WITH tf_fe AS (
    SELECT ts,
           vehicles_per_minute,
           speed_avg,
           LAG(speed_avg,1) OVER (ORDER BY ts) AS speed_lag1,
           AVG(speed_avg) OVER (ORDER BY ts ROWS BETWEEN 5 PRECEDING AND CURRENT ROW) AS speed_ma6
    FROM traffic_flow
    WHERE sensor_id = 1
), tf_train AS (
    SELECT speed_lag1, speed_ma6, vehicles_per_minute, speed_avg AS target
    FROM tf_fe
    WHERE speed_lag1 IS NOT NULL
)
SELECT ML_TRAIN_MODEL('smart_city_speed_forecast_gb','gradient_boosting', ARRAY[speed_lag1, speed_ma6, vehicles_per_minute], target)
FROM tf_train;

WITH tf_eval AS (
    SELECT speed_lag1, speed_ma6, vehicles_per_minute, speed_avg AS target
    FROM tf_fe
    WHERE speed_lag1 IS NOT NULL
)
SELECT ML_EVALUATE_MODEL('smart_city_speed_forecast_gb', ARRAY[speed_lag1, speed_ma6, vehicles_per_minute], target)
FROM tf_eval;

WITH tf_recent AS (
    SELECT ts,
           vehicles_per_minute,
           speed_avg,
           LAG(speed_avg,1) OVER (ORDER BY ts) AS speed_lag1,
           AVG(speed_avg) OVER (ORDER BY ts ROWS BETWEEN 5 PRECEDING AND CURRENT ROW) AS speed_ma6
    FROM traffic_flow
    WHERE sensor_id = 1
)
SELECT ts,
       speed_avg AS actual,
       ML_PREDICT('smart_city_speed_forecast_gb', ARRAY[speed_lag1, speed_ma6, vehicles_per_minute]) AS predicted,
       speed_avg - ML_PREDICT('smart_city_speed_forecast_gb', ARRAY[speed_lag1, speed_ma6, vehicles_per_minute]) AS residual,
       CASE WHEN vehicles_per_minute > 50 AND predicted < 15 THEN 'FORECAST_CONGESTION' ELSE 'NORMAL' END AS forecast_state
FROM tf_recent
ORDER BY ts DESC
LIMIT 60;

WITH aq_fe AS (
    SELECT ts,
           pm25,
           LAG(pm25,1) OVER (ORDER BY ts) AS pm25_lag1,
           AVG(pm25) OVER (ORDER BY ts ROWS BETWEEN 29 PRECEDING AND CURRENT ROW) AS pm25_ma30
    FROM air_quality_readings
    WHERE station_id = 1
), aq_train AS (
    SELECT pm25_lag1, pm25_ma30, pm25 AS target
    FROM aq_fe
    WHERE pm25_lag1 IS NOT NULL
)
SELECT ML_TRAIN_MODEL('smart_city_pm25_forecast_rf','random_forest', ARRAY[pm25_lag1, pm25_ma30], target)
FROM aq_train;

WITH aq_recent AS (
    SELECT ts,
           pm25,
           LAG(pm25,1) OVER (ORDER BY ts) AS pm25_lag1,
           AVG(pm25) OVER (ORDER BY ts ROWS BETWEEN 29 PRECEDING AND CURRENT ROW) AS pm25_ma30
    FROM air_quality_readings
    WHERE station_id = 1
)
SELECT ts,
       pm25 AS actual,
       ML_PREDICT('smart_city_pm25_forecast_rf', ARRAY[pm25_lag1, pm25_ma30]) AS predicted,
       pm25 - ML_PREDICT('smart_city_pm25_forecast_rf', ARRAY[pm25_lag1, pm25_ma30]) AS residual,
       CASE WHEN predicted > 35 THEN 'FORECAST_AQI_POOR' ELSE 'FORECAST_AQI_OK' END AS forecast_air_quality
FROM aq_recent
ORDER BY ts DESC
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
