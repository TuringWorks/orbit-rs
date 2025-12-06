CREATE EXTENSION IF NOT EXISTS vector;

CREATE TABLE IF NOT EXISTS kg_entities (
    entity_id SERIAL PRIMARY KEY,
    name TEXT,
    type TEXT,
    description TEXT,
    embedding vector(384),
    degree INTEGER DEFAULT 0
);

CREATE TABLE IF NOT EXISTS kg_relations (
    relation_id SERIAL PRIMARY KEY,
    name TEXT,
    embedding vector(384)
);

CREATE TABLE IF NOT EXISTS kg_triples (
    head_id INTEGER REFERENCES kg_entities(entity_id),
    relation_id INTEGER REFERENCES kg_relations(relation_id),
    tail_id INTEGER REFERENCES kg_entities(entity_id),
    label BOOLEAN DEFAULT TRUE
);

INSERT INTO kg_entities (name, type, description)
VALUES
    ('Alice', 'Person', 'Software engineer at Acme Corp'),
    ('Bob', 'Person', 'Network specialist at CityNet ISP'),
    ('Acme Corp', 'Organization', 'Technology company with global offices'),
    ('CityNet ISP', 'Organization', 'Internet service provider in urban areas'),
    ('Metropolis', 'Location', 'City with strong tech ecosystem')
ON CONFLICT DO NOTHING;

INSERT INTO kg_relations (name)
VALUES
    ('works_at'),
    ('located_in'),
    ('partner_of')
ON CONFLICT DO NOTHING;

UPDATE kg_entities
SET embedding = ML_EMBED_TEXT(description, 'sentence-transformers')
WHERE embedding IS NULL;

UPDATE kg_relations
SET embedding = ML_EMBED_TEXT(name, 'sentence-transformers')
WHERE embedding IS NULL;

INSERT INTO kg_triples (head_id, relation_id, tail_id)
SELECT e1.entity_id, r.relation_id, e2.entity_id
FROM kg_entities e1, kg_entities e2, kg_relations r
WHERE (e1.name = 'Alice' AND r.name = 'works_at' AND e2.name = 'Acme Corp')
   OR (e1.name = 'Bob' AND r.name = 'works_at' AND e2.name = 'CityNet ISP')
   OR (e1.name = 'Acme Corp' AND r.name = 'located_in' AND e2.name = 'Metropolis')
   OR (e1.name = 'CityNet ISP' AND r.name = 'located_in' AND e2.name = 'Metropolis')
   OR (e1.name = 'CityNet ISP' AND r.name = 'partner_of' AND e2.name = 'Acme Corp')
ON CONFLICT DO NOTHING;

UPDATE kg_entities e
SET degree = COALESCE(t.in_deg,0) + COALESCE(t.out_deg,0)
FROM (
    SELECT head_id AS entity_id, COUNT(*) AS out_deg, 0 AS in_deg FROM kg_triples GROUP BY head_id
    UNION ALL
    SELECT tail_id AS entity_id, 0 AS out_deg, COUNT(*) AS in_deg FROM kg_triples GROUP BY tail_id
) t
WHERE e.entity_id = t.entity_id;

