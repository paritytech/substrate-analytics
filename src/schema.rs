table! {
    benchmarking_systems (id) {
        id -> Int4,
        description -> Varchar,
        os -> Citext,
        cpu_qty -> Int4,
        cpu_clock -> Int4,
        memory -> Int4,
        disk_info -> Varchar,
    }
}

table! {
    peer_connections (id) {
        id -> Int4,
        ip_addr -> Varchar,
        peer_id -> Nullable<Varchar>,
        created_at -> Timestamp,
        audit -> Bool,
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
    benchmarking_systems,
    peer_connections,
    substrate_logs,
);
