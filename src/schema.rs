table! {
    benchmarks (id) {
        id -> Int4,
        ts_start -> Timestamp,
        ts_end -> Timestamp,
        description -> Nullable<Varchar>,
        chain_spec -> Nullable<Jsonb>,
        benchmark_spec -> Nullable<Jsonb>,
        host_system_id -> Int4,
    }
}

table! {
    host_systems (id) {
        id -> Int4,
        description -> Varchar,
        os -> Varchar,
        cpu_qty -> Int4,
        cpu_clock -> Int4,
        ram_mb -> Int4,
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

joinable!(benchmarks -> host_systems (host_system_id));
joinable!(substrate_logs -> peer_connections (peer_connection_id));

allow_tables_to_appear_in_same_query!(
    benchmarks,
    host_systems,
    peer_connections,
    substrate_logs,
);
