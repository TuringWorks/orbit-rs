WITH features AS (
  SELECT s.sat_id,
         AVG(t.temp_c) AS avg_temp,
         AVG(t.power_w) AS avg_power,
         AVG(t.vibration) AS avg_vibration,
         CASE WHEN AVG(t.vibration) > 0.015 THEN 1 ELSE 0 END AS label
  FROM satellites s
  LEFT JOIN telemetry t ON t.sat_id = s.sat_id
  GROUP BY s.sat_id
)
SELECT ML_TRAIN_MODEL('sat_anomaly_rf','random_forest',
       ARRAY[avg_temp, avg_power, avg_vibration], label)
FROM features;
WITH predict AS (
  SELECT 1 AS sat_id, 23.5 AS avg_temp, 119.0 AS avg_power, 0.018 AS avg_vibration
)
SELECT sat_id,
       ML_PREDICT('sat_anomaly_rf', ARRAY[avg_temp, avg_power, avg_vibration]) AS anomaly_probability
FROM predict;
