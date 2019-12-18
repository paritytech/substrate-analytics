CREATE TABLE benchmark_events(
    id              SERIAL PRIMARY KEY,
    benchmark_id    INTEGER REFERENCES benchmarks(id) NOT NULL,
    name            VARCHAR NOT NULL,
    phase           VARCHAR NOT NULL,
    created_at      TIMESTAMP NOT NULL DEFAULT (NOW() AT TIME ZONE 'utc')
);