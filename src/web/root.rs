use super::metrics::Metrics;
use crate::db::{
    models::{NewPeerConnection, NewSubstrateLog, PeerConnection},
    DbExecutor,
};
use crate::{CLIENT_TIMEOUT, HEARTBEAT_INTERVAL, WS_MAX_PAYLOAD};
use actix::prelude::*;
use actix_http::ws::Codec;
use actix_web::{error, web as a_web, Error, HttpRequest, HttpResponse};
use actix_web_actors::ws;
use chrono::DateTime;
use serde_json::Value;
use std::fmt;
use std::time::Instant;

#[derive(Default, Debug)]
struct MessageCount {
    ping: u64,
    pong: u64,
    text: u64,
    binary: u64,
}

impl fmt::Display for MessageCount {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "ping: {}, pong: {}, text: {}, binary {}",
            self.ping, self.pong, self.text, self.binary
        )
    }
}

struct NodeSocket {
    hb: Instant,
    ip: String,
    db: a_web::Data<Addr<DbExecutor>>,
    metrics: a_web::Data<Metrics>,
    peer_connection: PeerConnection,
    msg_count: MessageCount,
}

impl Drop for NodeSocket {
    fn drop(&mut self) {
        self.metrics.inc_ws_dropped_count();
        debug!("Dropped WS connection to ip: {}", self.ip);
    }
}

impl NodeSocket {
    fn new(
        ip: String,
        db: a_web::Data<Addr<DbExecutor>>,
        metrics: a_web::Data<Metrics>,
    ) -> Result<Self, String> {
        Ok(Self {
            peer_connection: Self::create_peer_connection(&db, &ip)?,
            ip,
            db,
            metrics,
            hb: Instant::now(),
            msg_count: MessageCount::default(),
        })
    }

    fn create_peer_connection(
        db: &a_web::Data<Addr<DbExecutor>>,
        ip: &str,
    ) -> Result<PeerConnection, String> {
        db.send(NewPeerConnection {
            ip_addr: String::from(ip), //
            peer_id: None,
        })
        .wait()
        .unwrap_or_else(|e| {
            error!("Failed to send NewPeerConnection to DB actor - {:?}", e);
            Err("Failed to send".to_string())
        })
    }

    fn hb(&self, ctx: &mut <Self as Actor>::Context) {
        let ip = self.ip.clone();
        ctx.run_interval(*HEARTBEAT_INTERVAL, move |act, ctx| {
            if Instant::now().duration_since(act.hb) > *CLIENT_TIMEOUT {
                info!("Websocket heartbeat failed for: {} - DISCONNECTING", ip);
                ctx.stop();
                return;
            }
            ctx.ping("");
        });
    }
}

impl Actor for NodeSocket {
    type Context = ws::WebsocketContext<Self>;

    // Initiate the heartbeat process on start
    fn started(&mut self, ctx: &mut Self::Context) {
        self.hb(ctx);
    }
}

