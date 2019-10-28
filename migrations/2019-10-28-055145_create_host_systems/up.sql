CREATE TABLE host_systems
(
    id          SERIAL PRIMARY KEY,
    description VARCHAR NOT NULL,
    os          VARCHAR NOT NULL,
    cpu_qty     INTEGER NOT NULL,
    cpu_clock   INTEGER NOT NULL,
    ram_mb      INTEGER NOT NULL,
    disk_info   VARCHAR NOT NULL
);