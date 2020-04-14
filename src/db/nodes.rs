// Copyright 2019 Parity Technologies (UK) Ltd.
// This file is part of Substrate Analytics.

// Substrate Analytics is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// Substrate Analytics is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with Substrate Analytics.  If not, see <http://www.gnu.org/licenses/>.

use actix::prelude::*;
use chrono::{NaiveDateTime, Utc};
use diesel::sql_types::*;
use diesel::{result::QueryResult, sql_query, RunQueryDsl};
use failure::Error;
use serde_json::Value;

use super::{filters::Filters, DbExecutor, RECORD_LIMIT};
use crate::db::models::PeerConnection;

pub struct NodesQuery(pub Filters);

impl Message for NodesQuery {
    type Result = Result<Vec<PeerConnection>, Error>;
}

impl Handler<NodesQuery> for DbExecutor {
    type Result = Result<Vec<PeerConnection>, Error>;

    fn handle(&mut self, msg: NodesQuery, _: &mut Self::Context) -> Self::Result {
        self.get_nodes(msg.0)
    }
}

pub struct LogsQuery(pub Filters);

impl Message for LogsQuery {
    type Result = Result<Vec<Log>, Error>;
}

impl Handler<LogsQuery> for DbExecutor {
    type Result = Result<Vec<Log>, Error>;

    fn handle(&mut self, msg: LogsQuery, _: &mut Self::Context) -> Self::Result {
        let has_msg = msg.0.msg.is_some();
        let has_target = msg.0.target.is_some();
        if has_msg {
            if has_target {
                self.get_log_msgs_with_target(msg.0)
            } else {
                self.get_log_msgs(msg.0)
            }
        } else {
            self.get_all_logs(msg.0)
        }
    }
}

pub struct StatsQuery(pub Filters);

impl Message for StatsQuery {
    type Result = Result<Vec<Stats>, Error>;
}

impl Handler<StatsQuery> for DbExecutor {
    type Result = Result<Vec<Stats>, Error>;

    fn handle(&mut self, msg: StatsQuery, _: &mut Self::Context) -> Self::Result {
        self.get_log_stats(msg.0)
    }
}

#[derive(Serialize, Deserialize, Debug, QueryableByName)]
pub struct Node {
    #[sql_type = "Nullable<Text>"]
    pub peer_id: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, QueryableByName)]
pub struct Log {
    #[sql_type = "Text"]
    pub ip_addr: String,
    #[sql_type = "Text"]
    pub peer_id: String,
    #[sql_type = "Text"]
    pub msg: String,
    #[sql_type = "Timestamp"]
    pub created_at: NaiveDateTime,
    #[sql_type = "Jsonb"]
    pub logs: Value,
}

#[derive(Serialize, Deserialize, Debug, QueryableByName)]
pub struct Stats {
    #[sql_type = "BigInt"]
    pub qty: i64,
    #[sql_type = "Text"]
    pub log_type: String,
}

