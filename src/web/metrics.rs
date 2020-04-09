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

use actix_web::{http::StatusCode, HttpRequest, HttpResponse, Result as AWResult};
use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use sysinfo::{NetworkExt, NetworksExt, System, SystemExt};

#[derive(Clone, Default)]
pub struct Metrics {
    ws_message_count: Arc<AtomicU64>,
    ws_connected_count: Arc<AtomicU64>,
    ws_dropped_count: Arc<AtomicU64>,
    ws_bytes_received: Arc<AtomicU64>,
    req_count: Arc<AtomicU64>,
    feeds_connected: Arc<AtomicU64>,
    feeds_disconnected: Arc<AtomicU64>,
    system: Arc<System>,
}

impl Metrics {
    pub fn inc_ws_message_count(&self) {
        self.ws_message_count.fetch_add(1, Ordering::Relaxed);
    }
    pub fn inc_ws_connected_count(&self) {
        self.ws_connected_count.fetch_add(1, Ordering::Relaxed);
    }
    pub fn inc_ws_dropped_count(&self) {
        self.ws_dropped_count.fetch_add(1, Ordering::Relaxed);
    }
    pub fn inc_ws_bytes_received(&self, n: u64) {
        self.ws_bytes_received.fetch_add(n, Ordering::Relaxed);
    }
    pub fn inc_req_count(&self) {
        self.req_count.fetch_add(1, Ordering::Relaxed);
    }
    pub fn inc_concurrent_feed_count(&self) {
        self.feeds_connected.fetch_add(1, Ordering::Relaxed);
    }
    pub fn dec_concurrent_feed_count(&self) {
        self.feeds_disconnected.fetch_add(1, Ordering::Relaxed);
    }

    fn bytes_io(&self, sys: &System) -> (u64, u64) {
        let mut total_sent = 0;
        let mut total_rec = 0;
        for (_, data) in sys.get_networks().iter() {
            total_sent += data.get_total_transmitted();
            total_rec += data.get_total_received();
        }
        (total_sent, total_rec)
    }
}

const WS_MESSAGE_COUNT_TEMPLATE: &str =
    "# HELP substrate_message_count Number of binary and text messages received - (does not include PING/PONG messages)\n\
     # TYPE substrate_message_count counter\n\
     substrate_message_count ";

const WS_CONNECTED_COUNT_TEMPLATE: &str =
    "# HELP nodes_connected_count Total number of WS connections made since launch.\n\
     # TYPE nodes_connected_count counter\n\
     nodes_connected_count ";

const WS_DROPPED_COUNT_TEMPLATE: &str =
    "# HELP nodes_dropped_count Total number of WS connections dropped since launch.\n\
     # TYPE nodes_dropped_count counter\n\
     nodes_dropped_count ";

const CURRENT_SUBSTRATE_CONNECTIONS_TEMPLATE: &str =
    "# HELP current_substrate_connections Number of WS substrate connections sending data.\n\
     # TYPE current_substrate_connections gauge\n\
     current_substrate_connections ";

const SUBSTRATE_BYTES_RECEIVED_TEMPLATE: &str =
    "# HELP substrate_bytes_received Total bytes received in binary and text WS messages from substrate clients.\n\
     # TYPE substrate_bytes_received counter\n\
     substrate_bytes_received ";

const BYTES_RECEIVED_TEMPLATE: &str =
    "# HELP bytes_received Total bytes received in binary and text WS messages.\n\
     # TYPE bytes_received counter\n\
     bytes_received ";

const BYTES_SENT_TEMPLATE: &str =
    "# HELP bytes_sent Total bytes received in binary and text WS messages.\n\
     # TYPE bytes_sent counter\n\
     bytes_sent ";

const REQ_COUNT_TEMPLATE: &str =
    "# HELP requests Number of get requests to non WS routes, also excluding metrics route.\n\
     # TYPE requests counter\n\
     requests ";

const CURRENT_FEED_COUNT_TEMPLATE: &str =
    "# HELP current_feed_connections Number of WS feed connections consuming data.\n\
     # TYPE current_feed_connections gauge\n\
     current_feed_connections ";

const FEEDS_CONNECTED_TEMPLATE: &str =
    "# HELP feeds_connected Number of WS connections for live feed\n\
     # TYPE feeds_connected counter\n\
     feeds_connected ";

const FEEDS_DROPPED_TEMPLATE: &str =
    "# HELP feeds_disconnected Number of WS disconnections for live feed\n\
     # TYPE feeds_disconnected counter\n\
     feeds_disconnected ";

