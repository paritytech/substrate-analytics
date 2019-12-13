DROP TABLE IF EXISTS benchmarks CASCADE;
CREATE TABLE benchmarks(
    id              SERIAL PRIMARY KEY,
    setup           JSONB NOT NULL,
    created_at      TIMESTAMP NOT NULL DEFAULT (NOW() AT TIME ZONE 'utc')
);