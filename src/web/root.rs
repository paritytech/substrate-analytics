use std::fmt;
use std::time::Instant;

use crate::db::{
    models::{NewPeerConnection, NewSubstrateLog, PeerConnection},
    DbExecutor,
};
use crate::{CLIENT_TIMEOUT, HEARTBEAT_INTERVAL};

use actix::prelude::*;
use actix_http::ws::Codec;
use actix_web::{error, web as a_web, Error, HttpRequest, HttpResponse};
use actix_web_actors::ws;
use serde_json::Value;

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
    peer_connection: PeerConnection,
    msg_count: MessageCount,
}

impl NodeSocket {
    fn new(ip: String, db: a_web::Data<Addr<DbExecutor>>) -> Result<Self, String> {
        Ok(Self {
            peer_connection: Self::create_peer_connection(&db, &ip)?,
            ip,
            db,
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
            self.db
                .try_send(NewSubstrateLog {
                    peer_connection_id: self.peer_connection.id, //
                    logs,
                })
                .unwrap_or_else(|e| error!("Failed to send NewSubstrateLog to DB actor - {:?}", e));
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
) -> Result<HttpResponse, Error> {
    let ip = r
        .connection_info()
        .remote()
        .unwrap_or("Unable to decode remote IP")
        .to_string();
    debug_headers(&r);
    info!("Establishing ws connection to node: {}", ip);
    match NodeSocket::new(ip.clone(), db) {
        Ok(ns) => {
            debug!(
                "Created PeerConnection record, id: {}, for ip: {}",
                ns.peer_connection.id, ip
            );
            let mut res = ws::handshake(&r).map_err(|e| Error::from(()))?;
            // Set Codec to accept payload size of 256 MiB because default 65KiB is not enough
            let mut codec = Codec::new().max_size(268_435_456);
            info!("Codec {:?}", codec);
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
