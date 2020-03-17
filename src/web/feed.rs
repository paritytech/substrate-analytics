use std::time::{Duration, Instant};

use crate::cache::{Cache, Interest, Subscription};
use crate::db::peer_data::PeerDataResponse;
use crate::db::DbExecutor;
use actix::prelude::*;
use actix_files as fs;
use actix_web::{middleware, web, web::Data, App, Error, HttpRequest, HttpResponse, HttpServer};
use actix_web_actors::ws;
use chrono::NaiveDateTime;
use serde_json::Value;
//use serde_json::json;

const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(5);
const CLIENT_TIMEOUT: Duration = Duration::from_secs(10);

pub fn configure(cfg: &mut actix_web::web::ServiceConfig) {
    cfg.service(actix_web::web::scope("/feed").route("", actix_web::web::get().to(ws_index)));
}

async fn ws_index(
    r: HttpRequest,
    stream: web::Payload,
    db: Data<Addr<DbExecutor>>,
    cache: Data<Addr<Cache>>,
) -> Result<HttpResponse, Error> {
    ws::start(WebSocket::new(db, cache), &r, stream)
}

struct WebSocket {
    hb: Instant,
    last_logs: Option<NaiveDateTime>,
    cache: Data<Addr<Cache>>,
    db: Data<Addr<DbExecutor>>,
}

impl Handler<PeerDataResponse> for WebSocket {
    type Result = Result<(), &'static str>;

    fn handle(&mut self, msg: PeerDataResponse, ctx: &mut Self::Context) -> Self::Result {
        use ws::Message::Text;
        ctx.text(json!(msg).to_string());
        Ok(())
    }
}

impl Actor for WebSocket {
    type Context = ws::WebsocketContext<Self>;

    // Start heartbeat and updates on new connection
    fn started(&mut self, ctx: &mut Self::Context) {
        self.hb(ctx);
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
                    ctx.text(e);
                }
            }
            Ok(ws::Message::Binary(bin)) => (),
            Ok(ws::Message::Close(_)) => {
                ctx.stop();
            }
            _ => ctx.stop(),
        }
    }
}

impl WebSocket {
    fn new(db: Data<Addr<DbExecutor>>, cache: Data<Addr<Cache>>) -> Self {
        Self {
            hb: Instant::now(),
            last_logs: None,
            cache,
            db,
        }
    }

    fn process_message(
        &self,
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

    fn hb(&self, ctx: &mut <Self as Actor>::Context) {
        ctx.run_interval(HEARTBEAT_INTERVAL, |act, ctx| {
            if Instant::now().duration_since(act.hb) > CLIENT_TIMEOUT {
                info!("Websocket Client heartbeat failed, disconnecting!");
                ctx.stop();
                return;
            }
            ctx.ping(b"");
        });
    }
}
