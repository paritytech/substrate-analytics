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
use crate::db::peer_data::CreatedAt;
use crate::db::{
    filters::Filters,
    peer_data::{
        create_date_time, PeerData, PeerDataResponse, PeerMessage, PeerMessageStartTime,
        PeerMessageStartTimeList, PeerMessageStartTimeRequest, PeerMessages, SubscriberInfo,
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
    cache: HashMap<PeerMessageStartTime, SliceDeque<PeerData>>,
    subscribers: HashMap<Recipient<PeerDataResponse>, SubscriberInfo>,
}

impl Actor for Cache {
    type Context = Context<Self>;
}

impl Handler<PeerDataResponse> for Cache {
    type Result = Result<(), &'static str>;

    fn handle(&mut self, msg: PeerDataResponse, ctx: &mut Self::Context) -> Self::Result {
        info!("Data length = {:?}", msg.data.len());
        // Update cache
        if let Some(element) = msg.data.get(0) {
            let msg_type = match element {
                PeerData::Interval(_) => "system.interval",
                PeerData::Profiling(_) => "tracing.profiling",
            };
            drop(element);
            let pmst = PeerMessageStartTime {
                peer_message: PeerMessage {
                    peer_id: msg.peer_id,
                    msg: msg_type.to_owned(),
                },
                last_accessed: create_date_time(0),
            };
            let mut pd = self.cache.get_mut(&pmst);
            let mut peer_data: &mut SliceDeque<PeerData>;
            match pd.is_some() {
                true => peer_data = pd.unwrap(),
                false => {
                    warn!("Received PeerDataResponse for PeerMessage we no longer have a cache for, discarding...");
                    return Ok(());
                }
            }
            let mut vpd = msg.data;
            peer_data.append(&mut vpd[..].into());
            // Iterate through subscribers and send latest data based on their last_updated time
            for (recipient, mut subscriber_info) in self.subscribers.iter_mut().filter(|(r, s)| {
                s.peer_messages
                    .0
                    .iter()
                    .find(|x| {
                        info!(
                            "x == &&pmst.peer_message: {:?} == {:?}",
                            x, &&pmst.peer_message
                        );
                        x == &&pmst.peer_message
                    })
                    .is_some()
            }) {
                //
                info!("SubscriberInfo: {:?}", &subscriber_info);
                let idx = match peer_data
                    .binary_search_by(|item| item.created_at().cmp(&subscriber_info.last_updated))
                {
                    Ok(n) => n,
                    _ => 0,
                };
                let response_data = peer_data[idx..].to_vec();
                let pdr = PeerDataResponse {
                    peer_id: pmst.peer_message.peer_id.clone(),
                    data: response_data,
                };
                if let Err(e) = recipient.try_send(pdr) {
                    error!("Unable to send PeerDataResponse: {:?}", e);
                } else {
                    subscriber_info.last_updated =
                        peer_data.last().unwrap().created_at().to_owned();
                }
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
        let keys: Vec<PeerMessageStartTime> = self.cache.keys().map(|k| k.to_owned()).collect();
        let pmstl = PeerMessageStartTimeList(keys);
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
        let pmc = PeerMessageStartTime {
            peer_message,
            last_accessed: start_time.unwrap_or(create_date_time(0)),
        };
        debug!("Adding cache: {:?}", &pmc);
        // Set last accessed now, otherwise could be missed if subscriber is closed before accessed
        let mut peer_cache = self.cache.entry(pmc).or_insert(SliceDeque::new());
    }
}
