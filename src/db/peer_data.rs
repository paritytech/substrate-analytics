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
use serde_json::Value;
use std::collections::HashSet;
use std::convert::TryInto;
use std::hash::{Hash, Hasher};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use super::{filters::Filters, DbExecutor, RECORD_LIMIT};
//use futures::{FutureExt, TryFutureExt};

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

#[derive(Debug)]
pub struct PeerMessages(pub HashSet<PeerMessage>);

#[derive(Clone, Debug)]
pub struct PeerMessageStartTime {
    pub peer_message: PeerMessage,
    pub last_accessed: NaiveDateTime,
}

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
                    info!("Last accessed: {}", &p.last_accessed);
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
        let pd_res = self.get_logs(filters);
        debug!("pd_res: {:?}", pd_res);
        if let Ok(pdr) = pd_res {
            // send to cache
            let req = self.cache.do_send(pdr);
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
        info!("Query start time: {}", &start_time);
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
