// Copyright 2020 Parity Technologies (UK) Ltd.
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
use chrono::NaiveDateTime;
use diesel::sql_types::*;
use diesel::{result::QueryResult, sql_query, RunQueryDsl};
use serde_json::Value;
use std::collections::HashMap;
use std::convert::TryInto;
use std::hash::Hash;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use super::{filters::Filters, DbExecutor, RECORD_LIMIT};

#[derive(Serialize, Deserialize, QueryableByName, Clone, Debug)]
pub struct SubstrateLog {
    #[sql_type = "Jsonb"]
    pub log: Value,
    #[sql_type = "Timestamp"]
    pub created_at: NaiveDateTime,
}

#[derive(Debug)]
pub struct PeerDataResponse {
    pub peer_message: PeerMessage,
    pub data: Vec<SubstrateLog>,
}

impl Message for PeerDataResponse {
    type Result = Result<(), &'static str>;
}

#[derive(Hash, Eq, PartialEq, Clone, Debug)]
pub struct PeerMessage {
    pub peer_id: String,
    pub msg: String,
}

pub struct PeerMessages(pub HashMap<PeerMessage, NaiveDateTime>);

#[derive(Clone, Debug)]
pub struct PeerMessageTime {
    pub peer_message: PeerMessage,
    pub time: NaiveDateTime,
}
#[derive(Debug)]
pub struct PeerMessageTimeList {
    pub list: Vec<PeerMessageTime>,
    pub cache: Recipient<PeerDataResponse>,
}

impl Message for PeerMessageTimeList {
    type Result = ();
}

impl Handler<PeerMessageTimeList> for DbExecutor {
    type Result = ();
    fn handle(&mut self, msg: PeerMessageTimeList, _ctx: &mut Self::Context) -> Self::Result {
        debug!("Handling PeerMessageStartTimeList");
        let cache = msg.cache;
        let pmuts = msg.list;
        for pmut in pmuts {
            let p = pmut.clone();
            let filters = Filters {
                start_time: Some(p.time),
                peer_id: Some(p.peer_message.peer_id),
                msg: Some(p.peer_message.msg),
                ..Default::default()
            };
            let pd_res = self.get_logs(filters);
            trace!("pd_res: {:?}", pd_res);
            if let Ok(pdr) = pd_res {
                // send to cache
                if let Err(e) = cache.do_send(pdr) {
                    error!("Sending PeerDataResponse to Cache failed : {:?}", e);
                }
            }
        }
    }
}

impl DbExecutor {
    fn get_logs(&self, filters: Filters) -> Result<PeerDataResponse, failure::Error> {
        let peer_id = filters.peer_id.clone().unwrap_or(String::new());
        let msg = filters.msg.clone().unwrap_or(String::new());
        let start_time = filters
            .start_time
            .unwrap_or_else(|| NaiveDateTime::from_timestamp(0, 0));
        match self.with_connection(|conn| {
            let query = sql_query(
                "SELECT sl.logs as log, \
                 sl.created_at \
                 FROM substrate_logs sl \
                 LEFT JOIN peer_connections pc ON sl.peer_connection_id = pc.id \
                 WHERE peer_id = $1 \
                 AND sl.created_at > $2 \
                 AND sl.logs->>'msg' = $3
                 ORDER BY created_at ASC \
                 LIMIT $4",
            )
            .bind::<Text, _>(peer_id.clone())
            .bind::<Timestamp, _>(start_time)
            .bind::<Text, _>(msg.clone())
            .bind::<Integer, _>(filters.limit.unwrap_or(RECORD_LIMIT));
            debug!(
                "get_profiling query: {}",
                diesel::debug_query::<diesel::pg::Pg, _>(&query)
            );
            let result: QueryResult<Vec<SubstrateLog>> = query.get_results(conn);
            result
        }) {
            Ok(Ok(data)) => {
                let peer_message = PeerMessage { peer_id, msg };
                Ok(PeerDataResponse { peer_message, data })
            }
            Ok(Err(e)) => Err(e.into()),
            Err(e) => Err(e.into()),
        }
    }
}

pub fn create_date_time(seconds_ago: u64) -> NaiveDateTime {
    let now = SystemTime::now();
    let ts = now
        .checked_sub(Duration::from_secs(seconds_ago))
        .expect("We should be using sane values for default_start_time");
    let ds = ts
        .duration_since(UNIX_EPOCH)
        .expect("We should be using sane values for default_start_time");
    NaiveDateTime::from_timestamp((ds.as_secs() as u64).try_into().unwrap(), 0)
}

impl std::fmt::Display for PeerMessage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "({}, {})", self.peer_id, self.msg)
    }
}
