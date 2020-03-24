// Copyright 2019 Parity Technologies (UK) Ltd.
// This file is part of Substrate Analytics.

// Substrate Analytics is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// Substrate Analytics is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with Substrate Analytics.  If not, see <http://www.gnu.org/licenses/>.

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

pub mod cache;
pub mod db;
pub mod schema;
pub mod util;
mod web;

use cache::Cache;

use dotenv::dotenv;
use std::env;
use std::time::Duration;

use crate::db::models::NewSubstrateLog;
//use crate::db::peer_data::UpdateCache;
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
    pub static ref CLIENT_TIMEOUT_S: Duration = Duration::from_secs(
        parse_env("CLIENT_TIMEOUT_S").unwrap_or(10)
    );
    pub static ref PURGE_INTERVAL_S: Duration = Duration::from_secs(
        parse_env("PURGE_INTERVAL_S").unwrap_or(600)
    );
    pub static ref LOG_EXPIRY_H: u32 = parse_env("LOG_EXPIRY_H").unwrap_or(3);
    pub static ref MAX_PENDING_CONNECTIONS: i32 = parse_env("MAX_PENDING_CONNECTIONS").unwrap_or(8192);

    // Set Codec to accept payload size of 512 MiB because default 65KiB is not enough
    pub static ref WS_MAX_PAYLOAD: usize = parse_env("WS_MAX_PAYLOAD").unwrap_or(524_288);

    pub static ref NUM_THREADS: usize = num_cpus::get() * 3;

    pub static ref DB_POOL_SIZE: u32 = parse_env("DB_POOL_SIZE").unwrap_or(*NUM_THREADS as u32);
    pub static ref DB_BATCH_SIZE: usize = parse_env("DB_BATCH_SIZE").unwrap_or(1024);
    pub static ref DB_SAVE_LATENCY_MS: Duration = Duration::from_millis(parse_env("DB_SAVE_LATENCY_MS").unwrap_or(100));

    pub static ref CACHE_UPDATE_TIMEOUT_S: Duration = Duration::from_secs(parse_env("CACHE_UPDATE_TIMEOUT_S").unwrap_or(15));
    pub static ref CACHE_UPDATE_INTERVAL_MS: Duration = Duration::from_millis(parse_env("CACHE_UPDATE_INTERVAL_MS").unwrap_or(1000));
    pub static ref CACHE_EXPIRY_S: u64 = parse_env("CACHE_EXPIRY_S").unwrap_or(3600);

    pub static ref ASSETS_PATH: String = parse_env("ASSETS_PATH").unwrap_or("./static".to_string());
}

struct LogBuffer {
    logs: Vec<NewSubstrateLog>,
    db_arbiter: Recipient<LogBatch>,
}

impl Actor for LogBuffer {
    type Context = Context<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        ctx.set_mailbox_capacity(10_000);
    }
}

impl Message for NewSubstrateLog {
    type Result = Result<(), &'static str>;
}

impl Handler<NewSubstrateLog> for LogBuffer {
    type Result = Result<(), &'static str>;

    fn handle(&mut self, msg: NewSubstrateLog, _: &mut Self::Context) -> Self::Result {
        self.logs.push(msg);
        Ok(())
    }
}

#[derive(Clone)]
struct SaveLogs;

impl Message for SaveLogs {
    type Result = Result<(), &'static str>;
}

impl Handler<SaveLogs> for LogBuffer {
    type Result = Result<(), &'static str>;

    fn handle(&mut self, _msg: SaveLogs, _: &mut Self::Context) -> Self::Result {
        while !self.logs.is_empty() {
            let lb = LogBatch(
                self.logs
                    .split_off(self.logs.len().saturating_sub(*DB_BATCH_SIZE)),
            );
            self.db_arbiter
                .try_send(lb)
                .unwrap_or_else(|e| error!("Failed to send LogBatch to DB arbiter - {:?}", e));
        }
        Ok(())
    }
}

#[actix_rt::main]
async fn main() -> std::io::Result<()> {
    dotenv().ok();
    env_logger::init();
    log_statics();
    info!("Starting substrate-analytics");
    info!("Creating database pool");
    let pool = create_pool();
    info!("Starting DbArbiter with {} threads", *NUM_THREADS);

    let db_arbiter = SyncArbiter::start(*NUM_THREADS, move || db::DbExecutor::new(pool.clone()));
    info!("DbExecutor started");

    let cache = Cache::new(db_arbiter.clone()).start();

    let log_buffer = LogBuffer {
        logs: Vec::new(),
        db_arbiter: db_arbiter.clone().recipient(),
    }
    .start();

    util::PeriodicAction {
        interval: *PURGE_INTERVAL_S,
        message: PurgeLogs {
            hours_valid: *LOG_EXPIRY_H,
        },
        recipient: db_arbiter.clone().recipient(),
    }
    .start();

    util::PeriodicAction {
        interval: *DB_SAVE_LATENCY_MS,
        message: SaveLogs,
        recipient: log_buffer.clone().recipient(),
    }
    .start();

    let metrics = web::metrics::Metrics::default();
    let address = format!("0.0.0.0:{}", &*PORT);
    info!("Starting server on: {}", &address);
    HttpServer::new(move || {
        App::new()
            .data(db_arbiter.clone())
            .data(metrics.clone())
            .data(log_buffer.clone())
            .data(cache.clone())
            .data(actix_web::web::JsonConfig::default().limit(4096))
            .wrap(middleware::NormalizePath)
            .wrap(middleware::Logger::default())
            .configure(web::nodes::configure)
            .configure(web::reputation::configure)
            .configure(web::stats::configure)
            .configure(web::metrics::configure)
            .configure(web::benchmarks::configure)
            .configure(web::dashboard::configure)
            .configure(web::feed::configure)
            .configure(web::root::configure)
    })
    .backlog(*MAX_PENDING_CONNECTIONS)
    .bind(&address)?
    .run()
    .await
}

// Private

fn log_statics() {
    info!("Configuration options:");
    info!("DATABASE_URL has been set");
    info!("PORT = {:?}", *PORT);
    info!("NUM_THREADS = {:?}", *NUM_THREADS);
    info!("HEARTBEAT_INTERVAL = {:?}", *HEARTBEAT_INTERVAL);
    info!("CLIENT_TIMEOUT_S = {:?}", *CLIENT_TIMEOUT_S);
    info!("MAX_PENDING_CONNECTIONS = {:?}", *MAX_PENDING_CONNECTIONS);
    info!("WS_MAX_PAYLOAD = {:?} bytes", *WS_MAX_PAYLOAD);
    info!("DB_POOL_SIZE = {:?}", *DB_POOL_SIZE);
    info!("DB_BATCH_SIZE = {:?}", *DB_BATCH_SIZE);
    info!("DB_SAVE_LATENCY_MS = {:?}", *DB_SAVE_LATENCY_MS);
    info!("PURGE_INTERVAL_S = {:?}", *PURGE_INTERVAL_S);
    info!("LOG_EXPIRY_H = {:?}", *LOG_EXPIRY_H);
    info!("CACHE_UPDATE_TIMEOUT_S = {:?}", *CACHE_UPDATE_TIMEOUT_S);
    info!("CACHE_UPDATE_INTERVAL_MS = {:?}", *CACHE_UPDATE_INTERVAL_MS);
    info!("CACHE_EXPIRY_S = {:?}", *CACHE_EXPIRY_S);
}

fn parse_env<T>(var: &'static str) -> Result<T, ()>
where
    T: std::str::FromStr,
{
    env::var(var)
        .map_err(|_| ())
        .and_then(|v| v.parse().map_err(|_| ()))
}
