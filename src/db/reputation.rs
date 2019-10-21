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
use diesel::sql_types::*;
use diesel::{result::QueryResult, sql_query, QueryDsl, RunQueryDsl};
use failure::Error;
use serde_json::Value;

use super::DbExecutor;
use crate::db::filters::Filters;

/// Message to indicate what information is required
/// Response is always json
pub enum Query {
    All(Filters),
    Logged(Filters),
    Selected(Vec<String>, Filters),
}

impl Message for Query {
    type Result = Result<Value, Error>;
}

impl Handler<Query> for DbExecutor {
    type Result = Result<Value, Error>;

    fn handle(&mut self, msg: Query, _: &mut Self::Context) -> Self::Result {
        match msg {
            Query::All(filters) => self.get_reputation_all(filters),
            Query::Logged(filters) => {
                self.get_reputation_selected(self.get_logged_nodes()?, filters)
            }
            Query::Selected(selected, filters) => self.get_reputation_selected(selected, filters),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, QueryableByName)]
pub struct PeerReputation {
    #[sql_type = "Text"]
    reporting_peer: String,
    #[sql_type = "Text"]
    remote_peer: String,
    #[sql_type = "BigInt"]
    peer_reputation: i64,
    #[sql_type = "Bool"]
    connected: bool,
}

impl DbExecutor {
    fn get_logged_nodes(&self) -> Result<Vec<String>, Error> {
        match self.with_connection(|conn| {
            use crate::schema::peer_connections::dsl::*;
            peer_connections
                .select(peer_id)
                .distinct()
                .load::<Option<String>>(conn)
        }) {
            Ok(Ok(v)) => {
                let r = v.into_iter().filter_map(|c| c).collect();
                Ok(r)
            }
            Ok(Err(e)) => Err(e.into()),
            Err(e) => Err(e.into()),
        }
    }

    fn get_reputation_all(&self, filters: Filters) -> Result<Value, Error> {
        match self.with_connection(|conn| {
            // diesel bind doesn't work with interval so must use format!
            let max_age_s = filters.max_age_s.unwrap_or_else(|| 60).to_string();
            let sql = format!(
                "SELECT \
                 DISTINCT ON (reporting_peer, remote_peer) \
                 peer_id as reporting_peer, \
                 peer.key as remote_peer, \
                 jsonb_extract_path_text(peer.value, 'reputation')::bigint as peer_reputation, \
                 jsonb_extract_path_text(peer.value, 'connected')::boolean as connected \
                 FROM peer_connections pc \
                 INNER JOIN substrate_logs sl \
                 ON peer_connection_id = pc.id \
                 AND logs->>'msg' = 'system.interval' \
                 AND sl.created_at > (now() - interval '{}' second) AT TIME ZONE 'UTC', \
                 LATERAL jsonb_each(logs->'network_state'->'peerset'->'nodes') as peer \
                 WHERE sl.id = ANY ( \
                 SELECT DISTINCT ON (peer_id) substrate_logs.id \
                 FROM substrate_logs \
                 INNER JOIN peer_connections ON peer_connection_id = peer_connections.id \
                 WHERE logs ->> 'msg' = 'system.interval' \
                 AND substrate_logs.created_at > (now() - interval '{}' second) AT TIME ZONE 'UTC' \
                 ORDER BY peer_id, substrate_logs.created_at DESC \
                 ) \
                 ORDER BY reporting_peer, remote_peer, sl.created_at DESC \
                 LIMIT $1",
                max_age_s, max_age_s
            );
            let query = sql_query(sql).bind::<Integer, _>(filters.limit.unwrap_or(100));
            let result: QueryResult<Vec<PeerReputation>> = query.get_results(conn);
            result
        }) {
            Ok(Ok(v)) => Ok(json!(v)),
            Ok(Err(e)) => Err(e.into()),
            Err(e) => Err(e.into()),
        }
    }

    fn get_reputation_selected(
        &self,
        selected: Vec<String>,
        filters: Filters,
    ) -> Result<Value, Error> {
        match self.with_connection(|conn| {
            let max_age_s = filters.max_age_s.unwrap_or_else(|| 60).to_string();
            // diesel bind doesn't work with interval so must use format!
            let sql = format!(
                "SELECT \
                 DISTINCT ON (reporting_peer, remote_peer) \
                 peer_id as reporting_peer, \
                 peer.key as remote_peer, \
                 jsonb_extract_path_text(peer.value, 'reputation')::bigint as peer_reputation, \
                 jsonb_extract_path_text(peer.value, 'connected')::boolean as connected \
                 FROM peer_connections pc \
                 INNER JOIN substrate_logs sl \
                 ON peer_connection_id = pc.id \
                 AND logs->>'msg' = 'system.interval' \
                 AND sl.created_at > (now() - interval '{} seconds') AT TIME ZONE 'UTC', \
                 LATERAL jsonb_each(logs->'network_state'->'peerset'->'nodes') as peer \
                 WHERE key::text = ANY ($1) \
                 AND sl.id = ANY ( \
                 SELECT DISTINCT ON (peer_id) substrate_logs.id \
                 FROM substrate_logs \
                 INNER JOIN peer_connections ON peer_connection_id = peer_connections.id \
                 WHERE logs ->> 'msg' = 'system.interval' \
                 AND substrate_logs.created_at > (now() - interval '{}' second) AT TIME ZONE 'UTC' \
                 ORDER BY peer_id, substrate_logs.created_at DESC \
                 ) \
                 ORDER BY reporting_peer, remote_peer DESC \
                 LIMIT $2",
                max_age_s, max_age_s
            );
            let query = sql_query(sql)
                .bind::<Array<Text>, _>(selected)
                .bind::<Integer, _>(filters.limit.unwrap_or(100));
            let result: QueryResult<Vec<PeerReputation>> = query.get_results(conn);
            result
        }) {
            Ok(Ok(v)) => Ok(json!(v)),
            Ok(Err(e)) => Err(e.into()),
            Err(e) => Err(e.into()),
        }
    }
}
