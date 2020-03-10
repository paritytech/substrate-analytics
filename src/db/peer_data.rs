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
use chrono::{NaiveDateTime, Utc};
use diesel::sql_types::*;
use diesel::{result::QueryResult, sql_query, RunQueryDsl};
use std::collections::HashSet;
use std::convert::TryInto;
use std::hash::{Hash, Hasher};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use super::{filters::Filters, DbExecutor, RECORD_LIMIT};
use futures::{FutureExt, TryFutureExt};

pub trait CreatedAt {
    fn created_at(&self) -> &NaiveDateTime;
}

impl CreatedAt for PeerData {
    fn created_at(&self) -> &NaiveDateTime {
        match self {
            PeerData::Profiling(v) => &v.created_at,
            PeerData::Interval(v) => &v.created_at,
        }
    }
}

#[derive(Clone, Debug)]
pub enum PeerData {
    Profiling(Profiling),
    Interval(Interval),
}

#[derive(Debug)]
pub struct PeerDataResponse {
    pub peer_id: String,
    pub data: Vec<PeerData>,
}

impl Message for PeerDataResponse {
    type Result = Result<(), &'static str>;
}

#[derive(Hash, Eq, PartialEq, Clone, Debug)]
pub struct PeerMessage {
    pub peer_id: String,
    pub msg: String,
}

#[derive(Debug)]
pub struct PeerMessages(pub HashSet<PeerMessage>);

#[derive(Clone, Debug)]
pub struct PeerMessageStartTime {
    pub peer_message: PeerMessage,
    pub last_accessed: NaiveDateTime,
}

// TODO need to refactor the types
impl PartialEq for PeerMessageStartTime {
    fn eq(&self, other: &Self) -> bool {
        self.peer_message == other.peer_message
    }
}
impl Eq for PeerMessageStartTime {}

impl Hash for PeerMessageStartTime {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.peer_message.hash(state);
    }
}

#[derive(Debug)]
pub struct SubscriberInfo {
    pub peer_messages: PeerMessages,
    pub last_updated: NaiveDateTime,
}

impl SubscriberInfo {
    pub fn new() -> Self {
        SubscriberInfo {
            peer_messages: PeerMessages(HashSet::new()),
            last_updated: create_date_time(0),
        }
    }
}

pub struct PeerMessageStartTimeRequest(pub Addr<crate::cache::Cache>);

impl Message for PeerMessageStartTimeRequest {
    type Result = Result<PeerMessageStartTimeList, &'static str>;
}

#[derive(Debug)]
pub struct PeerMessageStartTimeList(pub Vec<PeerMessageStartTime>);

impl Message for PeerMessageStartTimeList {
    type Result = ();
}

#[derive(Clone)]
pub struct UpdateCache(pub Addr<DbExecutor>);

impl Message for UpdateCache {
    type Result = Result<(), &'static str>;
}

impl Handler<UpdateCache> for DbExecutor {
    type Result = Result<(), &'static str>;

    fn handle(&mut self, msg: UpdateCache, ctx: &mut Self::Context) -> Self::Result {
        debug!("Updating Cache");
        // TODO limit concurrency to 1
        let UpdateCache(addr) = msg;
        let req = self
            .cache
            .send(PeerMessageStartTimeRequest(self.cache.clone()));

        let res = futures::executor::block_on(req);
        let res = res.map(|r| async move {
            match r {
                Ok(pmsts) => {
                    trace!("sending: {:?}", pmsts);
                    addr.do_send(pmsts);
                }
                Err(e) => error!("Unable to send PeerMessageStartTimeRequest : {:?}", e),
            }
        });
        if let Ok(res1) = res {
            futures::executor::block_on(res1);
        }

        impl Handler<PeerMessageStartTimeList> for DbExecutor {
            type Result = ();
            fn handle(
                &mut self,
                msg: PeerMessageStartTimeList,
                ctx: &mut Self::Context,
            ) -> Self::Result {
                debug!("Handling PeerMessageStartTimeList");
                let PeerMessageStartTimeList(pmuts) = msg;
                for pmut in pmuts {
                    let p = pmut.clone();
                    self.get_latest_msgs(
                        p.last_accessed,
                        p.peer_message.peer_id,
                        p.peer_message.msg,
                    );
                }
            }
        }
        Ok(())
    }
}

impl DbExecutor {
    fn get_latest_msgs(&self, start_time: NaiveDateTime, peer_id: String, msg: String) {
        let filters = Filters {
            start_time: Some(start_time),
            peer_id: Some(peer_id),
            msg: Some(msg),
            ..Default::default()
        };
        let pd_res = match filters.msg.clone().unwrap().as_ref() {
            "system.interval" => self.get_interval(filters),
            "system.interval" => self.get_profiling(filters),
            m => {
                warn!("PeerDataRequest not available for msg type: `{}`", m);
                return;
            }
        };
        debug!("pd_res: {:?}", pd_res);
        if let Ok(pdr) = pd_res {
            // send to cache
            let req = self.cache.send(pdr);
            futures::executor::block_on(req);
        }
    }
}

