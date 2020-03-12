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

use crate::db;
use crate::db::{
    filters::Filters,
    peer_data::{
        create_date_time, PeerDataResponse, PeerMessage, PeerMessageStartTime,
        PeerMessageStartTimeList, PeerMessageStartTimeRequest, PeerMessages, SubscriberInfo,
        SubstrateLog,
    },
    DbExecutor,
};
use crate::CACHE_UPDATE_TIMEOUT;
use actix::prelude::*;
use chrono::{NaiveDateTime, Utc};
use diesel::sql_types::*;
use diesel::{result::QueryResult, sql_query, RunQueryDsl};
use failure::Error;
use futures::future::join_all;
use futures::FutureExt;
use serde_json::Value;
use slice_deque::SliceDeque;
use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

#[derive(Debug)]
pub struct PeerMessageCache {
    pub deque: SliceDeque<SubstrateLog>,
    pub last_updated: NaiveDateTime,
    pub started_update: Option<Instant>,
}

impl Cache {
    pub fn updates_in_progress(&self) -> Vec<(PeerMessage, Instant)> {
        let ret = self
            .cache
            .iter()
            .filter_map(|(pm, pmc)| {
                if let Some(instant) = pmc.started_update {
                    Some((pm.to_owned(), instant.clone()))
                } else {
                    None
                }
            })
            .collect();
        ret
    }

    pub fn initialise_update(&mut self) -> Vec<PeerMessageStartTime> {
        let now = Instant::now();
        let ret = self
            .cache
            .iter_mut()
            .filter_map(|(pm, pmc)| {
                if let None = pmc.started_update {
                    pmc.started_update = Some(now.clone());
                    Some(PeerMessageStartTime {
                        peer_message: pm.to_owned(),
                        last_accessed: pmc.last_updated.to_owned(),
                    })
                } else {
                    None
                }
            })
            .collect();
        ret
    }
}

/// Cache is responsible for:
///
/// - Storing `PeerData` in memory, partitioned by `peer_id` and then `msg`
///
/// - Storing subscribers and pushing relevant data to them as it becomes available
///
/// - Receiving updates to the cache
///
/// - Cleaning out data when it's older than `LOG_EXPIRY_HOURS`
pub struct Cache {
    cache: HashMap<PeerMessage, PeerMessageCache>,
    subscribers: HashMap<Recipient<PeerDataResponse>, SubscriberInfo>,
    refresh_interval: Duration,
    db_arbiter: Addr<DbExecutor>,
}

impl Cache {
    pub fn new(refresh_interval: Duration, db_arbiter: Addr<DbExecutor>) -> Cache {
        Cache {
            cache: HashMap::new(),
            subscribers: HashMap::new(),
            refresh_interval,
            db_arbiter,
        }
    }
}

impl Actor for Cache {
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut Context<Self>) {
        ctx.run_interval(self.refresh_interval, |act, ctx| {
            act.request_updates(ctx);
        });
    }
}

impl Cache {
    fn request_updates(&mut self, ctx: &mut Context<Self>) {
        if self.subscribers.is_empty() {
            return;
        }
        let now = Instant::now();
        for (peer_message, started) in self.updates_in_progress() {
            let dur = now - started;
            if dur > *CACHE_UPDATE_TIMEOUT {
                warn!(
                    "Cache update timeout exceeded (running: {} s) for {} {}",
                    dur.as_secs_f32(),
                    peer_message.peer_id,
                    peer_message.msg
                );
                if dur > *CACHE_UPDATE_TIMEOUT * 4 {
                    warn!(
                        "Cache update timeout exceeded (running: {} s) for {} {}, dropping update...",
                        dur.as_secs_f32(),
                        peer_message.peer_id,
                        peer_message.msg
                    );
                    if let Some(c) = self.cache.get_mut(&peer_message) {
                        c.started_update.take();
                    }
                }
            }
        }
        let pmsts = self.initialise_update();
        if pmsts.is_empty() {
            return;
        }
        let pmstl = PeerMessageStartTimeList {
            list: pmsts,
            cache: ctx.address().recipient::<PeerDataResponse>(),
        };
        let fut = self.db_arbiter.send(pmstl).map(|r| match r {
            Err(e) => error!(
                "Unable to send PeerMessageStartTimeList to DbExecutor : {:?}",
                e
            ),
            _ => (),
        });
        ctx.wait(actix::fut::wrap_future::<_, Self>(fut));
    }
}

impl Handler<PeerDataResponse> for Cache {
    type Result = Result<(), &'static str>;