#[derive(Serialize, Deserialize, Debug, QueryableByName)]
pub struct PeerInfoDb {
    #[sql_type = "Text"]
    pub ip_addr: String,
    #[sql_type = "Text"]
    pub peer_id: String,
    #[sql_type = "Timestamp"]
    pub ts: NaiveDateTime,
    #[sql_type = "Integer"]
    pub peer_count: i32,
    #[sql_type = "Integer"]
    pub connection_id: i32,
    #[sql_type = "Nullable<Jsonb>"]
    pub not_connected: Option<Value>,
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

impl DbExecutor {
    fn get_log_stats(&self, filters: Filters) -> Result<Vec<Stats>, Error> {
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
            .bind::<Text, _>(filters.peer_id.unwrap_or(String::new()));
            debug!(
                "get_log_stats query: {}",
                diesel::debug_query::<diesel::pg::Pg, _>(&query)
            );
            let result: QueryResult<Vec<Stats>> = query.get_results(conn);
            result
        }) {
            Ok(Ok(v)) => Ok(v),
            Ok(Err(e)) => Err(e.into()),
            Err(e) => Err(e.into()),
        }
    }

    fn get_peer_counts(&self, filters: Filters) -> Result<Value, Error> {
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
            .bind::<Text, _>(filters.peer_id.unwrap_or(String::new()))
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
            .bind::<Integer, _>(filters.limit.unwrap_or(RECORD_LIMIT));
            debug!(
                "get_peer_counts query: {}",
                diesel::debug_query::<diesel::pg::Pg, _>(&query)
            );
            let result: QueryResult<Vec<PeerInfoDb>> = query.get_results(conn);
            result
        }) {
            Ok(Ok(v)) => Ok(json!(v)),
            Ok(Err(e)) => Err(e.into()),
            Err(e) => Err(e.into()),
        }
    }

    fn get_nodes(&self, _filters: Filters) -> Result<Vec<PeerConnection>, Error> {
        match self.with_connection(|conn| {
            let query = "SELECT DISTINCT ON (peer_id) peer_id, \
            id, ip_addr, created_at, audit, name, \
            chain, version, authority, startup_time, implementation \
             FROM peer_connections \
             ORDER BY peer_id, created_at DESC";
            let result: QueryResult<Vec<PeerConnection>> =
                diesel::sql_query(query).get_results(conn);
            result
        }) {
            Ok(Ok(v)) => Ok(v),
            Ok(Err(e)) => Err(e.into()),
            Err(e) => Err(e.into()),
        }
    }

    fn get_all_logs(&self, filters: Filters) -> Result<Vec<Log>, Error> {
        match self.with_connection(|conn| {
            let query = sql_query(
                "SELECT sl.id, \
                 ip_addr, \
                 peer_id, \
                 logs->>'msg' AS msg, \
                 logs, \
                 sl.created_at, \
                 peer_connection_id \
                 FROM substrate_logs sl \
                 LEFT JOIN peer_connections pc ON sl.peer_connection_id = pc.id \
                 WHERE peer_id = $1 \
                 AND sl.created_at > $2 \
                 AND sl.created_at < $3 \
                 ORDER BY created_at DESC \
                 LIMIT $4",
            )
            .bind::<Text, _>(filters.peer_id.unwrap_or(String::new()))
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
            .bind::<Integer, _>(filters.limit.unwrap_or(RECORD_LIMIT));
            debug!(
                "get_all_logs query: {}",
                diesel::debug_query::<diesel::pg::Pg, _>(&query)
            );
            let result: QueryResult<Vec<Log>> = query.get_results(conn);
            result
        }) {
            Ok(Ok(v)) => Ok(v),
            Ok(Err(e)) => Err(e.into()),
            Err(e) => Err(e.into()),
        }
    }

    fn get_log_msgs(&self, filters: Filters) -> Result<Vec<Log>, Error> {
        match self.with_connection(|conn| {
            let query = sql_query(
                "SELECT sl.id, \
                 ip_addr, \
                 peer_id, \
                 logs->>'msg' AS msg, \
                 logs, \
                 sl.created_at, \
                 peer_connection_id \
                 FROM substrate_logs sl \
                 LEFT JOIN peer_connections pc ON sl.peer_connection_id = pc.id \
                 WHERE peer_id = $1 \
                 AND sl.created_at > $2 \
                 AND sl.created_at < $3 \
                 AND logs->>'msg' = $4
                 ORDER BY created_at DESC \
                 LIMIT $5",
            )
            .bind::<Text, _>(filters.peer_id.unwrap_or(String::new()))
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
            .bind::<Text, _>(filters.msg.unwrap_or(String::new()))
            .bind::<Integer, _>(filters.limit.unwrap_or(RECORD_LIMIT));
            debug!(
                "get_log_msgs query: {}",
                diesel::debug_query::<diesel::pg::Pg, _>(&query)
            );
            let result: QueryResult<Vec<Log>> = query.get_results(conn);
            result
        }) {
            Ok(Ok(v)) => Ok(v),
            Ok(Err(e)) => Err(e.into()),
            Err(e) => Err(e.into()),
        }
    }

    fn get_log_msgs_with_target(&self, filters: Filters) -> Result<Vec<Log>, Error> {
        match self.with_connection(|conn| {
            let query = sql_query(
                "SELECT sl.id, \
                 ip_addr, \
                 peer_id, \
                 logs->>'msg' AS msg, \
                 logs, \
                 sl.created_at, \
                 peer_connection_id \
                 FROM substrate_logs sl \
                 LEFT JOIN peer_connections pc ON sl.peer_connection_id = pc.id \
                 WHERE peer_id = $1 \
                 AND sl.created_at > $2 \
                 AND sl.created_at < $3 \
                 AND logs->>'msg' = $4
                 AND logs->>'target' = $5
                 ORDER BY created_at DESC \
                 LIMIT $6",
            )
            .bind::<Text, _>(filters.peer_id.unwrap_or(String::new()))
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
            .bind::<Text, _>(filters.msg.unwrap_or(String::new()))
            .bind::<Text, _>(filters.target.unwrap_or(String::new()))
            .bind::<Integer, _>(filters.limit.unwrap_or(RECORD_LIMIT));
            debug!(
                "Query: get_log_msgs_with_target: {}",
                diesel::debug_query::<diesel::pg::Pg, _>(&query)
            );
            let result: QueryResult<Vec<Log>> = query.get_results(conn);
            result
        }) {
            Ok(Ok(v)) => Ok(v),
            Ok(Err(e)) => Err(e.into()),
            Err(e) => Err(e.into()),
        }
    }
}
