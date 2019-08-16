table! {
    peers (id) {
        id -> Int4,
        ip_addr -> Varchar,
        peer_id -> Varchar,
        created_at -> Timestamp,
    }
}

table! {
    substrate_logs (id) {
        id -> Int4,
        node_ip -> Varchar,
        created_at -> Timestamp,
        logs -> Jsonb,
    }
}

allow_tables_to_appear_in_same_query!(
    peers,
    substrate_logs,
);
