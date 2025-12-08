WITH features AS (
  SELECT f.farm_id,
         AVG(sr.moisture) AS avg_moisture,
         AVG(sr.temperature) AS avg_temperature,
         SUM(f.area_ha) AS area,
         AVG(f.soil_ph) AS soil_ph,
         MAX(y.total_yield_tons) AS label
  FROM fields f
  JOIN sensor_readings sr ON sr.farm_id = f.farm_id
  JOIN yields y ON y.farm_id = f.farm_id
  GROUP BY f.farm_id
)
SELECT ML_TRAIN_MODEL('agri_yield_rf','random_forest',
       ARRAY[avg_moisture, avg_temperature, area, soil_ph], label)
FROM features;
WITH predict_features AS (
  SELECT 1 AS farm_id,
         23.0 AS avg_moisture,
         19.5 AS avg_temperature,
         200.5 AS area,
         6.5 AS soil_ph
  UNION ALL
  SELECT 2, 19.0, 17.5, 180.0, 6.7
)
SELECT farm_id,
       ML_PREDICT('agri_yield_rf', ARRAY[avg_moisture, avg_temperature, area, soil_ph]) AS predicted_yield
FROM predict_features;
