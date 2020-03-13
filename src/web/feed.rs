use std::time::{Duration, Instant};

use crate::cache::Cache;
use crate::db::peer_data::PeerDataResponse;
use crate::db::DbExecutor;
use actix::prelude::*;
use actix_files as fs;
use actix_web::{middleware, web, web::Data, App, Error, HttpRequest, HttpResponse, HttpServer};
use actix_web_actors::ws;
use chrono::NaiveDateTime;
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
    println!("{:?}", r);
    let res = ws::start(WebSocket::new(db, cache), &r, stream);
    println!("{:?}", res);
    res
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
        self.test_subscribe(ctx);
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
    fn new(db: Data<Addr<DbExecutor>>, cache: Data<Addr<Cache>>) -> Self {
        Self {
            hb: Instant::now(),
            last_logs: None,
            cache,
            db,
        }
    }

    fn test_subscribe(&self, ctx: &mut <Self as Actor>::Context) {
        let subscription = crate::cache::Subscription {
            peer_id: "QmWrgWU55yCBPaffKmTRX7rZPEhvY4fu1TnEnn94zATvDv".to_owned(),
            msg: "system.interval".to_owned(),
            subscriber_addr: ctx.address().recipient(),
            start_time: Some(crate::db::peer_data::create_date_time(60)),
            interest: crate::cache::Interest::Subscribe,
        };
        let subscription2 = crate::cache::Subscription {
            peer_id: "QmWrgWU55yCBPaffKmTRX7rZPEhvY4fu1TnEnn94zATvDv".to_owned(),
            msg: "tracing.profiling".to_owned(),
            subscriber_addr: ctx.address().recipient(),
            start_time: Some(crate::db::peer_data::create_date_time(60)),
            interest: crate::cache::Interest::Subscribe,
        };

        match self.cache.try_send(subscription) {
            Ok(_) => info!("Sent subscription"),
            Err(e) => error!("Could not send subscription due to: {:?}", e),
        }

        match self.cache.try_send(subscription2) {
            Ok(_) => info!("Sent subscription"),
            Err(e) => error!("Could not send subscription due to: {:?}", e),
        }
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
