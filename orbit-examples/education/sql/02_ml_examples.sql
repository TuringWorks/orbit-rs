WITH features AS (
  SELECT e.student_id,
         AVG(e.attendance_rate) AS attendance,
         AVG(e.assignment_score) AS score,
         AVG(c.difficulty) AS difficulty,
         MAX(CASE WHEN e.dropped THEN 1 ELSE 0 END) AS label
  FROM enrollments e
  JOIN courses c ON c.course_id = e.course_id
  GROUP BY e.student_id
)
SELECT ML_TRAIN_MODEL('dropout_rf','random_forest',
       ARRAY[attendance, score, difficulty], label)
FROM features;
WITH predict AS (
  SELECT 1 AS student_id, 0.75 AS attendance, 70.0 AS score, 5 AS difficulty
)
SELECT student_id,
       ML_PREDICT('dropout_rf', ARRAY[attendance, score, difficulty]) AS dropout_prob
FROM predict;
