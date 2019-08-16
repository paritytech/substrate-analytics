CREATE TABLE peers
(
    id         SERIAL    PRIMARY KEY,
    ip_addr    VARCHAR   NOT NULL,
    peer_id    VARCHAR   NOT NULL,
    created_at TIMESTAMP NOT NULL DEFAULT NOW()
);