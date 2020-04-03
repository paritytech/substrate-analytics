use crate::cache::{Cache, Interest, Subscription};
use crate::db::peer_data::{PeerDataArray, PeerMessage, SubstrateLog};
use crate::web::metrics::Metrics;
use actix::prelude::*;
use actix_web::{web, web::Data, Error, HttpRequest, HttpResponse};
use actix_web_actors::ws;
use chrono::NaiveDateTime;
use serde_json::Value;
use std::collections::{HashMap, VecDeque};
use std::convert::TryFrom;
use std::convert::TryInto;
use std::time::{Duration, Instant};

const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(5);
const CLIENT_TIMEOUT_S: Duration = Duration::from_secs(60);

pub fn configure(cfg: &mut actix_web::web::ServiceConfig) {
    cfg.service(actix_web::web::scope("/feed").route("", actix_web::web::get().to(ws_index)));
}

async fn ws_index(
    r: HttpRequest,
    stream: web::Payload,
    cache: Data<Addr<Cache>>,
    metrics: actix_web::web::Data<Metrics>,
) -> Result<HttpResponse, Error> {
    ws::start(WebSocket::new(cache, metrics), &r, stream)
}

struct WebSocket {
    hb: Instant,
    cache: Data<Addr<Cache>>,
    metrics: actix_web::web::Data<Metrics>,
    aggregate_subscriptions: HashMap<PeerMessage, AggregateSubscription>,
}

impl Drop for WebSocket {
    fn drop(&mut self) {
        self.metrics.dec_concurrent_feed_count();
    }
    //    info("Dropped client feed connection, mailbox backlog = {}", );
}

impl Handler<PeerDataArray> for WebSocket {
    type Result = Result<(), &'static str>;

    fn handle(&mut self, msg: PeerDataArray, ctx: &mut Self::Context) -> Self::Result {
        match self.aggregate_subscriptions.get_mut(&msg.peer_message) {
            Some(subs) => {
                subs.aggregate_remainder.append(&mut msg.data.into());
                let mut aggregates: Vec<(NaiveDateTime, HashMap<(String, String), Vec<i64>>)> =
                    Vec::new();
                let mut accum: HashMap<(String, String), Vec<i64>> = HashMap::new();
                let mut last_index = 0usize;
                let mut start_ts = subs
                    .aggregate_remainder
                    .get(0)
                    .expect(
                        "Shouldn't be empty because we don't send to Subscribers if array is empty",
                    )
                    .created_at;
                debug!("Aggregate interval start_ts initialized to: {:?}", start_ts);
                for (idx, log) in subs.aggregate_remainder.iter().enumerate() {
                    if log.created_at.timestamp()
                        > (start_ts
                            .checked_add_signed(
                                chrono::Duration::from_std(subs.aggregate.update_interval)
                                    .expect("Shouldn't be out of range"),
                            )
                            .expect("Shouldn't overflow"))
                        .timestamp()
                    {
                        // Increment time, store accum in aggregates, store index we last used
                        let interval_dur =
                            chrono::Duration::from_std(subs.aggregate.update_interval)
                                .expect("Shouldn't be out of range");
                        while start_ts
                            .checked_add_signed(interval_dur)
                            .expect("Shouldn't overflow")
                            < log.created_at
                        {
                            start_ts = start_ts
                                .checked_add_signed(interval_dur)
                                .expect("Shouldn't overflow");
                        }
                        debug!(
                            "Aggregate interval start_ts incremented to = {:?}",
                            start_ts
                        );
                        aggregates.push((start_ts, std::mem::replace(&mut accum, HashMap::new())));
                        last_index = idx;
                    } else {
                        let l_time = log.log["time"].as_str();
                        let l_target = log.log["target"].as_str();
                        let l_name = log.log["name"].as_str();
                        if let (Some(Some(time)), Some(target), Some(name)) =
                            (l_time.map(|x| x.parse::<i64>().ok()), l_target, l_name)
                        {
                            accum
                                .entry((target.to_string(), name.to_string()))
                                .or_insert(Vec::new())
                                .push(time);
                        } else {
                            debug!(
                                "Unable to parse `time`, `target` or `name` from log: {:?}",
                                log.log
                            )
                        }
                    }
                }
                // remove measurements that have been aggregated
                subs.aggregate_remainder = subs.aggregate_remainder.split_off(last_index);
                let mut results = Vec::new();
                // Go through each aggregate interval
                for (ndt, aggregate_map) in aggregates {
                    // Go through each target/name partitioned array of measurements for this interval
                    trace!(
                        "Aggregate time: {:?}, aggregate map len = {}",
                        ndt,
                        aggregate_map.len()
                    );
                    for ((target, name), mut measurements) in aggregate_map {
                        trace!(
                            "In target: {} name: {} - measurements.len = {}",
                            target,
                            name,
                            measurements.len()
                        );
                        let time: i64;
                        match subs.aggregate.aggregate_type {
                            AggregateType::Mean => {
                                time = measurements.iter().sum::<i64>() / measurements.len() as i64
                            }
                            AggregateType::Median => {
                                measurements.sort();
                                time = measurements[(measurements.len() as f64 * 0.5) as usize]
                            }
                            AggregateType::Min => {
                                time = measurements
                                    .iter()
                                    .min()
                                    .expect("Iterator should not be empty")
                                    .to_owned()
                            }
                            AggregateType::Max => {
                                time = measurements
                                    .iter()
                                    .max()
                                    .expect("Iterator should not be empty")
                                    .to_owned()
                            }
                            AggregateType::Percentile90 => {
                                measurements.sort();
                                time = measurements[(measurements.len() as f64 * 0.9) as usize]
                            }
                        }
                        let agm = AggregateMeasurement {
                            time: time.clone(),
                            name: name.clone(),
                            target: target.clone(),
                            values: "".to_string(),
                            created_at: ndt.clone(),
                        };
                        results.push(agm);
                    }
                }
                if results.is_empty() {
                    return Ok(());
                }
                let message = AggregateDataMessage {
                    peer_message: PeerMessage {
                        peer_id: msg.peer_message.peer_id,
                        msg: msg.peer_message.msg,
                    },
                    data: results,
                };
                ctx.text(json!(message).to_string())
            }
            None => {
                trace!("No match for aggregate subscription in PeerDataResponse");
                ctx.text(json!(msg).to_string())
            }
        }
        Ok(())
    }
}

