WITH features AS (
  SELECT i.user_id,
         AVG(i.watch_time_min) AS avg_watch,
         SUM(CASE WHEN i.liked THEN 1 ELSE 0 END) AS likes,
         MAX(CASE WHEN i.liked THEN 1 ELSE 0 END) AS label
  FROM interactions i
  GROUP BY i.user_id
)
SELECT ML_TRAIN_MODEL('engagement_rf','random_forest',
       ARRAY[avg_watch, likes], label)
FROM features;
WITH predict AS (
  SELECT 1 AS user_id, 35 AS avg_watch, 1 AS likes
)
SELECT user_id,
       ML_PREDICT('engagement_rf', ARRAY[avg_watch, likes]) AS like_probability
FROM predict;
