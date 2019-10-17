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

use actix_web::{
    http::StatusCode, Error as AWError, HttpRequest, HttpResponse, Result as AWResult,
};
use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

#[derive(Clone, Default)]
pub struct Metrics {
    ws_message_count: Arc<AtomicU64>,
    ws_connected_count: Arc<AtomicU64>,
    ws_dropped_count: Arc<AtomicU64>,
    ws_bytes_received: Arc<AtomicU64>,
    req_count: Arc<AtomicU64>,
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
}

const WS_MESSAGE_COUNT_TEMPLATE: &'static str =
    "# HELP save_ws_message_count Number of binary and text messages received - (does not include PING/PONG messages)\n\
     # TYPE save_ws_message_count counter\n\
     save_ws_message_count ";

const WS_CONNECTED_COUNT_TEMPLATE: &'static str =
    "# HELP save_ws_connected_count Total number of WS connections made since launch.\n\
     # TYPE save_ws_connected_count counter\n\
     save_ws_connected_count ";

const WS_DROPPED_COUNT_TEMPLATE: &'static str =
    "# HELP save_ws_dropped_count Total number of WS connections dropped since launch.\n\
     # TYPE save_ws_dropped_count counter\n\
     save_ws_dropped_count ";

const WS_BYTES_RECEIVED_TEMPLATE: &'static str =
    "# HELP save_ws_bytes_received Total bytes received in binary and text WS messages.\n\
     # TYPE save_ws_bytes_received counter\n\
     save_ws_bytes_received ";

const REQ_COUNT_TEMPLATE: &'static str =
    "# HELP save_requests Number of get requests to non WS routes, also excluding metrics route.\n\
     # TYPE save_requests counter\n\
     save_requests ";

impl fmt::Display for Metrics {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}{}\n\
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
            WS_BYTES_RECEIVED_TEMPLATE,
            self.ws_bytes_received.load(Ordering::Relaxed),
            REQ_COUNT_TEMPLATE,
            self.req_count.load(Ordering::Relaxed),
        )
    }
}

pub fn configure(cfg: &mut actix_web::web::ServiceConfig) {
    cfg.service(actix_web::web::scope("/metrics").route("", actix_web::web::get().to(root)));
}

fn root(
    _r: HttpRequest,
    metrics: actix_web::web::Data<Metrics>,
) -> AWResult<HttpResponse, AWError> {
    Ok(HttpResponse::build(StatusCode::OK)
        .content_type("text/plain; version=0.0.4; charset=utf-8")
        .body(metrics.to_string()))
}