#[derive(Serialize, Debug)]
struct AggregateMeasurement {
    time: i64,
    name: String,
    target: String,
    values: String,
    created_at: NaiveDateTime,
}

#[derive(Serialize, Debug)]
struct AggregateDataMessage {
    peer_message: PeerMessage,
    data: Vec<AggregateMeasurement>,
}

impl Actor for WebSocket {
    type Context = ws::WebsocketContext<Self>;

    // Start heartbeat and updates on new connection
    fn started(&mut self, ctx: &mut Self::Context) {
        // Ensure we can keep sufficient backlog to survive a temp disconnect
        ctx.set_mailbox_capacity(64);
        self.hb(ctx);
        self.metrics.inc_concurrent_feed_count();
    }
}

/// Handler for `ws::Message`
impl StreamHandler<Result<ws::Message, ws::ProtocolError>> for WebSocket {
    fn handle(&mut self, msg: Result<ws::Message, ws::ProtocolError>, ctx: &mut Self::Context) {
        match msg {
            Ok(ws::Message::Ping(msg)) => {
                self.hb = Instant::now();
                ctx.pong(&msg);
            }
            Ok(ws::Message::Pong(_)) => {
                self.hb = Instant::now();
            }
            Ok(ws::Message::Text(text)) => {
                if let Err(e) = self.process_message(text, ctx) {
                    trace!("Unable to decode message: {}", e);
                    ctx.text(json!({ "error": e }).to_string());
                }
            }
            Ok(ws::Message::Binary(_bin)) => (),
            Ok(ws::Message::Close(_)) => {
                ctx.stop();
            }
            _ => ctx.stop(),
        }
    }
}

impl WebSocket {
    fn new(cache: Data<Addr<Cache>>, metrics: actix_web::web::Data<Metrics>) -> Self {
        Self {
            hb: Instant::now(),
            cache,
            metrics,
            aggregate_subscriptions: HashMap::new(),
        }
    }

