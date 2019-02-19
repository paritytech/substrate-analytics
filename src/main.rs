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

pub mod db;
pub mod schema;
pub mod util;

use dotenv::dotenv;
use std::env;
use std::time::{Duration, Instant};

use crate::db::models::NewSubstrateLog;
use crate::db::*;
use actix::prelude::*;
use actix_web::{http, middleware, server, ws, App, Error, HttpRequest, HttpResponse};

struct State {
    db: Addr<db::DbExecutor>,
}

struct NodeSocket {
    hb: Instant,
    ip: String,
}

lazy_static! {
    /// Must be set
    pub static ref DATABASE_URL: String =
        env::var("DATABASE_URL").expect("Must have DATABASE_URL environment variable set");
    pub static ref PORT: String = env::var("PORT").expect("Unable to get PORT from ENV");
    /// Optional
    pub static ref HEARTBEAT_INTERVAL: Duration = Duration::from_secs(
        parse_env("HEARTBEAT_INTERVAL").unwrap_or(5)
    );
    pub static ref CLIENT_TIMEOUT: Duration = Duration::from_secs(
        parse_env("CLIENT_TIMEOUT").unwrap_or(10)
    );
    pub static ref PURGE_FREQUENCY: Duration = Duration::from_secs(
        parse_env("PURGE_FREQUENCY").unwrap_or(600)
    );
    pub static ref LOG_EXPIRY_HOURS: u32 = parse_env("LOG_EXPIRY_HOURS").unwrap_or(168);
    pub static ref MAX_PENDING_CONNECTIONS: i32 = parse_env("MAX_PENDING_CONNECTIONS").unwrap_or(8192);
    pub static ref DATABASE_POOL_SIZE: u32 = parse_env("DATABASE_POOL_SIZE").unwrap_or(10);
}

fn main() {
    dotenv().ok();
    env_logger::init();
    log_statics();
    info!("Starting Substrate SAVE");
    let sys = actix::System::new("substrate-save");
    info!("Creating database pool");
    let pool = create_pool();
    let num_threads = num_cpus::get() * 3;
    info!("Starting DbExecutor with {} threads", num_threads);
    let db_arbiter = SyncArbiter::start(num_threads, move || db::DbExecutor { pool: pool.clone() });
    info!("DbExecutor started");

    let purger = util::PeriodicAction {
        frequency: *PURGE_FREQUENCY,
        message: PurgeLogs {
            hours_valid: *LOG_EXPIRY_HOURS,
        },
        recipient: db_arbiter.clone().recipient(),
    };
    purger.start();

    let address = format!("0.0.0.0:{}", &*PORT);
    info!("Starting server on: {}", &address);
    server::new(move || {
        App::with_state(State {
            db: db_arbiter.clone(),
        })
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

// impl

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

// Private

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

fn parse_env<T>(var: &'static str) -> Result<T, ()>
where
    T: std::str::FromStr,
{
    env::var(var)
        .map_err(|_| ())
        .and_then(|v| v.parse().map_err(|_| ()))
}

// websocket handshake and start actor
fn ws_index(r: &HttpRequest<State>) -> Result<HttpResponse, Error> {
    let ip = r
        .connection_info()
        .remote()
        .unwrap_or("Unable to decode remote IP")
        .to_string();
    info!("Establishing connection to node: {}", ip);
    ws::start(r, NodeSocket::new(ip))
}
