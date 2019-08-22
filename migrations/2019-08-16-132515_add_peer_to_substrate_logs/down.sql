ALTER TABLE substrate_logs DROP COLUMN peer_connection_id;
ALTER TABLE substrate_logs ADD COLUMN node_ip VARCHAR NOT NULL DEFAULT 'NULL';