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

use crate::db::{
    peer_data::{
        time_secs_ago, PeerDataArray, PeerMessage, PeerMessageTime, PeerMessageTimeList,
        PeerMessages, SubstrateLog,
    },
    DbExecutor,
};
use crate::{
    CACHE_EXPIRY_S, CACHE_TIMEOUT_S, CACHE_UPDATE_INTERVAL_MS, CACHE_UPDATE_TIMEOUT_S,
    PURGE_INTERVAL_S,
};
use actix::prelude::*;
use chrono::NaiveDateTime;
use failure::_core::time::Duration;
use futures::FutureExt;
use slice_deque::SliceDeque;
use std::collections::HashMap;
use std::time::Instant;

#[derive(Debug)]
pub struct PeerMessageCache {
    pub deque: SliceDeque<SubstrateLog>,
    pub last_updated: NaiveDateTime,
    pub started_update: Option<Instant>,
    pub last_used: Instant,
}

impl Cache {
    fn updates_in_progress(&self) -> Vec<(PeerMessage, Instant)> {
        self.cache
            .iter()
            .filter_map(|(pm, pmc)| {
                if let Some(instant) = pmc.started_update {
                    Some((pm.to_owned(), instant.clone()))
                } else {
                    None
                }
            })
            .collect()
    }

