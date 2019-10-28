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

use super::DbExecutor;
use crate::db::filters::Filters;
use actix::prelude::*;
use chrono::{NaiveDateTime, Utc};
use diesel::sql_types::*;
use diesel::{result::QueryResult, sql_query, QueryDsl, RunQueryDsl};
use failure::Error;
use std::time::Duration;

/// Message to indicate what information is required
/// Response is always json
pub enum Query {
    All(Filters),
    Logged(Filters),
    Selected(Vec<String>, Filters),
    Mock(usize),
}

impl Message for Query {
    type Result = Result<Vec<PeerReputations>, Error>;
}

impl Handler<Query> for DbExecutor {
    type Result = Result<Vec<PeerReputations>, Error>;

    fn handle(&mut self, msg: Query, _: &mut Self::Context) -> Self::Result {
        match msg {
            Query::All(filters) => self.get_reputation_all(filters),
            Query::Logged(filters) => {
                self.get_reputation_selected(self.get_logged_nodes()?, filters)
            }
            Query::Selected(selected, filters) => self.get_reputation_selected(selected, filters),
            Query::Mock(qty) => self.get_mock_results(qty),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, QueryableByName)]
pub struct PeerReputations {
    #[sql_type = "Text"]
    reporting_peer: String,
    #[sql_type = "Array<Text>"]
    remote_peer: Vec<String>,
    #[sql_type = "Array<BigInt>"]
    reputation: Vec<i64>,
    #[sql_type = "Array<Bool>"]
    connected: Vec<bool>,
    #[sql_type = "Timestamp"]
    ts: NaiveDateTime,
}

fn start_time_from_offset(offset_s: u64) -> NaiveDateTime {
    let utc_now = Utc::now();
    let utc = utc_now
        .checked_sub_signed(
            chrono::Duration::from_std(Duration::from_secs(offset_s))
                .unwrap_or(chrono::Duration::seconds(60)),
        )
        .unwrap_or(utc_now);
    NaiveDateTime::from_timestamp_opt(utc.timestamp(), utc.timestamp_subsec_nanos())
        .unwrap_or(NaiveDateTime::from_timestamp(60, 0))
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

    fn get_reputation_all(&self, filters: Filters) -> Result<Vec<PeerReputations>, Error> {
        match self.with_connection(|conn| {
            let max_age_s = filters.max_age_s.unwrap_or_else(|| 60);
            let start_time = start_time_from_offset(max_age_s as u64);
            let sql =
                "SELECT  \
                    DISTINCT ON (reporting_peer) \
                    peer_id as reporting_peer, \
                    array_agg(peers.key::varchar) as remote_peer, \
                    array_agg(jsonb_extract_path_text(peers.value, 'reputation')::bigint) as reputation, \
                    array_agg(jsonb_extract_path_text(peers.value, 'connected')::boolean) as connected, \
                    sl.created_at as ts \
                FROM peer_connections pc \
                    INNER JOIN substrate_logs sl \
                ON peer_connection_id = pc.id \
                    AND logs->>'msg' = 'system.interval' \
                    AND sl.created_at > $1 AT TIME ZONE 'UTC', \
                    lateral jsonb_each(logs->'network_state'->'peerset'->'nodes') as peers \
                WHERE sl.id = ANY (\
                    SELECT DISTINCT ON (peer_id) substrate_logs.id \
                    FROM substrate_logs \
                    INNER JOIN peer_connections ON peer_connection_id = peer_connections.id \
                    WHERE logs ->> 'msg' = 'system.interval' \
                    AND substrate_logs.created_at > $2 AT TIME ZONE 'UTC' \
                    ORDER BY peer_id, substrate_logs.created_at DESC \
                    ) \
                GROUP BY reporting_peer, sl.created_at \
                LIMIT $3";
            let query = sql_query(sql)
                .bind::<Timestamp, _>(start_time)
                .bind::<Timestamp, _>(start_time)
                .bind::<Integer, _>(filters.limit.unwrap_or(100));
            debug!(
                "get_reputation_all query: {}",
                diesel::debug_query::<diesel::pg::Pg, _>(&query)
            );
            let result: QueryResult<Vec<PeerReputations>> = query.get_results(conn);
            result
        }) {
            Ok(Ok(v)) => Ok(v),
            Ok(Err(e)) => Err(e.into()),
            Err(e) => Err(e.into()),
        }
    }

    fn get_reputation_selected(
        &self,
        selected: Vec<String>,
        filters: Filters,
    ) -> Result<Vec<PeerReputations>, Error> {
        match self.with_connection(|conn| {
            let max_age_s = filters.max_age_s.unwrap_or_else(|| 60);
            let start_time = start_time_from_offset(max_age_s as u64);
            let sql =
                "SELECT  \
                    DISTINCT ON (reporting_peer) \
                    peer_id as reporting_peer, \
                    array_agg(peers.key::varchar) as remote_peer, \
                    array_agg(jsonb_extract_path_text(peers.value, 'reputation')::bigint) as reputation, \
                    array_agg(jsonb_extract_path_text(peers.value, 'connected')::boolean) as connected, \
                    sl.created_at as ts \
                FROM peer_connections pc \
                    INNER JOIN substrate_logs sl \
                ON peer_connection_id = pc.id \
                    AND logs->>'msg' = 'system.interval' \
                    AND sl.created_at > $1 AT TIME ZONE 'UTC', \
                    lateral jsonb_each(logs->'network_state'->'peerset'->'nodes') as peers \
                WHERE key::text = ANY ($2) \
                    AND sl.id = ANY (\
                    SELECT DISTINCT ON (peer_id) substrate_logs.id \
                    FROM substrate_logs \
                    INNER JOIN peer_connections ON peer_connection_id = peer_connections.id \
                    WHERE logs ->> 'msg' = 'system.interval' \
                    AND substrate_logs.created_at > $3 AT TIME ZONE 'UTC' \
                    ORDER BY peer_id, substrate_logs.created_at DESC \
                    ) \
                GROUP BY reporting_peer, sl.created_at \
                LIMIT $4";
            let query = sql_query(sql)
                .bind::<Timestamp, _>(start_time)
                .bind::<Array<Text>, _>(selected)
                .bind::<Timestamp, _>(start_time)
                .bind::<Integer, _>(filters.limit.unwrap_or(100));

            debug!(
                "get_reputation_selected query: {}",
                diesel::debug_query::<diesel::pg::Pg, _>(&query)
            );
            let result: QueryResult<Vec<PeerReputations>> = query.get_results(conn);
            result
        }) {
            Ok(Ok(v)) => Ok(v),
            Ok(Err(e)) => Err(e.into()),
            Err(e) => Err(e.into()),
        }
    }

    /// For front-end tests
    pub fn get_mock_results(&self, max: usize) -> Result<Vec<PeerReputations>, Error> {
        use rand::distributions::Distribution;
        use rand::Rng;
        use statrs::distribution::Exponential;
        let mut r = rand::thread_rng();
        let n = Exponential::new(0.01).unwrap();
        let mut results: Vec<PeerReputations> = Vec::new();
        let mut peer_ids = vec![
            "QmSk5HQbn6LhUwDiNMseVUjuRYhEtYj4aUZ6WfWoGURpdV",
            "QmSk5HQbn6LhUwDiNMseVUjuRYhEtYj4aUZ6WfWoGURpdW",
            "QmWv9Ww7znzgLFyCzf21SR6tUKXrmHCZH9KhebeH4gyE9f",
            "QmWv9Ww7znzgLFyCzf21SR6tUKXrmHCZH9KhebeH4gyE9g",
            "QmTtcYKJho9vFmqtMA548QBSmLbmwAkBSiEKK3kWKfb6bJ",
            "QmTtcYKJho9vFmqtMA548QBSmLbmwAkBSiEKK3kWKfb6bK",
            "QmQJmDorK9c8KjMF5PdWiH2WGUXyzJtgTeJ55S5gggdju6",
            "QmQJmDorK9c8KjMF5PdWiH2WGUXyzJtgTeJ55S5gggdju7",
        ];
        if max < peer_ids.len() {
            peer_ids.split_off(max);
        };
        for peer_id in peer_ids.clone() {
            let mut p = vec![];
            let mut c = vec![];
            let mut re = vec![];
            for peer_id2 in peer_ids.clone() {
                if peer_id == peer_id2 {
                    continue;
                }
                let x = n.sample(&mut r) as i64;
                let y = r.gen_range(-1_000, 10_000);
                let rep: i64;
                if y < 0 {
                    rep = -(x * y * y);
                } else {
                    rep = 0;
                }
                p.push(peer_id2.to_string());
                c.push(true);
                re.push(rep);
            }
            results.push(PeerReputations {
                reporting_peer: peer_id.to_string(),
                remote_peer: p,
                reputation: re,
                connected: c,
                ts: start_time_from_offset(10),
            });
        }
        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test() {}
}
