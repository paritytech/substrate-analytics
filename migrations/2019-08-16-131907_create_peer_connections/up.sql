CREATE TABLE peer_connections
(
    id         SERIAL    PRIMARY KEY,
    ip_addr    VARCHAR   NOT NULL,
    peer_id    VARCHAR,
    created_at TIMESTAMP NOT NULL DEFAULT NOW()
);