    fn handle(&mut self, msg: PeerDataResponse, _ctx: &mut Self::Context) -> Self::Result {
        self.process_peer_data_response(msg);
        Ok(())
    }
}

impl Cache {
    fn process_peer_data_response(&mut self, msg: PeerDataResponse) {
        let updates_in_progress = self.updates_in_progress();
        // Check that we are expecting this update
        if updates_in_progress
            .iter()
            .find(|(pm, _)| {
                let same = pm == &msg.peer_message;
                same
            })
            .is_none()
        {
            warn!("Received PeerDataResponse for PeerMessage we no longer have a cache for, or updated already.");
            return;
        }
        // Flag this cache as no longer expecting update
        let started_update = match self.cache.get_mut(&msg.peer_message) {
            Some(p) => p.started_update.take(),
            _ => None,
        };
        // Make sure data is not empty
        let len = msg.data.len();
        debug!("PeerDataResponse {} length = {}", msg.peer_message, len);
        if len == 0 {
            return;
        }
        // Update cache
        let mut pd = self.cache.get_mut(&msg.peer_message);
        let mut peer_data = self
            .cache
            .get_mut(&msg.peer_message)
            .expect("Already checked in updates_in_progress()");
        let peer_message = msg.peer_message;
        let mut vpd = msg.data;
        // TODO Is there any possibility that we append duplicate data?
        peer_data.deque.append(&mut vpd[..].into());
        // Iterate through subscribers and send latest data based on their last_updated time
        for (recipient, mut subscriber_info) in self.subscribers.iter_mut().filter(|(_, s)| {
            s.peer_messages
                .0
                .iter()
                .find(|x| x == &&peer_message)
                .is_some()
        }) {
            //
            debug!("SubscriberInfo: {:?}", &subscriber_info);
            let idx = match peer_data
                .deque
                .binary_search_by(|item| item.created_at.cmp(&subscriber_info.last_updated))
            {
                Ok(n) => n + 1,
                _ => 0,
            };
            dbg!(&idx);
            //            if idx == 0 {
            //                dbg!(&subscriber_info.last_updated);
            //                dbg!(&peer_data.deque);
            //            }
            let response_data = peer_data.deque[idx..].to_vec();
            let pdr = PeerDataResponse {
                peer_message: peer_message.clone(),
                data: response_data,
            };
            if let Err(e) = recipient.try_send(pdr) {
                error!("Unable to send PeerDataResponse: {:?}", e);
            } else {
                peer_data.last_updated = peer_data.deque.last().unwrap().created_at.to_owned();
                subscriber_info.last_updated = peer_data.last_updated.clone();
                trace!(
                    "SubscriberInfo for ({:?}) - {:?} last updated: {}",
                    recipient,
                    subscriber_info.peer_messages,
                    subscriber_info.last_updated,
                );
            }
        }
        // TODO refactor this
        match started_update {
            Some(s) => {
                let dur = Instant::now() - s;
                debug!(
                    "Cache update cycle for {} took: {} seconds",
                    peer_message,
                    dur.as_secs_f32()
                );
            }
            _ => warn!("Should be unreachable"),
        }
    }
}

// TODO
impl Message for PeerMessages {
    type Result = ();
}

#[derive(Debug)]
pub enum Interest {
    Subscribe,
    Unsubscribe,
}

#[derive(Debug)]
pub struct Subscription {
    pub peer_id: String,
    pub msg: String,
    pub subscriber_addr: Recipient<PeerDataResponse>,
    pub start_time: Option<NaiveDateTime>,
    pub interest: Interest,
}

impl Message for Subscription {
    type Result = Result<(), ()>;
}

impl Handler<Subscription> for Cache {
    type Result = Result<(), ()>;

    fn handle(&mut self, msg: Subscription, ctx: &mut Self::Context) -> Self::Result {
        info!("Received subscription: {:?}", &msg);
        match &msg.interest {
            Interest::Subscribe => {
                self.subscribe(msg.peer_id, msg.msg, msg.subscriber_addr, msg.start_time)
            }
            Interest::Unsubscribe => self.unsubscribe(msg.peer_id, msg.msg, msg.subscriber_addr),
        }

        Ok(())
    }
}

impl Cache {
    /// Subscribe to `Log`s for the given `peer_id` and `msg`
    /// Optionally specify start_time which defaults to `LOG_EXPIRY_HOURS`
    pub fn subscribe(
        &mut self,
        peer_id: String,
        msg: String,
        subscriber_addr: Recipient<PeerDataResponse>,
        start_time: Option<NaiveDateTime>,
    ) {
        let mut subscriber = self
            .subscribers
            .entry(subscriber_addr)
            .or_insert(SubscriberInfo::new());
        let peer_message = PeerMessage {
            peer_id: peer_id.clone(),
            msg: msg.to_owned(),
        };
        subscriber.peer_messages.0.insert(peer_message.clone());
        self.subscribe_msgs(peer_message, start_time);
    }