    fn process_message(
        &mut self,
        text: String,
        ctx: &mut <Self as Actor>::Context,
    ) -> Result<(), &'static str> {
        if let Ok(j) = serde_json::from_str::<Value>(&text) {
            let peer_id: String = j["peer_id"]
                .as_str()
                .ok_or("`peer_id` not found")?
                .to_owned();
            let msg = j["msg"].as_str().ok_or("`msg` not found")?.to_owned();
            let mut start_time: Option<NaiveDateTime> = None;
            let interest = match j["interest"].as_str().ok_or("`interest` not found")? {
                "subscribe" => {
                    start_time = Some(
                        j["start_time"]
                            .as_str()
                            .ok_or("`start_time` not found")?
                            .parse::<NaiveDateTime>()
                            .map_err(|_| "unable to parse `start_time`")?,
                    );
                    Interest::Subscribe
                }
                "unsubscribe" => Interest::Unsubscribe,
                _ => return Err("`interest` must be either `subscribe` or `unsubscribe`"),
            };
            let subscription = Subscription {
                peer_id,
                msg,
                subscriber_addr: ctx.address().recipient(),
                start_time,
                interest,
            };

            self.handle_aggregate_subscription(&subscription, j)?;

            match self.cache.try_send(subscription) {
                Ok(_) => debug!("Sent subscription"),
                Err(e) => {
                    error!("Could not send subscription due to: {:?}", e);
                    return Err("Internal server error");
                }
            }
        }
        Ok(())
    }

    fn handle_aggregate_subscription(
        &mut self,
        subscription: &Subscription,
        json: Value,
    ) -> Result<(), &'static str> {
        let peer_message = PeerMessage {
            peer_id: subscription.peer_id.to_owned(),
            msg: subscription.msg.to_owned(),
        };
        if subscription.interest == Interest::Unsubscribe {
            self.aggregate_subscriptions.remove(&peer_message);
            return Ok(());
        }
        if json.get("aggregate_type").is_none() {
            return Ok(());
        }
        let aggregate_type = json["aggregate_type"].as_str();
        if let Some(a_type) = aggregate_type {
            if a_type.len() == 0 {
                return Ok(());
            }
            let aggregate_interval = json["aggregate_interval"].as_u64();
            if let Some(a_interval) = aggregate_interval {
                let aggregate_type: AggregateType = a_type.try_into()?;
                let aggregate = Aggregate {
                    aggregate_type,
                    key: "time".to_string(),
                    update_interval: Duration::from_secs(a_interval),
                };
                let aggregate_subscription = AggregateSubscription {
                    subscription: subscription.to_owned(),
                    aggregate,
                    aggregate_remainder: Default::default(),
                };
                if let Some(mut existing_sub) = self
                    .aggregate_subscriptions
                    .insert(peer_message, aggregate_subscription)
                {
                    existing_sub.subscription.interest = Interest::Unsubscribe;
                    match self.cache.try_send(existing_sub.subscription) {
                        Ok(_) => debug!("Sent unsubscribe for existing aggregate subscription"),
                        Err(e) => {
                            error!("Could not send unsubscribe due to: {:?}", e);
                            return Err("Internal server error");
                        }
                    }
                }
            } else {
                return Err("Unable to parse `aggregate_type` or `aggregate_interval`");
            }
        }
        Ok(())
    }

    fn hb(&self, ctx: &mut <Self as Actor>::Context) {
        ctx.run_interval(HEARTBEAT_INTERVAL, |act, ctx| {
            if Instant::now().duration_since(act.hb) > CLIENT_TIMEOUT_S {
                info!("Websocket Client heartbeat failed, disconnecting!");
                ctx.stop();
                return;
            }
            ctx.ping(b"");
        });
    }
}

#[derive(Hash, Debug, Clone)]
pub enum AggregateType {
    Mean,
    Median,
    Min,
    Max,
    Percentile90,
}

impl TryFrom<&str> for AggregateType {
    type Error = &'static str;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value.to_lowercase().as_str() {
            "mean" => Ok(AggregateType::Mean),
            "median" => Ok(AggregateType::Median),
            "min" => Ok(AggregateType::Min),
            "max" => Ok(AggregateType::Max),
            "percentile90" => Ok(AggregateType::Percentile90),
            _ => Err("Unable to parse AggregateType"),
        }
    }
}

#[derive(Hash, Debug, Clone)]
pub struct Aggregate {
    pub aggregate_type: AggregateType,
    pub key: String,
    pub update_interval: Duration,
}

#[derive(Debug)]
struct AggregateSubscription {
    subscription: Subscription,
    aggregate: Aggregate,
    aggregate_remainder: VecDeque<SubstrateLog>,
}
