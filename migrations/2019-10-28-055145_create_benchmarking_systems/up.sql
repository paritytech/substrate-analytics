CREATE EXTENSION IF NOT EXISTS citext WITH SCHEMA public;
CREATE TABLE benchmarking_systems (
                                      id SERIAL PRIMARY KEY,
                                      description VARCHAR NOT NULL,
                                      os citext NOT NULL,
                                      cpu_qty INTEGER NOT NULL,
                                      cpu_clock INTEGER NOT NULL,
                                      memory INTEGER NOT NULL,
                                      disk_info VARCHAR NOT NULL
);