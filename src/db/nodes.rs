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
    PeerInfo,
    AllLogs,
    Logs(String),
    LogStats,
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
                NodeQueryType::PeerInfo => self.get_peer_counts(node_ip, filters),
                NodeQueryType::AllLogs => self.get_all_logs(node_ip, filters),
                NodeQueryType::Logs(msg_type) => self.get_logs(node_ip, msg_type, filters),
                NodeQueryType::LogStats => self.get_log_stats(node_ip),
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
pub struct PeerInfoDb {
    #[sql_type = "Text"]
    node_ip: String,
    #[sql_type = "Timestamp"]
    ts: NaiveDateTime,
    #[sql_type = "Integer"]
    peer_count: i32,
    #[sql_type = "Nullable<Jsonb>"]
    not_connected: Option<Value>,
}

impl PeerInfoDb {
    pub fn get_not_connected(&self) -> Option<usize> {
        if let Some(value) = &self.not_connected {
            if let Some(obj) = value.as_object() {
                return Some(obj.len());
            }
        }
        None
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct PeerInfo {
    node_ip: String,
    ts: NaiveDateTime,
    peers_connected: i32,
    not_connected: Option<usize>,
}

impl From<PeerInfoDb> for PeerInfo {
    fn from(p: PeerInfoDb) -> Self {
        PeerInfo {
            not_connected: p.get_not_connected(),
            node_ip: p.node_ip,
            ts: p.ts,
            peers_connected: p.peer_count,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, QueryableByName)]
pub struct LogStats {
    #[sql_type = "BigInt"]
    pub qty: i64,
    #[sql_type = "Text"]
    pub log_type: String,
}

impl DbExecutor {
    fn get_log_stats(&self, node_ip: String) -> Result<Value, Error> {
        match self.with_connection(|conn| {
            let query = sql_query(
                "SELECT COUNT(log_type) as qty, log_type \
                 FROM (SELECT logs->>'msg' as log_type from substrate_logs WHERE node_ip LIKE $1) t \
                 GROUP BY t.log_type",
            )
            .bind::<Text, _>(format!("{}%", node_ip));
            debug!(
                "get_log_stats query: {}",
                diesel::debug_query::<diesel::pg::Pg, _>(&query)
            );
            let result: QueryResult<Vec<LogStats>> = query.get_results(conn);
            result
        }) {
            Ok(Ok(v)) => Ok(json!(v)),
            Ok(Err(e)) => Err(e.into()),
            Err(e) => Err(e.into()),
        }
    }

    fn get_peer_counts(&self, node_ip: String, filters: Filters) -> Result<Value, Error> {
        match self.with_connection(|conn| {
            let query = sql_query(
                "SELECT node_ip, \
                 CAST (logs->>'peers' as INTEGER) as peer_count, \
                 CAST (logs->>'ts' as TIMESTAMP) as ts, \
                 logs->'network_state'->'notConnectedPeers' as not_connected \
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
            let result: QueryResult<Vec<PeerInfoDb>> = query.get_results(conn);
            result
        }) {
            Ok(Ok(v)) => {
                let ncs: Vec<PeerInfo> = v.into_iter().map(|r| PeerInfo::from(r)).collect();
                Ok(json!(ncs))
            }
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

    fn get_all_logs(&self, node_ip: String, filters: Filters) -> Result<Value, Error> {
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

    fn get_logs(
        &self,
        node_ip: String,
        msg_type: String,
        filters: Filters,
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
                 AND logs->>'msg' = $4
                 ORDER BY created_at DESC \
                 LIMIT $5",
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
            .bind::<Text, _>(msg_type)
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
