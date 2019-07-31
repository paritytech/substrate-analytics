use std::time::Instant;

use super::State;
use crate::db::models::NewSubstrateLog;
use crate::{CLIENT_TIMEOUT, HEARTBEAT_INTERVAL};

use actix::prelude::*;
use actix_web::{http::Method, ws, App, Error, HttpRequest, HttpResponse};
use serde_json::Value;

pub fn configure(app: App<State>) -> App<State> {
    app.scope("", |scope| {
        // Polkadot 0.3 currently adds a trailing slash to the url
        scope.resource("/", |r| r.method(Method::GET).f(ws_index))
    })
}

// Websocket handshake and start actor
fn ws_index(r: &HttpRequest<State>) -> Result<HttpResponse, Error> {
    let ip = r
        .connection_info()
        .remote()
        .unwrap_or("Unable to decode remote IP")
        .to_string();
    info!("Establishing ws connection to node: {}", ip);
    ws::start(r, NodeSocket::new(ip))
}

struct NodeSocket {
    hb: Instant,
    ip: String,
}

impl NodeSocket {
    fn new(ip: String) -> Self {
        Self {
            ip,
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
    type Context = ws::WebsocketContext<Self, State>;

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
            ws::Message::Binary(mut bin) => {
                trace!("BINARY from: {} - {:?}", ip, bin);
                logs = match serde_json::from_slice(&bin.take()[..]) {
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
        }
        if let Some(logs) = logs {
            ctx.state()
                .db
                .try_send(NewSubstrateLog {
                    node_ip: self.ip.clone(),
                    logs,
                })
                .unwrap_or_else(|e| error!("Failed to send log to DB actor - {:?}", e));
        }
    }
}
