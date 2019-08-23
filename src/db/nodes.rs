use actix::prelude::*;
use chrono::{NaiveDateTime, Utc};
use diesel::sql_types::*;
use diesel::{result::QueryResult, sql_query, RunQueryDsl};
use failure::Error;
use serde_json::Value;

use super::DbExecutor;
use crate::db::filters::Filters;

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
    ip_addr: String,
    #[sql_type = "Text"]
    peer_id: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, QueryableByName)]
pub struct SubstrateLogQueryDb {
    #[sql_type = "Text"]
    ip_addr: String,
    #[sql_type = "Text"]
    peer_id: String,
    #[sql_type = "Timestamp"]
    ts: NaiveDateTime,
    #[sql_type = "Timestamp"]
    created_at: NaiveDateTime,
    #[sql_type = "Jsonb"]
    logs: Value,
}

#[derive(Serialize, Deserialize, Debug, QueryableByName)]
pub struct PeerInfoDb {
    #[sql_type = "Text"]
    ip_addr: String,
    #[sql_type = "Text"]
    peer_id: String,
    #[sql_type = "Timestamp"]
    ts: NaiveDateTime,
    #[sql_type = "Integer"]
    peer_count: i32,
    #[sql_type = "Integer"]
    connection_id: i32,
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
    ip_addr: String,
    peer_id: String,
    connection_id: i32,
    ts: NaiveDateTime,
    peers_connected: i32,
    not_connected: Option<usize>,
}

impl From<PeerInfoDb> for PeerInfo {
    fn from(p: PeerInfoDb) -> Self {
        PeerInfo {
            not_connected: p.get_not_connected(),
            ip_addr: p.ip_addr,
            peer_id: p.peer_id,
            ts: p.ts,
            peers_connected: p.peer_count,
            connection_id: p.connection_id,
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
    fn get_log_stats(&self, peer_id: String) -> Result<Value, Error> {
        match self.with_connection(|conn| {
            let query = sql_query(
                "SELECT COUNT(log_type) as qty, log_type \
                 FROM ( \
                 SELECT logs->>'msg' AS log_type \
                 FROM substrate_logs sl \
                 LEFT JOIN peer_connections pc ON sl.peer_connection_id = pc.id \
                 WHERE peer_id = $1) t \
                 GROUP BY t.log_type",
            )
            .bind::<Text, _>(format!("{}", peer_id));
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

    fn get_peer_counts(&self, peer_id: String, filters: Filters) -> Result<Value, Error> {
        match self.with_connection(|conn| {
            let query = sql_query(
                "SELECT ip_addr, peer_id, pc.id as connection_id, \
                 CAST (logs->>'peers' as INTEGER) as peer_count, \
                 CAST (logs->>'ts' as TIMESTAMP) as ts, \
                 logs->'network_state'->'notConnectedPeers' as not_connected \
                 FROM substrate_logs sl \
                 LEFT JOIN peer_connections pc ON sl.peer_connection_id = pc.id \
                 WHERE logs->>'msg' = 'system.interval' \
                 AND peer_id = $1 \
                 AND sl.created_at > $2 \
                 AND sl.created_at < $3 \
                 GROUP BY pc.id, peer_id, ip_addr, sl.created_at, logs \
                 ORDER BY pc.id, ts ASC \
                 LIMIT $4",
            )
            .bind::<Text, _>(format!("{}", peer_id))
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
            let query = "SELECT DISTINCT ON (peer_id, ip_addr) * FROM peer_connections";
            let result: QueryResult<Vec<Node>> = diesel::sql_query(query).get_results(conn);
            result
        }) {
            Ok(Ok(v)) => Ok(json!(v)),
            Ok(Err(e)) => Err(e.into()),
            Err(e) => Err(e.into()),
        }
    }

    fn get_all_logs(&self, peer_id: String, filters: Filters) -> Result<Value, Error> {
        match self.with_connection(|conn| {
            let query = sql_query(
                "SELECT sl.id, \
                 ip_addr, \
                 peer_id, \
                 logs, \
                 CAST (logs->>'ts' as TIMESTAMP) as ts, \
                 sl.created_at, \
                 peer_connection_id \
                 FROM substrate_logs sl \
                 LEFT JOIN peer_connections pc ON sl.peer_connection_id = pc.id \
                 WHERE peer_id = $1 \
                 AND sl.created_at > $2 \
                 AND sl.created_at < $3 \
                 ORDER BY ts DESC \
                 LIMIT $4",
            )
            .bind::<Text, _>(format!("{}", peer_id))
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
            let result: QueryResult<Vec<SubstrateLogQueryDb>> = query.get_results(conn);
            result
        }) {
            Ok(Ok(v)) => Ok(json!(v)),
            Ok(Err(e)) => Err(e.into()),
            Err(e) => Err(e.into()),
        }
    }

    fn get_logs(
        &self,
        peer_id: String,
        msg_type: String,
        filters: Filters,
    ) -> Result<Value, Error> {
        match self.with_connection(|conn| {
            let query = sql_query(
                "SELECT sl.id, \
                 ip_addr, \
                 peer_id, \
                 logs, \
                 CAST (logs->>'ts' as TIMESTAMP) as ts, \
                 sl.created_at, \
                 peer_connection_id \
                 FROM substrate_logs sl \
                 LEFT JOIN peer_connections pc ON sl.peer_connection_id = pc.id \
                 WHERE peer_id = $1 \
                 AND sl.created_at > $2 \
                 AND sl.created_at < $3 \
                 AND logs->>'msg' = $4
                 ORDER BY ts DESC \
                 LIMIT $5",
            )
            .bind::<Text, _>(format!("{}", peer_id))
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
            let result: QueryResult<Vec<SubstrateLogQueryDb>> = query.get_results(conn);
            result
        }) {
            Ok(Ok(v)) => Ok(json!(v)),
            Ok(Err(e)) => Err(e.into()),
            Err(e) => Err(e.into()),
        }
    }
}
