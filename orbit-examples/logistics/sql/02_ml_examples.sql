WITH features AS (
  SELECT s.shipment_id,
         s.weight_kg,
         s.distance_km,
         COALESCE(s.delay_hours,0) AS delay,
         CASE WHEN s.delivered AND COALESCE(s.delay_hours,0) <= 4 THEN 1 ELSE 0 END AS label
  FROM shipments s
)
SELECT ML_TRAIN_MODEL('delivery_rf','random_forest',
       ARRAY[weight_kg, distance_km, delay], label)
FROM features;
WITH predict AS (
  SELECT 1001 AS shipment_id, 450.0 AS weight_kg, 200.0 AS distance_km, 3 AS delay
)
SELECT shipment_id,
       ML_PREDICT('delivery_rf', ARRAY[weight_kg, distance_km, delay]) AS on_time_probability
FROM predict;