const LOAD_AVG_ONE_TEMPLATE: &str = "# HELP load_avg_one System load average one minute\n\
     # TYPE load_avg_one gauge\n\
     load_avg_one ";

const LOAD_AVG_FIVE_TEMPLATE: &str = "# HELP load_avg_five System load average five minutes\n\
     # TYPE load_avg_five gauge\n\
     load_avg_five ";

const LOAD_AVG_FIFTEEN_TEMPLATE: &str =
    "# HELP load_avg_fifteen System load average fifteen minutes\n\
     # TYPE load_avg_fifteen gauge\n\
     load_avg_fifteen ";

const TOTAL_MEM_TEMPLATE: &str = "# HELP total_mem Total system RAM (KiB)\n\
     # TYPE total_mem gauge\n\
     total_mem ";

const USED_MEM_TEMPLATE: &str = "# HELP used_mem Used system RAM (KiB)\n\
     # TYPE used_mem gauge\n\
     used_mem ";

const TOTAL_SWAP_TEMPLATE: &str = "# HELP total_swap Total system swap (KiB)\n\
     # TYPE total_swap gauge\n\
     total_swap ";

const USED_SWAP_TEMPLATE: &str = "# HELP used_swap Used swap (KiB)\n\
     # TYPE used_swap gauge\n\
     used_swap ";

impl fmt::Display for Metrics {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let sys = System::new_all();
        let (total_sent, total_rec) = self.bytes_io(&sys);
        let load_avg = sys.get_load_average();
        write!(
            f,
            "{}{}\n\
             {}{}\n\
             {}{}\n\
             {}{}\n\
             {}{}\n\
             {}{}\n\
             {}{}\n\
             {}{}\n\
             {}{}\n\
             {}{}\n\
             {}{}\n\
             {}{}\n\
             {}{}\n\
             {}{}\n\
             {}{}\n\
             {}{}\n\
             {}{}\n\
             {}{}\n",
            WS_MESSAGE_COUNT_TEMPLATE,
            self.ws_message_count.load(Ordering::Relaxed),
            WS_CONNECTED_COUNT_TEMPLATE,
            self.ws_connected_count.load(Ordering::Relaxed),
            WS_DROPPED_COUNT_TEMPLATE,
            self.ws_dropped_count.load(Ordering::Relaxed),
            CURRENT_SUBSTRATE_CONNECTIONS_TEMPLATE,
            self.ws_connected_count.load(Ordering::Relaxed)
                - self.ws_dropped_count.load(Ordering::Relaxed),
            SUBSTRATE_BYTES_RECEIVED_TEMPLATE,
            self.ws_bytes_received.load(Ordering::Relaxed),
            BYTES_RECEIVED_TEMPLATE,
            total_rec,
            BYTES_SENT_TEMPLATE,
            total_sent,
            REQ_COUNT_TEMPLATE,
            self.req_count.load(Ordering::Relaxed),
            CURRENT_FEED_COUNT_TEMPLATE,
            self.feeds_connected.load(Ordering::Relaxed)
                - self.feeds_disconnected.load(Ordering::Relaxed),
            FEEDS_CONNECTED_TEMPLATE,
            self.feeds_connected.load(Ordering::Relaxed),
            FEEDS_DROPPED_TEMPLATE,
            self.feeds_disconnected.load(Ordering::Relaxed),
            LOAD_AVG_ONE_TEMPLATE,
            load_avg.one,
            LOAD_AVG_FIVE_TEMPLATE,
            load_avg.five,
            LOAD_AVG_FIFTEEN_TEMPLATE,
            load_avg.fifteen,
            TOTAL_MEM_TEMPLATE,
            sys.get_total_memory(),
            USED_MEM_TEMPLATE,
            sys.get_used_memory(),
            TOTAL_SWAP_TEMPLATE,
            sys.get_total_swap(),
            USED_SWAP_TEMPLATE,
            sys.get_used_swap(),
        )
    }
}

pub fn configure(cfg: &mut actix_web::web::ServiceConfig) {
    cfg.service(actix_web::web::scope("/metrics").route("", actix_web::web::get().to(root)));
}

async fn root(
    _r: HttpRequest,
    metrics: actix_web::web::Data<Metrics>,
) -> AWResult<HttpResponse, actix_web::Error> {
    Ok(HttpResponse::build(StatusCode::OK)
        .content_type("text/plain; version=0.0.4; charset=utf-8")
        .body(metrics.to_string()))
}
