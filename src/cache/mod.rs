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
};
use actix::prelude::*;
use chrono::{NaiveDateTime, Utc};
use diesel::sql_types::*;
use diesel::{result::QueryResult, sql_query, RunQueryDsl};
use failure::Error;
use futures::future::join_all;
use serde_json::Value;
use slice_deque::SliceDeque;
use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub struct PeerMessageCache {
    pub deque: SliceDeque<SubstrateLog>,
    pub last_updated: NaiveDateTime,
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
}

impl Actor for Cache {
    type Context = Context<Self>;
}

impl Handler<PeerDataResponse> for Cache {
    type Result = Result<(), &'static str>;

    fn handle(&mut self, msg: PeerDataResponse, ctx: &mut Self::Context) -> Self::Result {
        let len = msg.data.len();
        info!("Data length = {:?}", len);
        if len == 0 {
            return Ok(());
        }

        // Update cache

        let mut pd = self.cache.get_mut(&msg.peer_message);
        let mut peer_data: &mut PeerMessageCache;
        match pd.is_some() {
            true => peer_data = pd.unwrap(),
            false => {
                warn!("Received PeerDataResponse for PeerMessage we no longer have a cache for, discarding...");
                return Ok(());
            }
        }
        let peer_message = msg.peer_message;
        let mut vpd = msg.data;
        // Is there any possibility that we append duplicate data?
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
            info!("SubscriberInfo: {:?}", &subscriber_info);
            let idx = match peer_data
                .deque
                .binary_search_by(|item| item.created_at.cmp(&subscriber_info.last_updated))
            {
                Ok(n) => n + 1,
                _ => 0,
            };
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
                info!(
                    "SubscriberInfo last updated: {}",
                    subscriber_info.last_updated
                );
            }
        }

        Ok(())
    }
}

impl Handler<PeerMessageStartTimeRequest> for Cache {
    type Result = Result<PeerMessageStartTimeList, &'static str>;

    fn handle(
        &mut self,
        msg: PeerMessageStartTimeRequest,
        ctx: &mut Self::Context,
    ) -> Self::Result {
        //        let keys: Vec<PeerMessageStartTime> = self.cache.keys().map(|k| k.to_owned()).collect();
        let pmsts: Vec<PeerMessageStartTime> = self
            .cache
            .iter()
            .map(|(pm, pmc)| PeerMessageStartTime {
                peer_message: pm.to_owned(),
                last_accessed: pmc.last_updated.to_owned(),
            })
            .collect();

        let pmstl = PeerMessageStartTimeList(pmsts);
        debug!(
            "Received PeerMessageStartTimeRequest, returning PeerMessageStartTimeList: {:?}",
            &pmstl
        );
        Ok(pmstl)
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
    pub fn new() -> Cache {
        Cache {
            cache: HashMap::new(),
            subscribers: HashMap::new(),
        }
    }

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
        });
    }
}
