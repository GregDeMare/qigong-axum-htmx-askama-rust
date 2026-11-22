-- Keep writes from racing with sequence initialization. SQLx runs this migration
-- in a transaction, so the lock lasts until the default is installed.
LOCK TABLE models IN ACCESS EXCLUSIVE MODE;

CREATE SEQUENCE models_id_seq AS INTEGER OWNED BY models.id;

-- An empty table (or one with only nonpositive IDs) starts at 1. Otherwise,
-- mark the maximum ID as used so the next value is strictly above existing IDs.
SELECT setval(
    'models_id_seq',
    GREATEST(COALESCE(MAX(id), 1), 1),
    COALESCE(MAX(id), 0) >= 1
) FROM models;

ALTER TABLE models ALTER COLUMN id SET DEFAULT nextval('models_id_seq');