WITH positives AS (
    SELECT 
        kt.head_id,
        kt.relation_id,
        kt.tail_id,
        1 - (e2.embedding <=> ML_EMBED_TEXT(e1.name || ' ' || r.name, 'sentence-transformers')) AS similarity,
        CASE WHEN e1.type = 'Person' AND r.name = 'works_at' AND e2.type = 'Organization' THEN 1 ELSE 0 END AS type_match,
        e1.degree AS degree_head,
        e2.degree AS degree_tail,
        TRUE AS label
    FROM kg_triples kt
    JOIN kg_entities e1 ON kt.head_id = e1.entity_id
    JOIN kg_entities e2 ON kt.tail_id = e2.entity_id
    JOIN kg_relations r ON kt.relation_id = r.relation_id
),
negatives AS (
    SELECT 
        e1.entity_id AS head_id,
        r.relation_id,
        e2.entity_id AS tail_id,
        1 - (e2.embedding <=> ML_EMBED_TEXT(e1.name || ' ' || r.name, 'sentence-transformers')) AS similarity,
        CASE WHEN e1.type = 'Person' AND r.name = 'works_at' AND e2.type = 'Organization' THEN 1 ELSE 0 END AS type_match,
        e1.degree AS degree_head,
        e2.degree AS degree_tail,
        FALSE AS label
    FROM kg_entities e1
    CROSS JOIN kg_entities e2
    CROSS JOIN kg_relations r
    WHERE e1.entity_id <> e2.entity_id
      AND NOT EXISTS (
            SELECT 1 FROM kg_triples kt
            WHERE kt.head_id = e1.entity_id AND kt.relation_id = r.relation_id AND kt.tail_id = e2.entity_id
        )
      AND (r.name IN ('works_at','partner_of','located_in'))
    LIMIT 20
),
training AS (
    SELECT * FROM positives
    UNION ALL
    SELECT * FROM negatives
)
SELECT ML_TRAIN_MODEL(
    'kg_link_predictor',
    'gradient_boosting',
    ARRAY[similarity, type_match, degree_head, degree_tail],
    label
) FROM training;

WITH candidates AS (
    SELECT 
        e1.entity_id AS head_id,
        r.relation_id,
        e2.entity_id AS tail_id,
        1 - (e2.embedding <=> ML_EMBED_TEXT(e1.name || ' ' || r.name, 'sentence-transformers')) AS similarity,
        CASE WHEN e1.type = 'Person' AND r.name = 'works_at' AND e2.type = 'Organization' THEN 1 ELSE 0 END AS type_match,
        e1.degree AS degree_head,
        e2.degree AS degree_tail
    FROM kg_entities e1
    CROSS JOIN kg_entities e2
    JOIN kg_relations r ON r.name = 'works_at'
    WHERE e1.name = 'Alice' AND e1.entity_id <> e2.entity_id
      AND NOT EXISTS (
            SELECT 1 FROM kg_triples kt
            WHERE kt.head_id = e1.entity_id AND kt.relation_id = r.relation_id AND kt.tail_id = e2.entity_id
        )
)
SELECT 
    c.tail_id,
    ML_PREDICT('kg_link_predictor', ARRAY[c.similarity, c.type_match, c.degree_head, c.degree_tail]) AS link_probability
FROM candidates c
ORDER BY link_probability DESC
LIMIT 5;

ALTER TABLE kg_entities ADD COLUMN IF NOT EXISTS is_org BOOLEAN;
UPDATE kg_entities SET is_org = (type = 'Organization');

SELECT ML_TRAIN_MODEL(
    'kg_node_classifier',
    'logistic_regression',
    ARRAY[
        degree,
        1 - (embedding <=> ML_EMBED_TEXT('organization', 'sentence-transformers'))
    ],
    is_org
) FROM kg_entities;

SELECT 
    entity_id,
    name,
    ML_PREDICT(
        'kg_node_classifier',
        ARRAY[
            degree,
            1 - (embedding <=> ML_EMBED_TEXT('organization', 'sentence-transformers'))
        ]
    ) AS organization_probability
FROM kg_entities
ORDER BY organization_probability DESC;

SELECT 
    r.name AS relation_candidate,
    1 - (r.embedding <=> ML_EMBED_TEXT(eh.name || ' ' || et.name, 'sentence-transformers')) AS score
FROM kg_relations r
CROSS JOIN LATERAL (
    SELECT name FROM kg_entities WHERE name = 'Alice'
) AS eh(name)
CROSS JOIN LATERAL (
    SELECT name FROM kg_entities WHERE name = 'Acme Corp'
) AS et(name)
ORDER BY score DESC
LIMIT 3;
