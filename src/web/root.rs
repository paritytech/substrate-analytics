use std::time::Instant;

use super::State;
use crate::db::logs::RawLog;
use crate::{CLIENT_TIMEOUT, HEARTBEAT_INTERVAL};

use actix::prelude::*;
use actix_web::{http::Method, ws, App, Error, HttpRequest, HttpResponse};

pub fn configure(app: App<State>) -> App<State> {
    app.scope("", |scope| {
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
                    "Websocket heartbeat failed for node: {}, disconnecting!",
                    ip
                );
                ctx.stop();
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
        match msg {
            ws::Message::Ping(msg) => {
                self.hb = Instant::now();
                ctx.pong(&msg);
            }
            ws::Message::Pong(_) => {
                self.hb = Instant::now();
            }
            ws::Message::Text(text) => {
                ctx.state()
                    .db
                    .try_send(RawLog {
                        ip_addr: self.ip.clone(),
                        json: text,
                    })
                    .unwrap_or_else(|e| error!("{:?}", e));
            }
            ws::Message::Binary(_bin) => (),
            ws::Message::Close(_) => {
                ctx.stop();
            }
        }
    }
}