    pub fn unsubscribe(
        &mut self,
        peer_id: String,
        msg: String,
        subscriber_addr: Recipient<PeerDataResponse>,
    ) {
        let mut subscriber = self
            .subscribers
            .entry(subscriber_addr)
            .or_insert(SubscriberInfo::new());
        let peer_message = PeerMessage {
            peer_id: peer_id.clone(),
            msg: msg.to_owned(),
        };
        subscriber.peer_messages.0.remove(&peer_message);
    }

    fn subscribe_msgs(&mut self, peer_message: PeerMessage, start_time: Option<NaiveDateTime>) {
        let last_updated = start_time.unwrap_or(create_date_time(0));
        // Set last accessed now, otherwise could be missed if subscriber is closed before accessed
        self.cache.entry(peer_message).or_insert(PeerMessageCache {
            deque: SliceDeque::new(),
            last_updated,
            started_update: None,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::test;
    use dotenv::dotenv;
    use std::env;

    fn get_test_setup() -> Cache {
        let pool = db::create_pool();
        let db_arbiter = SyncArbiter::start(1, move || db::DbExecutor::new(pool.clone()));
        Cache::new(Duration::from_secs(2), db_arbiter.clone())
    }

    fn add_cache_entries(cache: &mut Cache) {
        let k = PeerMessage {
            peer_id: "Peer 1".to_string(),
            msg: "Message 1".to_string(),
        };
        let v = PeerMessageCache {
            deque: SliceDeque::new(),
            last_updated: create_date_time(0),
            started_update: None,
        };
        cache.cache.insert(k, v);
        let k = PeerMessage {
            peer_id: "Peer 2".to_string(),
            msg: "Message 2".to_string(),
        };
        let v = PeerMessageCache {
            deque: SliceDeque::new(),
            last_updated: create_date_time(0),
            started_update: None,
        };
        cache.cache.insert(k, v);
    }

    fn generate_peer_Data_response1() -> PeerDataResponse {
        PeerDataResponse {
            peer_message: PeerMessage {
                peer_id: "Peer 1".to_string(),
                msg: "Message 1".to_string(),
            },
            data: vec![SubstrateLog {
                log: Default::default(),
                created_at: create_date_time(10),
            }],
        }
    }

    fn generate_peer_Data_response2() -> PeerDataResponse {
        PeerDataResponse {
            peer_message: PeerMessage {
                peer_id: "Peer 2".to_string(),
                msg: "Message 2".to_string(),
            },
            data: vec![SubstrateLog {
                log: Default::default(),
                created_at: create_date_time(10),
            }],
        }
    }

    #[actix_rt::test]
    async fn initialise_update_test() {
        dotenv().ok();
        let mut cache = get_test_setup();
        add_cache_entries(&mut cache);
        assert_eq!(cache.cache.len(), 2);
        for (pm, pmc) in &cache.cache {
            assert!(pmc.started_update.is_none());
        }
        let n_updating = cache.initialise_update().len();
        assert_eq!(n_updating, 2);
        for (pm, pmc) in &cache.cache {
            assert!(pmc.started_update.is_some());
        }
    }

    #[actix_rt::test]
    async fn updates_in_progress_test() {
        dotenv().ok();
        let mut cache = get_test_setup();
        add_cache_entries(&mut cache);
        cache.initialise_update();
        let updates = cache.updates_in_progress();
        assert_eq!(updates.len(), 2);
        for (pm, pmc) in &cache.cache {
            assert!(pmc.started_update.is_some());
        }
    }

    #[actix_rt::test]
    async fn process_peer_data_response_test() {
        dotenv().ok();
        let mut cache = get_test_setup();
        add_cache_entries(&mut cache);
        cache.initialise_update();
        assert_eq!(cache.updates_in_progress().len(), 2);
        cache.process_peer_data_response(generate_peer_Data_response1());
        assert_eq!(cache.updates_in_progress().len(), 1);
        cache.process_peer_data_response(generate_peer_Data_response1());
        assert_eq!(cache.updates_in_progress().len(), 1);
        cache.process_peer_data_response(generate_peer_Data_response2());
        assert_eq!(cache.updates_in_progress().len(), 0);
    }

    #[actix_rt::test]
    async fn cache_purges_expired_data_test() {
        // TODO
    }
}
