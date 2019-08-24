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
mod web;

use dotenv::dotenv;
use std::env;
use std::time::Duration;

use crate::db::*;
use actix::prelude::*;
use actix_web::{middleware, App, HttpServer};

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
    pub static ref LOG_EXPIRY_HOURS: u32 = parse_env("LOG_EXPIRY_HOURS").unwrap_or(core::u32::MAX);
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
    info!("Starting DbArbiter with {} threads", num_threads);
    let db_arbiter = SyncArbiter::start(num_threads, move || db::DbExecutor::new(pool.clone()));
    info!("DbExecutor started");

    let db_housekeeper = util::PeriodicAction {
        frequency: *PURGE_FREQUENCY,
        message: PurgeLogs {
            hours_valid: *LOG_EXPIRY_HOURS,
        },
        recipient: db_arbiter.clone().recipient(),
    };
    db_housekeeper.start();

    let address = format!("0.0.0.0:{}", &*PORT);
    info!("Starting server on: {}", &address);
    HttpServer::new(move || {
        App::new()
            .data(db_arbiter.clone())
            .wrap(middleware::NormalizePath)
            .wrap(middleware::Logger::default())
            .configure(web::nodes::configure)
            .configure(web::stats::configure)
            .configure(web::root::configure)
    })
    .backlog(*MAX_PENDING_CONNECTIONS)
    .bind(&address)
    .unwrap()
    .start();

    info!("Server started");
    let _ = sys.run();
}

// Private

fn log_statics() {
    info!("HEARTBEAT_INTERVAL = {:?}", *HEARTBEAT_INTERVAL);
    info!("CLIENT_TIMEOUT = {:?}", *CLIENT_TIMEOUT);
    info!("PURGE_FREQUENCY = {:?}", *PURGE_FREQUENCY);
    info!("LOG_EXPIRY_HOURS = {:?}", *LOG_EXPIRY_HOURS);
    info!("MAX_PENDING_CONNECTIONS = {:?}", *MAX_PENDING_CONNECTIONS);
    info!("DATABASE_URL = *HIDDEN*");
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
