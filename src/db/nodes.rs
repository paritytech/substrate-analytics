use actix::prelude::*;
use chrono::{NaiveDateTime, Utc};
use diesel::sql_types::*;
use diesel::{result::QueryResult, sql_query, RunQueryDsl};
use failure::Error;
use serde_json::Value;

use super::DbExecutor;
use crate::db::filters::Filters;
use crate::db::models::SubstrateLog;

pub enum NodeQueryType {
    PeerCounts,
    RecentLogs,
}

/// Message to indicate what information is required
/// Response is always json
pub enum NodesQuery {
    AllNodes,
    Node {
        node_ip: String,
        filters: Filters,
        kind: NodeQueryType,
    },
}

impl Message for NodesQuery {
    type Result = Result<Value, Error>;
}

impl Handler<NodesQuery> for DbExecutor {
    type Result = Result<Value, Error>;

    fn handle(&mut self, msg: NodesQuery, _: &mut Self::Context) -> Self::Result {
        match msg {
            NodesQuery::AllNodes => self.get_nodes(),
            NodesQuery::Node {
                node_ip,
                filters,
                kind,
            } => match kind {
                NodeQueryType::PeerCounts => self.get_peer_counts(node_ip, filters),
                NodeQueryType::RecentLogs => self.get_recent_logs(node_ip, filters),
            },
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

impl DbExecutor {
    fn get_peer_counts(&self, node_ip: String, filters: Filters) -> Result<Value, Error> {
        match self.with_connection(|conn| {
            let query = sql_query(
                "SELECT node_ip, \
                 CAST (logs->>'peers' as INTEGER) as peer_count, \
                 CAST (logs->>'ts' as TIMESTAMP) as ts \
                 FROM substrate_logs \
                 WHERE logs->>'msg' = 'system.interval' \
                 AND node_ip LIKE $1 \
                 AND created_at > $2 \
                 AND created_at < $3 \
                 GROUP BY node_ip, created_at, logs \
                 ORDER BY created_at ASC \
                 LIMIT $4",
            )
            .bind::<Text, _>(format!("{}%", node_ip))
            .bind::<Timestamp, _>(
                filters
                    .start_time
                    .unwrap_or_else(|| NaiveDateTime::from_timestamp(0, 0)),
            )
            .bind::<Timestamp, _>(
                filters
                    .end_time
                    .unwrap_or_else(|| NaiveDateTime::from_timestamp(Utc::now().timestamp(), 0)),
            )
            .bind::<Integer, _>(filters.limit.unwrap_or(1000));
            debug!(
                "get_peer_counts query: {}",
                diesel::debug_query::<diesel::pg::Pg, _>(&query)
            );
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

    fn get_recent_logs(&self, node_ip: String, filters: Filters) -> Result<Value, Error> {
        match self.with_connection(|conn| {
            let query = sql_query(
                "SELECT id, \
                 node_ip, \
                 logs, \
                 created_at \
                 FROM substrate_logs \
                 WHERE node_ip LIKE $1 \
                 AND created_at > $2 \
                 AND created_at < $3 \
                 ORDER BY created_at DESC \
                 LIMIT $4",
            )
            .bind::<Text, _>(format!("{}%", node_ip))
            .bind::<Timestamp, _>(
                filters
                    .start_time
                    .unwrap_or_else(|| NaiveDateTime::from_timestamp(0, 0)),
            )
            .bind::<Timestamp, _>(
                filters
                    .end_time
                    .unwrap_or_else(|| NaiveDateTime::from_timestamp(Utc::now().timestamp(), 0)),
            )
            .bind::<Integer, _>(filters.limit.unwrap_or(100));
            debug!(
                "get_recent_logs query: {}",
                diesel::debug_query::<diesel::pg::Pg, _>(&query)
            );
            let result: QueryResult<Vec<SubstrateLog>> = query.get_results(conn);
            result
        }) {
            Ok(Ok(v)) => Ok(json!(v)),
            Ok(Err(e)) => Err(e.into()),
            Err(e) => Err(e.into()),
        }
    }
}
