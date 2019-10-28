CREATE TABLE benchmarks
(
    id             SERIAL PRIMARY KEY,
    ts_start       TIMESTAMP NOT NULL DEFAULT NOW(),
    ts_end         TIMESTAMP NOT NULL DEFAULT NOW(),
    description    VARCHAR,
    chain_spec     JSONB,
    benchmark_spec JSONB,
    host_system_id INTEGER REFERENCES host_systems (id) NOT NULL
);
