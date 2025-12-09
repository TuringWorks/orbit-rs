WITH features AS (
  SELECT g.node_id,
         AVG(m.voltage) AS avg_voltage,
         AVG(m.load_kw) AS avg_load,
         COUNT(o.outage_id) AS outage_count,
         COALESCE(AVG(o.duration_min),0) AS avg_outage_duration,
         COUNT(o.outage_id) AS label
  FROM grid_nodes g
  LEFT JOIN meter_readings m ON m.node_id = g.node_id
  LEFT JOIN outages o ON o.node_id = g.node_id
  GROUP BY g.node_id
)
SELECT ML_TRAIN_MODEL('outage_rf','random_forest',
       ARRAY[avg_voltage, avg_load, avg_outage_duration], label)
FROM features;
WITH predict AS (
  SELECT 1 AS node_id, 229.0 AS avg_voltage, 1400.0 AS avg_load, 20.0 AS avg_outage_duration
)
SELECT node_id,
       ML_PREDICT('outage_rf', ARRAY[avg_voltage, avg_load, avg_outage_duration]) AS outage_risk
FROM predict;
