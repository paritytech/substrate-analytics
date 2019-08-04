use std::time::Instant;

use crate::db::{models::NewSubstrateLog, DbExecutor};
use crate::{CLIENT_TIMEOUT, HEARTBEAT_INTERVAL};

use actix::prelude::*;
//use actix_http::body::Body;
use actix_web::{web as a_web, Error, HttpRequest, HttpResponse};
use actix_web_actors::ws;
use serde_json::Value;

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
    info!("Establishing ws connection to node: {}", ip);
    ws::start(NodeSocket::new(ip, db), &r, stream)
}

struct NodeSocket {
    hb: Instant,
    ip: String,
    db: a_web::Data<Addr<DbExecutor>>,
}

impl NodeSocket {
    fn new(ip: String, db: a_web::Data<Addr<DbExecutor>>) -> Self {
        Self {
            ip,
            db,
            hb: Instant::now(),
        }
    }

    fn hb(&self, ctx: &mut <Self as Actor>::Context) {
        let ip = self.ip.clone();
        ctx.run_interval(*HEARTBEAT_INTERVAL, move |act, ctx| {
            if Instant::now().duration_since(act.hb) > *CLIENT_TIMEOUT {
                info!(
                    "Websocket heartbeat failed for node: {}", // TODO re-add 'disconnecting' to msg
                    ip
                );
                // ctx.stop(); TODO re-enable this once substrate is fixed and responding to PING
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
                debug!("PING from: {}", ip);
                self.hb = Instant::now();
                ctx.pong(&msg);
            }
            ws::Message::Pong(_) => {
                debug!("PONG from: {}", ip);
                self.hb = Instant::now();
            }
            ws::Message::Text(text) => {
                trace!("TEXT from: {} - {}", ip, text);
                logs = match serde_json::from_str(&text) {
                    Ok(a) => Some(a),
                    Err(e) => {
                        error!("{:?}", e);
                        return;
                    }
                };
            }
            ws::Message::Binary(bin) => {
                trace!("BINARY from: {} - {:?}", ip, bin);
                logs = match serde_json::from_slice(&bin[..]) {
                    Ok(a) => Some(a),
                    Err(e) => {
                        error!("{:?}", e);
                        return;
                    }
                };
            }
            ws::Message::Close(_) => {
                ctx.stop();
            }
            ws::Message::Nop => (),
        }
        if let Some(logs) = logs {
            self.db
                .try_send(NewSubstrateLog {
                    node_ip: self.ip.clone(),
                    logs,
                })
                .unwrap_or_else(|e| error!("Failed to send log to DB actor - {:?}", e));
        }
    }
}
