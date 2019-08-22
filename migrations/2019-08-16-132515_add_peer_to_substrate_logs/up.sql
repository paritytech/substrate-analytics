TRUNCATE substrate_logs;
ALTER TABLE substrate_logs DROP COLUMN node_ip;
ALTER TABLE substrate_logs ADD COLUMN peer_connection_id INTEGER REFERENCES peer_connections(id) NOT NULL;