    fn initialise_update(&mut self) -> Vec<PeerMessageTime> {
        let now = Instant::now();
        self.cache
            .retain(|_, pmc| pmc.last_used + Duration::from_secs(*CACHE_TIMEOUT_S) > now);
        self.cache
            .iter_mut()
            .filter_map(|(pm, pmc)| {
                if let None = pmc.started_update {
                    pmc.started_update = Some(now.clone());
                    Some(PeerMessageTime {
                        peer_message: pm.to_owned(),
                        time: pmc.last_updated.to_owned(),
                    })
                } else {
                    None
                }
            })
            .collect()
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
/// - Cleaning out data when it's older than `CACHE_EXPIRY_S`
pub struct Cache {
    cache: HashMap<PeerMessage, PeerMessageCache>,
    subscribers: HashMap<Recipient<PeerDataArray>, PeerMessages>,
    db_arbiter: Addr<DbExecutor>,
}

impl Cache {
    pub fn new(db_arbiter: Addr<DbExecutor>) -> Cache {
        Cache {
            cache: HashMap::new(),
            subscribers: HashMap::new(),
            db_arbiter,
        }
    }
}

impl Actor for Cache {
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut Context<Self>) {
        // Start update cycle
        ctx.run_interval(*CACHE_UPDATE_INTERVAL_MS, |act, ctx| {
            act.request_updates(ctx);
        });
        // Start purge cycle
        ctx.run_interval(*PURGE_INTERVAL_S, |act, _ctx| {
            act.purge_expired();
        });
        // Start purge cycle
        ctx.run_interval(Duration::from_secs(60), |act, _ctx| {
            debug!("Cache subscribers: {}", act.subscribers.len());
            debug!("Cache len: {}", act.cache.len());
        });
    }
}

impl Cache {
    fn request_updates(&mut self, ctx: &mut Context<Self>) {
        let now = Instant::now();
        for (peer_message, started) in self.updates_in_progress() {
            let dur = now - started;
            if dur > *CACHE_UPDATE_TIMEOUT_S {
                warn!(
                    "Cache update timeout exceeded (running: {} s) for {} {}",
                    dur.as_secs_f32(),
                    peer_message.peer_id,
                    peer_message.msg
                );
                if dur > *CACHE_UPDATE_TIMEOUT_S * 4 {
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
        let pmstl = PeerMessageTimeList {
            list: pmsts,
            cache: ctx.address().recipient::<PeerDataArray>(),
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

    fn purge_expired(&mut self) {
        let expiry_time = time_secs_ago((*CACHE_EXPIRY_S).into());
        for (_, pmc) in &mut self.cache {
            if pmc.deque.is_empty() {
                continue;
            };
            let mut idx = 0;
            for (n, sl) in pmc.deque.iter().enumerate() {
                let ms_since = sl
                    .created_at
                    .signed_duration_since(expiry_time)
                    .num_milliseconds();
                if ms_since > 0 {
                    idx = n;
                    break;
                }
            }
            let new_len = pmc.deque.len() - idx;
            pmc.deque.truncate_front(new_len);
        }
    }
}

impl Handler<PeerDataArray> for Cache {
    type Result = Result<(), &'static str>;

    fn handle(&mut self, msg: PeerDataArray, _ctx: &mut Self::Context) -> Self::Result {
        self.process_peer_data_response(msg);
        Ok(())
    }
}

impl Cache {
    fn process_peer_data_response(&mut self, msg: PeerDataArray) {
        let updates_in_progress = self.updates_in_progress();
        // Check that we are expecting this update
        if updates_in_progress
            .iter()
            .find(|(pm, _)| pm == &msg.peer_message)
            .is_none()
        {
            warn!("Received PeerDataResponse for PeerMessage we no longer have a cache for, or updated already.");
            return;
        }
        // Take started_update to flag that this cache as no longer expecting update
        let started_update = self.take_started_update(&msg.peer_message);
        // Make sure data is not empty
        let len = msg.data.len();
        debug!("PeerDataResponse {} length = {}", msg.peer_message, len);
        if len == 0 {
            return;
        }
        // Update cache
        let mut peer_message_cache = self
            .cache
            .get_mut(&msg.peer_message)
            .expect("Already checked in updates_in_progress()");
        let peer_message = msg.peer_message.clone();
        let mut vpd = msg.data;
        // TODO Is there any possibility that we append duplicate data?
        peer_message_cache.deque.append(&mut vpd[..].into());
        // Probably unnecessary, TODO remove last_updated field, replace with method returning it
        peer_message_cache.last_updated = peer_message_cache
            .deque
            .last()
            .unwrap()
            .created_at
            .to_owned();
        // Iterate through subscribers and send latest data based on their last_updated time
        // Can be optimised for usual best case with fallback to binary_search
        let mut dead = Vec::new();
        let now = Instant::now();
        for (recipient, peer_messages) in self
            .subscribers
            .iter_mut()
            .filter(|(_, s)| s.0.iter().find(|(p, _)| p == &&peer_message).is_some())
        {
            //
            //            debug!("Subscriber PeerMessages: {:?}", &peer_messages);
            let pmt = peer_messages
                .0
                .get_mut(&msg.peer_message)
                .expect("Must not be modified anywhere else");
            let idx = match peer_message_cache
                .deque
                .binary_search_by(|item| item.created_at.cmp(&pmt))
            {
                Ok(n) => Some(n + 1),
                _ => None,
            };
            let idx = match idx {
                Some(n) => n,
                None => {
                    debug!("Finding closest time");
                    peer_message_cache
                        .deque
                        .iter_mut()
                        .enumerate()
                        .min_by(|a, b| {
                            let y =
                                a.1.created_at
                                    .signed_duration_since(*pmt)
                                    .num_milliseconds();
                            let z =
                                &b.1.created_at
                                    .signed_duration_since(*pmt)
                                    .num_milliseconds();
                            (y * y).cmp(&(z * z))
                        })
                        .expect("")
                        .0
                }
            };
            let response_data = peer_message_cache.deque[idx..].to_vec();
            let pdr = PeerDataArray {
                peer_message: peer_message.clone(),
                data: response_data,
            };
            if let Err(e) = recipient.try_send(pdr) {
                debug!("Unable to send PeerDataResponse: {:?}", e);
                dead.push(recipient.to_owned());
            } else {
                *pmt = peer_message_cache.last_updated.clone();
                peer_message_cache.last_used = now;
                trace!(
                    "SubscriberInfo for ({:?}) - {:?} last updated: {}",
                    recipient,
                    msg.peer_message,
                    pmt,
                );
            }
        }
        for r in dead {
            self.subscribers.remove(&r);
        }
        let dur = Instant::now() - started_update;
        debug!(
            "Cache update cycle for {} took: {} seconds",
            peer_message,
            dur.as_secs_f32()
        );
    }

    fn take_started_update(&mut self, peer_message: &PeerMessage) -> Instant {
        self.cache
            .get_mut(peer_message)
            .expect("Already checked that this cache is expecting update")
            .started_update
            .take()
            .expect("Already checked that this cache is expecting update")
    }
}

#[derive(Eq, PartialEq, Debug, Clone)]
pub enum Interest {
    Subscribe,
    Unsubscribe,
}

#[derive(Eq, PartialEq, Debug, Clone)]
pub struct Subscription {
    pub peer_id: String,
    pub msg: String,
    pub subscriber_addr: Recipient<PeerDataArray>,
    pub start_time: Option<NaiveDateTime>,
    pub interest: Interest,
}

impl Message for Subscription {
    type Result = Result<(), ()>;
}

impl Handler<Subscription> for Cache {
    type Result = Result<(), ()>;

    fn handle(&mut self, msg: Subscription, _ctx: &mut Self::Context) -> Self::Result {
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
    /// Optionally specify start_time which defaults to `CACHE_EXPIRY_S`
    pub fn subscribe(
        &mut self,
        peer_id: String,
        msg: String,
        subscriber_addr: Recipient<PeerDataArray>,
        start_time: Option<NaiveDateTime>,
    ) {
        let last_updated = start_time.unwrap_or(time_secs_ago(0));
        let peer_message = PeerMessage {
            peer_id: peer_id.clone(),
            msg: msg.to_owned(),
        };

        self.subscribe_msgs(peer_message.clone(), start_time);

        self.subscribers
            .entry(subscriber_addr)
            .or_insert(PeerMessages(HashMap::new()))
            .0
            .insert(peer_message, last_updated);
    }

    pub fn unsubscribe(
        &mut self,
        peer_id: String,
        msg: String,
        subscriber_addr: Recipient<PeerDataArray>,
    ) {
        if let Some(peer_messages) = self.subscribers.get_mut(&subscriber_addr) {
            let peer_message = PeerMessage {
                peer_id: peer_id.clone(),
                msg: msg.to_owned(),
            };
            peer_messages.0.remove(&peer_message);
        }
    }

    fn subscribe_msgs(&mut self, peer_message: PeerMessage, _start_time: Option<NaiveDateTime>) {
        // start_time currently not used, but possible to optimise memory use with it
        use std::collections::hash_map::Entry;
        let last_updated = time_secs_ago(*CACHE_EXPIRY_S);
        // Set last accessed now, otherwise could be missed if subscriber is closed before accessed
        match self.cache.entry(peer_message) {
            Entry::Occupied(mut _o) => {
                // temporarily noop, because we fix cache start time to CACHE_EXPIRY_S
            }
            Entry::Vacant(v) => {
                v.insert(PeerMessageCache {
                    deque: SliceDeque::new(),
                    last_updated,
                    started_update: None,
                    last_used: Instant::now(),
                });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::test;
    use dotenv::dotenv;
    use std::env;
    use std::sync::RwLock;

    fn get_test_setup() -> Cache {
        lazy_static! {
            pub static ref POOL: RwLock<diesel::r2d2::Pool<diesel::r2d2::ConnectionManager<diesel::PgConnection>>> =
                RwLock::new(crate::db::create_pool());
        }
        let pool = POOL.read().unwrap().to_owned();
        let db_arbiter = SyncArbiter::start(1, move || DbExecutor::new(pool.clone()));
        Cache::new(db_arbiter.clone())
    }

    fn add_cache_entries(cache: &mut Cache) {
        let k = PeerMessage {
            peer_id: "Peer 1".to_string(),
            msg: "Message 1".to_string(),
        };
        let v = PeerMessageCache {
            deque: SliceDeque::new(),
            last_updated: time_secs_ago(0),
            started_update: None,
            last_used: Instant::now(),
        };
        cache.cache.insert(k, v);
        let k = PeerMessage {
            peer_id: "Peer 2".to_string(),
            msg: "Message 2".to_string(),
        };
        let v = PeerMessageCache {
            deque: SliceDeque::new(),
            last_updated: time_secs_ago(0),
            started_update: None,
            last_used: Instant::now(),
        };
        cache.cache.insert(k, v);
    }

    fn generate_peer_data_response1() -> PeerDataArray {
        PeerDataArray {
            peer_message: PeerMessage {
                peer_id: "Peer 1".to_string(),
                msg: "Message 1".to_string(),
            },
            data: vec![SubstrateLog {
                log: Default::default(),
                created_at: time_secs_ago(10),
            }],
        }
    }

    fn generate_peer_data_response2() -> PeerDataArray {
        PeerDataArray {
            peer_message: PeerMessage {
                peer_id: "Peer 2".to_string(),
                msg: "Message 2".to_string(),
            },
            data: vec![SubstrateLog {
                log: Default::default(),
                created_at: time_secs_ago(10),
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
        for (_pm, pmc) in &cache.cache {
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
        for (_pm, pmc) in &cache.cache {
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
        cache.process_peer_data_response(generate_peer_data_response1());
        assert_eq!(cache.updates_in_progress().len(), 1);
        cache.process_peer_data_response(generate_peer_data_response1());
        assert_eq!(cache.updates_in_progress().len(), 1);
        cache.process_peer_data_response(generate_peer_data_response2());
        assert_eq!(cache.updates_in_progress().len(), 0);
    }

    #[actix_rt::test]
    async fn cache_purges_expired_data_test() {
        dotenv().ok();
        let mut cache = get_test_setup();
        add_cache_entries(&mut cache);
        let k = PeerMessage {
            peer_id: "Peer 1".to_string(),
            msg: "Message 1".to_string(),
        };
        let t1 = time_secs_ago(((*CACHE_EXPIRY_S) + 1).into());
        let t2 = time_secs_ago(((*CACHE_EXPIRY_S) - 1).into());
        let sl1 = SubstrateLog {
            log: Default::default(),
            created_at: t1,
        };
        let sl2 = SubstrateLog {
            log: Default::default(),
            created_at: t2.clone(),
        };
        let sl3 = SubstrateLog {
            log: Default::default(),
            created_at: t2,
        };
        cache
            .cache
            .get_mut(&k)
            .unwrap()
            .deque
            .append(&mut vec![sl1, sl2][..].into());
        cache
            .cache
            .get_mut(&k)
            .unwrap()
            .deque
            .append(&mut vec![sl3][..].into());
        assert_eq!(cache.cache.get(&k).unwrap().deque.len(), 3);
        cache.purge_expired();
        assert_eq!(cache.cache.get(&k).unwrap().deque.len(), 2);
    }
}
