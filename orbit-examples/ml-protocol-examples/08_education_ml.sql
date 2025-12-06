CREATE TABLE IF NOT EXISTS students (
    student_id SERIAL PRIMARY KEY,
    name VARCHAR(100),
    attendance_pct FLOAT,
    assignments_completed INTEGER,
    quizzes_avg FLOAT,
    projects_score FLOAT,
    final_grade FLOAT,
    at_risk BOOLEAN DEFAULT FALSE,
    risk_score FLOAT
);

INSERT INTO students (name, attendance_pct, assignments_completed, quizzes_avg, projects_score, final_grade)
VALUES
    ('Alex', 0.92, 18, 85.0, 88.0, 86.0),
    ('Sam', 0.65, 10, 60.0, 70.0, 62.0),
    ('Riley', 0.80, 15, 78.0, 82.0, 79.0);

UPDATE students
SET at_risk = (attendance_pct < 0.75) OR (final_grade < 70.0);

SELECT ML_TRAIN_MODEL(
    'student_risk_lr',
    'logistic_regression',
    ARRAY[
        attendance_pct,
        assignments_completed,
        quizzes_avg,
        projects_score
    ],
    at_risk
) FROM students;

SELECT ML_EVALUATE_MODEL(
    'student_risk_lr',
    ARRAY[
        attendance_pct,
        assignments_completed,
        quizzes_avg,
        projects_score
    ],
    at_risk
) FROM students;

UPDATE students
SET risk_score = ML_PREDICT(
    'student_risk_lr',
    ARRAY[
        attendance_pct,
        assignments_completed,
        quizzes_avg,
        projects_score
    ]
);
