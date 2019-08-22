table! {
    peer_connections (id) {
        id -> Int4,
        ip_addr -> Varchar,
        peer_id -> Nullable<Varchar>,
        created_at -> Timestamp,
    }
}

table! {
    substrate_logs (id) {
        id -> Int4,
        created_at -> Timestamp,
        logs -> Jsonb,
        peer_connection_id -> Int4,
    }
}

joinable!(substrate_logs -> peer_connections (peer_connection_id));

allow_tables_to_appear_in_same_query!(
    peer_connections,
    substrate_logs,
);
