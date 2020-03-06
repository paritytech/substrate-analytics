use std::time::{Duration, Instant};

use crate::cache;
use crate::db::*;
use actix::prelude::*;
use actix_files as fs;
use actix_web::{middleware, web, App, Error, HttpRequest, HttpResponse, HttpServer};
use actix_web_actors::ws;
use chrono::NaiveDateTime;

const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(5);
const CLIENT_TIMEOUT: Duration = Duration::from_secs(10);

pub fn configure(cfg: &mut actix_web::web::ServiceConfig) {
    cfg.service(actix_web::web::scope("/streaming").route("", actix_web::web::get().to(ws_index)));
}

async fn ws_index(r: HttpRequest, stream: web::Payload) -> Result<HttpResponse, Error> {
    println!("{:?}", r);
    let res = ws::start(WebSocket::new(), &r, stream);
    println!("{:?}", res);
    res
}

struct WebSocket {
    hb: Instant,
    last_logs: Option<NaiveDateTime>,
}

impl Actor for WebSocket {
    type Context = ws::WebsocketContext<Self>;

    // Start heartbeat and updates on new connection
    fn started(&mut self, ctx: &mut Self::Context) {
        self.hb(ctx);
        //        ctx.run_interval(Duration::from_secs(2), |_act, ctx| {
        //            ctx.text(
        //                ctx.wait(cache::get_latest(self.last_logs))
        //                    .into()
        //                    .unwrap()
        //                    .to_string(),
        //            );
        //        });
    }
}

/// Handler for `ws::Message`
impl StreamHandler<Result<ws::Message, ws::ProtocolError>> for WebSocket {
    fn handle(&mut self, msg: Result<ws::Message, ws::ProtocolError>, ctx: &mut Self::Context) {
        // process websocket messages
        println!("WS: {:?}", msg);
        match msg {
            Ok(ws::Message::Ping(msg)) => {
                self.hb = Instant::now();
                ctx.pong(&msg);
            }
            Ok(ws::Message::Pong(_)) => {
                self.hb = Instant::now();
            }
            Ok(ws::Message::Text(text)) => ctx.text(text),
            Ok(ws::Message::Binary(bin)) => ctx.binary(bin),
            Ok(ws::Message::Close(_)) => {
                ctx.stop();
            }
            _ => ctx.stop(),
        }
    }
}

impl WebSocket {
    fn new() -> Self {
        Self {
            hb: Instant::now(),
            last_logs: None,
        }
    }

    /// helper method that sends ping to client every second.
    ///
    /// also this method checks heartbeats from client
    fn hb(&self, ctx: &mut <Self as Actor>::Context) {
        ctx.run_interval(HEARTBEAT_INTERVAL, |act, ctx| {
            // check client heartbeats
            if Instant::now().duration_since(act.hb) > CLIENT_TIMEOUT {
                // heartbeat timed out
                println!("Websocket Client heartbeat failed, disconnecting!");

                // stop actor
                ctx.stop();

                // don't try to send a ping
                return;
            }

            ctx.ping(b"");
        });
    }
}
