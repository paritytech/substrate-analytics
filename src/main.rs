#[macro_use]
extern crate log;
extern crate env_logger;
#[macro_use]
extern crate diesel;
#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate serde_json;

use dotenv::dotenv;
use std::env;
use std::time::{Duration, Instant};

use actix::prelude::*;
use actix_web::{http, middleware, server, ws, App, Error, HttpRequest, HttpResponse};

pub mod db;
pub mod schema;
pub mod util;
use crate::db::models::NewSubstrateLog;
use crate::db::*;

lazy_static! {
    pub static ref HEARTBEAT_INTERVAL: Duration = Duration::from_secs(
        env::var("HEARTBEAT_INTERVAL")
            .unwrap_or("".to_string())
            .parse()
            .unwrap_or(5)
    );
    pub static ref CLIENT_TIMEOUT: Duration = Duration::from_secs(
        env::var("CLIENT_TIMEOUT")
            .unwrap_or("".to_string())
            .parse()
            .unwrap_or(10)
    );
    pub static ref PURGE_FREQUENCY: Duration = Duration::from_secs(
        env::var("PURGE_FREQUENCY")
            .unwrap_or("".to_string())
            .parse()
            .unwrap_or(600)
    );
    pub static ref LOG_EXPIRY_HOURS: u32 = env::var("HEARTBEAT_INTERVAL")
        .unwrap_or("".to_string())
        .parse()
        .unwrap_or(168);
    pub static ref MAX_PENDING_CONNECTIONS: i32 = env::var("MAX_PENDING_CONNECTIONS")
        .unwrap_or("".to_string())
        .parse()
        .unwrap_or(8192);
    pub static ref DATABASE_URL: String =
        env::var("DATABASE_URL").expect("Must have DATABASE_URL environment variable set");
    pub static ref DATABASE_POOL_SIZE: u32 = env::var("DATABASE_POOL_SIZE")
        .unwrap_or("".to_string())
        .parse()
        .unwrap_or(10);
    pub static ref PORT: String = env::var("PORT").expect("Unable to get PORT from ENV");
}

fn log_statics() {
    info!("HEARTBEAT_INTERVAL = {:?}", *HEARTBEAT_INTERVAL);
    info!("CLIENT_TIMEOUT = {:?}", *CLIENT_TIMEOUT);
    info!("PURGE_FREQUENCY = {:?}", *PURGE_FREQUENCY);
    info!("LOG_EXPIRY_HOURS = {:?}", *LOG_EXPIRY_HOURS);
    info!("MAX_PENDING_CONNECTIONS = {:?}", *MAX_PENDING_CONNECTIONS);
    info!("DATABASE_URL = {:?}", *DATABASE_URL);
    info!("DATABASE_POOL_SIZE = {:?}", *DATABASE_POOL_SIZE);
    info!("PORT = {:?}", *PORT);
}

/// websocket handshake and start actor
fn ws_index(r: &HttpRequest<State>) -> Result<HttpResponse, Error> {
    let ip = r
        .connection_info()
        .remote()
        .unwrap_or("Unable to decode remote IP")
        .to_string();
    info!("Establishing connection to node: {}", ip);
    ws::start(r, NodeSocket::new(ip))
}

struct NodeSocket {
    hb: Instant,
    ip: String,
}

impl Actor for NodeSocket {
    type Context = ws::WebsocketContext<Self, State>;

    /// Method is called on actor start. Start the heartbeat process here.
    fn started(&mut self, ctx: &mut Self::Context) {
        self.hb(ctx);
    }
}

/// Handler for `ws::Message`
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
                let log = NewSubstrateLog {
                    node_ip: self.ip.clone(),
                    logs: json!(text),
                };
                ctx.state()
                    .db
                    .try_send(CreateSubstrateLog { log })
                    .unwrap_or_else(|e| error!("{:?}", e));
            }
            ws::Message::Binary(_bin) => (),
            ws::Message::Close(_) => {
                ctx.stop();
            }
        }
    }
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

struct State {
    db: Addr<db::DbExecutor>,
}

fn main() {
    dotenv().ok();
    env_logger::init();
    log_statics();
    info!("Starting Substrate S.A.V.E.");
    let sys = actix::System::new("substrate-save");
    info!("Creating database pool");
    let pool = create_pool();
    let num_threads = num_cpus::get() * 3;
    info!("Starting DbExecutor with {} threads", num_threads);
    let db_arbiter = SyncArbiter::start(num_threads, move || db::DbExecutor { pool: pool.clone() });
    info!("DbExecutor started");

    let frequency = *PURGE_FREQUENCY;
    let purge_logs = PurgeLogs {
        hours_valid: *LOG_EXPIRY_HOURS,
    };
    let db = db_arbiter.clone();
    let purger = util::DbPurge {
        frequency,
        purge_logs,
        db,
    };
    purger.start();

    let mut address = String::from("0.0.0.0:");
    address.push_str(&*PORT);
    info!("Starting server on: {}", &address);
    server::new(move || {
        App::with_state(State { db: db_arbiter.clone() })
            .middleware(middleware::Logger::default())
            .resource("/", |r| r.method(http::Method::GET).f(ws_index))
    })
    .backlog(*MAX_PENDING_CONNECTIONS) // max pending connections
    .bind(&address)
    .unwrap()
    .start();

    info!("Server started");
    let _ = sys.run();
}