impl DbExecutor {
    fn get_profiling(&self, filters: Filters) -> Result<PeerDataResponse, failure::Error> {
        let peer_id = filters.peer_id.clone().unwrap_or(String::new());
        match self.with_connection(|conn| {
            let query = sql_query(
                "SELECT CAST(logs->>'ns' AS bigint) AS ns,\
                 logs->>'name' AS name, \
                 logs->>'target' AS target, \
                 created_at\
                 FROM ( \
                 SELECT logs->>'msg' AS log_type \
                 FROM substrate_logs sl \
                 LEFT JOIN peer_connections pc ON sl.peer_connection_id = pc.id \
                 WHERE peer_id = $1 \
                 AND sl.created_at > $2 \
                 AND logs->>'msg' = $3
                 ORDER BY created_at DESC \
                 LIMIT $4",
            )
            .bind::<Text, _>(peer_id.clone())
            .bind::<Timestamp, _>(
                filters
                    .start_time
                    .unwrap_or_else(|| NaiveDateTime::from_timestamp(0, 0)),
            )
            .bind::<Text, _>(filters.msg.unwrap_or(String::new()))
            .bind::<Integer, _>(filters.limit.unwrap_or(RECORD_LIMIT));
            debug!(
                "get_profiling query: {}",
                diesel::debug_query::<diesel::pg::Pg, _>(&query)
            );
            let result: QueryResult<Vec<Profiling>> = query.get_results(conn);
            result
        }) {
            Ok(Ok(v)) => {
                let data: Vec<PeerData> = v.into_iter().map(|p| PeerData::Profiling(p)).collect();
                Ok(PeerDataResponse { peer_id, data })
            }
            Ok(Err(e)) => Err(e.into()),
            Err(e) => Err(e.into()),
        }
    }
}

#[derive(Serialize, Deserialize, QueryableByName, Clone, Debug)]
pub struct Profiling {
    #[sql_type = "BigInt"]
    pub ns: i64,
    #[sql_type = "Text"]
    pub name: String,
    #[sql_type = "Text"]
    pub target: String,
    #[sql_type = "Timestamp"]
    pub created_at: NaiveDateTime,
}

impl DbExecutor {
    fn get_interval(&self, filters: Filters) -> Result<PeerDataResponse, failure::Error> {
        let peer_id = filters.peer_id.clone().unwrap_or(String::new());
        match self.with_connection(|conn| {
            let query = sql_query(
                "SELECT CAST(logs->>'cpu' AS FLOAT) AS cpu, \
                 CAST(logs->>'peers' AS INTEGER) AS peers, \
                 CAST(logs->>'height' AS INTEGER) AS height, \
                 CAST(logs->>'memory' AS INTEGER) AS memory, \
                 CAST(logs->>'txcount' AS INTEGER) AS txcount, \
                 CAST(logs->>'finalized_height' AS INTEGER) AS finalized_height, \
                 CAST(logs->>'bandwidth_upload' AS INTEGER) AS bandwidth_upload, \
                 CAST(logs->>'bandwidth_download' AS INTEGER) AS bandwidth_download, \
                 CAST(logs->>'disk_read_per_sec' AS INTEGER) AS disk_read_per_sec, \
                 CAST(logs->>'disk_write_per_sec' AS INTEGER) AS disk_write_per_sec, \
                 CAST(logs->>'used_db_cache_size' AS INTEGER) AS used_db_cache_size, \
                 CAST(logs->>'used_state_cache_size' AS INTEGER) AS used_state_cache_size, \
                 logs->>'finalized_hash' AS finalized_hash, \
                 logs->>'best' AS best, \
                 sl.created_at as created_at \
                 FROM substrate_logs sl \
                 LEFT JOIN peer_connections pc ON sl.peer_connection_id = pc.id \
                 WHERE pc.peer_id = $1 \
                 AND sl.created_at > $2 \
                 AND logs->>'msg' = $3
                 ORDER BY sl.created_at DESC \
                 LIMIT $4",
            )
            .bind::<Text, _>(peer_id.clone())
            .bind::<Timestamp, _>(
                filters
                    .start_time
                    .unwrap_or_else(|| NaiveDateTime::from_timestamp(0, 0)),
            )
            .bind::<Text, _>(filters.msg.unwrap_or(String::new()))
            .bind::<Integer, _>(filters.limit.unwrap_or(RECORD_LIMIT));
            debug!(
                "get_profiling query: {}",
                diesel::debug_query::<diesel::pg::Pg, _>(&query)
            );
            let result: QueryResult<Vec<Interval>> = query.get_results(conn);
            result
        }) {
            Ok(Ok(v)) => {
                let data: Vec<PeerData> = v.into_iter().map(|p| PeerData::Interval(p)).collect();
                Ok(PeerDataResponse { peer_id, data })
            }
            Ok(Err(e)) => Err(e.into()),
            Err(e) => Err(e.into()),
        }
    }
}

#[derive(Serialize, Deserialize, QueryableByName, Clone, Debug)]
pub struct Interval {
    #[sql_type = "Float"]
    pub cpu: f32,
    #[sql_type = "Integer"]
    pub peers: i32,
    #[sql_type = "Integer"]
    pub height: i32,
    #[sql_type = "Integer"]
    pub memory: i32,
    #[sql_type = "Integer"]
    pub txcount: i32,
    #[sql_type = "Integer"]
    pub finalized_height: i32,
    #[sql_type = "Integer"]
    pub bandwidth_upload: i32,
    #[sql_type = "Integer"]
    pub bandwidth_download: i32,
    #[sql_type = "Integer"]
    pub disk_read_per_sec: i32,
    #[sql_type = "Integer"]
    pub disk_write_per_sec: i32,
    #[sql_type = "Integer"]
    pub used_db_cache_size: i32,
    #[sql_type = "Integer"]
    pub used_state_cache_size: i32,
    #[sql_type = "Text"]
    pub finalized_hash: String,
    #[sql_type = "Text"]
    pub best: String,
    #[sql_type = "Timestamp"]
    pub created_at: NaiveDateTime,
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
