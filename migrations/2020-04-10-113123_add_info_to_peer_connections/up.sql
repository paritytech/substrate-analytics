ALTER TABLE peer_connections ADD COLUMN "name" VARCHAR;
ALTER TABLE peer_connections ADD COLUMN chain VARCHAR;
ALTER TABLE peer_connections ADD COLUMN version VARCHAR;
ALTER TABLE peer_connections ADD COLUMN authority BOOLEAN;
ALTER TABLE peer_connections ADD COLUMN startup_time BIGINT;
ALTER TABLE peer_connections ADD COLUMN implementation VARCHAR;