use actix::prelude::*;
use chrono::NaiveDateTime;
use diesel::sql_types::*;
use diesel::{result::QueryResult, sql_query, RunQueryDsl};
use failure::Error;
use serde_json::Value;

use super::DbExecutor;
use crate::db::models::SubstrateLog;

/// Message to indicate what information is required
/// Response is always json
pub enum NodesQuery {
    Nodes,
    PeerCounts {
        node_ip: String,
        start_time: NaiveDateTime,
        end_time: NaiveDateTime,
    },
    RecentLogs {
        node_ip: String,
        start_time: NaiveDateTime,
        end_time: NaiveDateTime,
        limit: Option<i32>,
    },
}

impl Message for NodesQuery {
    type Result = Result<Value, Error>;
}

impl Handler<NodesQuery> for DbExecutor {
    type Result = Result<Value, Error>;

    fn handle(&mut self, msg: NodesQuery, _: &mut Self::Context) -> Self::Result {
        match msg {
            NodesQuery::PeerCounts {
                node_ip,
                start_time,
                end_time,
            } => self.get_peer_counts(node_ip, start_time, end_time),
            NodesQuery::Nodes => self.get_nodes(),
            NodesQuery::RecentLogs {
                node_ip,
                start_time,
                end_time,
                limit,
            } => self.get_recent_logs(node_ip, start_time, end_time, limit),
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
    fn get_peer_counts(
        &self,
        node_ip: String,
        start_time: NaiveDateTime,
        end_time: NaiveDateTime,
    ) -> Result<Value, Error> {
        match self.with_connection(|conn| {
            let query = sql_query(
                "SELECT node_ip, \
                 CAST (logs->>'peers' as INTEGER) as peer_count, \
                 CAST (logs->>'ts' as TIMESTAMP) as ts \
                 FROM substrate_logs \
                 WHERE logs->>'msg' = 'system.interval' \
                 AND created_at > $1 \
                 AND created_at < $2 \
                 AND node_ip LIKE $3 \
                 GROUP BY node_ip, logs \
                 ORDER BY ts ASC \
                 LIMIT 1000",
            )
            .bind::<Timestamp, _>(start_time)
            .bind::<Timestamp, _>(end_time)
            .bind::<Text, _>(format!("{}%", node_ip));
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

    fn get_recent_logs(
        &self,
        node_ip: String,
        start_time: NaiveDateTime,
        end_time: NaiveDateTime,
        limit: Option<i32>,
    ) -> Result<Value, Error> {
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
            .bind::<Timestamp, _>(start_time)
            .bind::<Timestamp, _>(end_time)
            .bind::<Nullable<Integer>, _>(limit.unwrap_or(100));
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
