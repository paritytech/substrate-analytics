table! {
    substrate_logs (id) {
        id -> Int4,
        node_ip -> Varchar,
        created_at -> Timestamp,
        logs -> Jsonb,
    }
}
