use actix::prelude::*;
use chrono::NaiveDateTime;
use diesel::sql_types::*;
use diesel::{result::QueryResult, sql_query, RunQueryDsl};
use failure::Error;
use serde_json::Value;

use super::DbExecutor;

/// Message to indicate what information is required
/// Response is always json
pub enum Query {
    DbSize,
    PeerCounts { node_ip: String },
    Nodes,
}

impl Message for Query {
    type Result = Result<Value, Error>;
}

impl Handler<Query> for DbExecutor {
    type Result = Result<Value, Error>;

    fn handle(&mut self, msg: Query, _: &mut Self::Context) -> Self::Result {
        match msg {
            Query::DbSize => self.get_db_size(),
            Query::PeerCounts { node_ip } => self.get_peer_counts(node_ip),
            Query::Nodes => self.get_nodes(),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, QueryableByName)]
pub struct Node {
    #[sql_type = "Text"]
    node_ip: String,
}

#[derive(Serialize, Deserialize, Debug, QueryableByName)]
pub struct PeerCount {
    #[sql_type = "Text"]
    node_ip: String,
    #[sql_type = "Timestamp"]
    ts: NaiveDateTime,
    #[sql_type = "Integer"]
    peer_count: i32,
}

#[derive(Serialize, Deserialize, Debug, QueryableByName)]
pub struct DbSize {
    #[sql_type = "Text"]
    relation: String,
    #[sql_type = "Text"]
    size: String,
}

impl DbExecutor {
    fn get_peer_counts(&self, node_ip: String) -> Result<Value, Error> {
        match self.with_connection(|conn| {
            let query = sql_query(
                "SELECT node_ip, \
                 CAST (logs->>'peers' as INTEGER) as peer_count, \
                 CAST (logs->>'ts' as TIMESTAMP) as ts \
                 FROM substrate_logs \
                 WHERE logs->> 'msg' = 'system.interval' \
                 AND node_ip LIKE $1 \
                 GROUP BY node_ip, logs \
                 ORDER BY ts ASC \
                 LIMIT 10000",
            )
            .bind::<Text, _>(format!("{}%", node_ip));
            let result: QueryResult<Vec<PeerCount>> = query.get_results(conn);
            result
        }) {
            Ok(Ok(v)) => Ok(json!(v)),
            Ok(Err(e)) => Err(e.into()),
            Err(e) => Err(e.into()),
        }
    }

    fn get_nodes(&self) -> Result<Value, Error> {
        match self.with_connection(|conn| {
            let query = "SELECT DISTINCT node_ip FROM substrate_logs";
            let result: QueryResult<Vec<Node>> = diesel::sql_query(query).get_results(conn);
            result
        }) {
            Ok(Ok(v)) => Ok(json!(v)),
            Ok(Err(e)) => Err(e.into()),
            Err(e) => Err(e.into()),
        }
    }

    fn get_db_size(&self) -> Result<Value, Error> {
        match self.with_connection(|conn| {
            let query = "SELECT nspname || '.' || relname AS relation, \
                         pg_size_pretty(pg_relation_size(C.oid)) AS size \
                         FROM pg_class C \
                         LEFT JOIN pg_namespace N ON (N.oid = C.relnamespace) \
                         WHERE nspname NOT IN ('pg_catalog', 'information_schema') \
                         ORDER BY pg_relation_size(C.oid) DESC \
                         LIMIT 1000;";
            let result: QueryResult<Vec<DbSize>> = diesel::sql_query(query).get_results(conn);
            result
        }) {
            Ok(Ok(v)) => Ok(json!(v)),
            Ok(Err(e)) => Err(e.into()),
            Err(e) => Err(e.into()),
        }
    }
}
