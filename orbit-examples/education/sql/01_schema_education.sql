CREATE TABLE IF NOT EXISTS students (
  student_id SERIAL PRIMARY KEY,
  name VARCHAR(200) NOT NULL,
  age INT,
  grade_level INT
);
CREATE TABLE IF NOT EXISTS courses (
  course_id SERIAL PRIMARY KEY,
  name VARCHAR(200) NOT NULL,
  difficulty INT
);
CREATE TABLE IF NOT EXISTS enrollments (
  enrollment_id SERIAL PRIMARY KEY,
  student_id INT REFERENCES students(student_id),
  course_id INT REFERENCES courses(course_id),
  attendance_rate DECIMAL(4,2),
  assignment_score DECIMAL(5,2),
  dropped BOOLEAN DEFAULT FALSE
);
INSERT INTO students(name, age, grade_level) VALUES
('Alice', 16, 10),
('Bob', 17, 11) ON CONFLICT DO NOTHING;
INSERT INTO courses(name, difficulty) VALUES
('Mathematics', 4),
('Physics', 5) ON CONFLICT DO NOTHING;
INSERT INTO enrollments(student_id, course_id, attendance_rate, assignment_score, dropped) VALUES
(1, 1, 0.92, 88.0, FALSE),
(1, 2, 0.70, 65.0, TRUE),
(2, 1, 0.85, 75.0, FALSE) ON CONFLICT DO NOTHING;
