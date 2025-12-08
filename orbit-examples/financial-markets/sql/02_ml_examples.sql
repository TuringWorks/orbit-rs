WITH features AS (
  SELECT s.symbol_id,
         COALESCE(i.volatility,0) AS vol,
         COALESCE(i.momentum,0) AS mom,
         COALESCE(i.rsi,50.0) AS rsi,
         CASE WHEN i.momentum > 0.08 AND i.rsi < 70 THEN 1 ELSE 0 END AS label
  FROM symbols s
  LEFT JOIN indicators i ON i.symbol_id = s.symbol_id
)
SELECT ML_TRAIN_MODEL('alpha_gb','gradient_boosting',
       ARRAY[vol, mom, rsi], label)
FROM features;
WITH predict AS (
  SELECT 1 AS symbol_id, 0.20 AS vol, 0.09 AS mom, 60.0 AS rsi
)
SELECT symbol_id,
       ML_PREDICT('alpha_gb', ARRAY[vol, mom, rsi]) AS buy_probability
FROM predict;
