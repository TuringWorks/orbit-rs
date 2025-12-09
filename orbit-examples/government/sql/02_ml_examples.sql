WITH features AS (
  SELECT a.applicant_id,
         a.age,
         a.income,
         AVG(CASE WHEN p.approved THEN 1 ELSE 0 END) AS label
  FROM applicants a
  LEFT JOIN permits p ON p.applicant_id = a.applicant_id
  GROUP BY a.applicant_id, a.age, a.income
)
SELECT ML_TRAIN_MODEL('permit_approval_lr','gradient_boosting',
       ARRAY[age, income], label)
FROM features;
WITH predict AS (
  SELECT 1 AS applicant_id, 30 AS age, 65000.0 AS income
)
SELECT applicant_id,
       ML_PREDICT('permit_approval_lr', ARRAY[age, income]) AS approval_probability
FROM predict;
