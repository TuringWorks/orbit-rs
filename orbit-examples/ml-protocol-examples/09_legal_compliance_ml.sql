CREATE EXTENSION IF NOT EXISTS vector;

CREATE TABLE IF NOT EXISTS legal_documents (
    doc_id SERIAL PRIMARY KEY,
    title TEXT,
    content TEXT,
    doc_type VARCHAR(50),
    risk_label VARCHAR(20),
    embedding vector(384),
    risk_score FLOAT
);

INSERT INTO legal_documents (title, content, doc_type, risk_label, embedding)
VALUES
    ('Contract A', 'Payment terms include late fees and arbitration clause.', 'contract', 'LOW', ML_EMBED_TEXT('Payment terms include late fees and arbitration clause.', 'sentence-transformers')),
    ('Contract B', 'Indemnification and liability caps are missing.', 'contract', 'HIGH', ML_EMBED_TEXT('Indemnification and liability caps are missing.', 'sentence-transformers')),
    ('Policy X', 'Privacy policy with data retention of 10 years.', 'policy', 'MEDIUM', ML_EMBED_TEXT('Privacy policy with data retention of 10 years.', 'sentence-transformers'));

SELECT ML_TRAIN_MODEL(
    'legal_risk_classifier',
    'xgboost',
    ARRAY[
        embedding
    ],
    CASE risk_label WHEN 'HIGH' THEN 1 WHEN 'MEDIUM' THEN 0 ELSE 0 END
) FROM legal_documents;

SELECT ML_EVALUATE_MODEL(
    'legal_risk_classifier',
    ARRAY[
        embedding
    ],
    CASE risk_label WHEN 'HIGH' THEN 1 WHEN 'MEDIUM' THEN 0 ELSE 0 END
) FROM legal_documents;

UPDATE legal_documents
SET risk_score = ML_PREDICT(
    'legal_risk_classifier',
    ARRAY[
        embedding
    ]
);

SELECT title, content
FROM legal_documents
ORDER BY embedding <=> ML_EMBED_TEXT('missing indemnification and liability clauses', 'sentence-transformers')
LIMIT 3;