// Handler for ws::Message
impl StreamHandler<ws::Message, ws::ProtocolError> for NodeSocket {
    fn handle(&mut self, msg: ws::Message, ctx: &mut Self::Context) {
        let ip = self.ip.clone();
        let mut logs: Option<Value> = None;
        match msg {
            ws::Message::Ping(msg) => {
                self.msg_count.ping += 1;
                debug!("PING from: {}", ip);
                self.hb = Instant::now();
                ctx.pong(&msg);
            }
            ws::Message::Pong(_) => {
                self.msg_count.pong += 1;
                debug!("PONG from: {} - message count: ({})", ip, self.msg_count);
                self.hb = Instant::now();
            }
            ws::Message::Text(text) => {
                self.metrics
                    .inc_ws_bytes_received(text.as_bytes().len() as u64);
                self.msg_count.text += 1;
                logs = match serde_json::from_str(&text) {
                    Ok(a) => Some(a),
                    Err(e) => {
                        error!("{:?}", e);
                        return;
                    }
                };
            }
            ws::Message::Binary(bin) => {
                self.metrics.inc_ws_bytes_received(bin.len() as u64);
                self.msg_count.binary += 1;
                logs = match serde_json::from_slice(&bin[..]) {
                    Ok(a) => Some(a),
                    Err(e) => {
                        error!("{:?}", e);
                        return;
                    }
                };
            }
            ws::Message::Close(_) => {
                info!(
                    "Close received, disconnecting: {} - message count: ({})",
                    ip, self.msg_count
                );
                ctx.stop();
            }
            ws::Message::Nop => (),
        }
        if let Some(logs) = logs {
            self.metrics.inc_ws_message_count();
            if self.peer_connection.peer_id.is_none() {
                debug!("Searching for peerId for ip address: {}", &ip);
                if let Some(peer_id) = logs["network_state"]["peerId"].as_str() {
                    debug!("Found peerId: {}, for ip address: {}", &peer_id, &ip);
                    self.peer_connection.peer_id = Some(peer_id.to_string());
                    match self.db.send(self.peer_connection.clone()).wait() {
                        Ok(Ok(())) => debug!(
                            "Saved new peer connection record (ID: {:?}) for peer_id: {}",
                            self.peer_connection.id, peer_id
                        ),
                        _ => error!(
                            "Failed to send updated PeerConnection to DB actor for peer_connection_id: {}",
                            self.peer_connection.id),
                    }
                }
            }
            if let Some(ts) = logs["ts"].as_str() {
                if let Ok(ts_utc) = DateTime::parse_from_rfc3339(ts) {
                    self.db
                        .try_send(NewSubstrateLog {
                            peer_connection_id: self.peer_connection.id,
                            created_at: ts_utc.naive_utc(),
                            logs,
                        })
                        .unwrap_or_else(|e| {
                            error!("Failed to send NewSubstrateLog to DB actor - {:?}", e)
                        });
                } else {
                    warn!("Unable to parse_from_rfc3339 for timestamp: {:?}", ts);
                }
            } else {
                warn!("Unable to find timestamp in logs: {:?}", logs);
            }
        }
    }
}

pub fn configure(cfg: &mut a_web::ServiceConfig) {
    cfg.service(
        a_web::scope("/").service(a_web::resource("").route(a_web::get().to_async(ws_index))),
    );
}

// Websocket handshake and start actor
fn ws_index(
    r: HttpRequest,
    stream: a_web::Payload,
    db: a_web::Data<Addr<DbExecutor>>,
    metrics: a_web::Data<Metrics>,
) -> Result<HttpResponse, Error> {
    let ip = r
        .connection_info()
        .remote()
        .unwrap_or("Unable to decode remote IP")
        .to_string();
    debug_headers(&r);
    info!("Establishing ws connection to node: {}", ip);
    match NodeSocket::new(ip.clone(), db, metrics.clone()) {
        Ok(ns) => {
            metrics.inc_ws_connected_count();
            debug!(
                "Created PeerConnection record, id: {}, for ip: {}",
                ns.peer_connection.id, ip
            );
            let mut res = ws::handshake(&r)?;
            let codec = Codec::new().max_size(*WS_MAX_PAYLOAD);
            let ws_context = ws::WebsocketContext::with_codec(ns, stream, codec);
            Ok(res.streaming(ws_context))
        }
        Err(e) => {
            error!(
                "Unable to save PeerConnection, aborting WS handshake for ip: {}",
                ip
            );
            Err(error::ErrorInternalServerError(e))
        }
    }
}

fn debug_headers(req: &HttpRequest) {
    let head = req.head();
    let headers = head.headers();
    debug!(
        "HTTP peer_addr (could be proxy): {:?}",
        head.peer_addr
            .expect("Should always have access to peer_addr from request")
    );
    for (k, v) in headers.iter() {
        trace!("HEADER MAP: Key: {}", k);
        trace!("HEADER MAP: Value: {:?}", v);
    }
}
