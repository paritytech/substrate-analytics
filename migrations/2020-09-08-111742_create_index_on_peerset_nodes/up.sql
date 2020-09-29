CREATE INDEX substrate_logs_peerset_nodes_idx ON substrate_logs USING GIN ((logs->'state'->'peerset'->'nodes') jsonb_path_ops);
