use actix_web::{
    http::StatusCode, web as a_web, Error as AWError, HttpRequest, HttpResponse, Result as AWResult,
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
        self.ws_message_count.fetch_add(1, Ordering::SeqCst);
    }
    pub fn inc_ws_connected_count(&self) {
        self.ws_connected_count.fetch_add(1, Ordering::SeqCst);
    }
    pub fn inc_ws_dropped_count(&self) {
        self.ws_dropped_count.fetch_add(1, Ordering::SeqCst);
    }
    pub fn inc_ws_bytes_received(&self, n: u64) {
        self.ws_bytes_received.fetch_add(n, Ordering::SeqCst);
    }
    pub fn inc_req_count(&self) {
        self.req_count.fetch_add(1, Ordering::SeqCst);
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

pub fn configure(cfg: &mut a_web::ServiceConfig) {
    cfg.service(a_web::scope("/metrics").route("", a_web::get().to(root)));
}

fn root(_r: HttpRequest, metrics: a_web::Data<Metrics>) -> AWResult<HttpResponse, AWError> {
    Ok(HttpResponse::build(StatusCode::OK)
        .content_type("text/plain; version=0.0.4; charset=utf-8")
        .body(metrics.to_string()))
}
