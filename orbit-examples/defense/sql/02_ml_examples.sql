WITH features AS (
  SELECT m.mission_id,
         COALESCE(m.duration_hours,0) AS duration,
         COUNT(i.incident_id) AS incident_count,
         AVG(i.severity) AS avg_severity,
         m.risk_level AS label
  FROM missions m
  LEFT JOIN incidents i ON TRUE
  GROUP BY m.mission_id, m.duration_hours, m.risk_level
)
SELECT ML_TRAIN_MODEL('mission_risk_rf','random_forest',
       ARRAY[duration, incident_count, COALESCE(avg_severity,0)], label)
FROM features;
WITH predict AS (
  SELECT 1 AS mission_id, 18 AS duration, 2 AS incident_count, 1.5 AS avg_severity
)
SELECT mission_id,
       ML_PREDICT('mission_risk_rf', ARRAY[duration, incident_count, avg_severity]) AS risk_score
FROM predict;
