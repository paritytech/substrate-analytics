table! {
    benchmark_events (id) {
        id -> Int4,
        benchmark_id -> Int4,
        name -> Varchar,
        phase -> Varchar,
        created_at -> Timestamp,
    }
}

table! {
    benchmarks (id) {
        id -> Int4,
        setup -> Jsonb,
        created_at -> Timestamp,
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

joinable!(benchmark_events -> benchmarks (benchmark_id));
joinable!(substrate_logs -> peer_connections (peer_connection_id));

allow_tables_to_appear_in_same_query!(
    benchmark_events,
    benchmarks,
    host_systems,
    peer_connections,
    substrate_logs,
);
