use super::State;
use crate::db::nodes::NodesQuery;
use chrono::NaiveDateTime;

use actix_web::{http::Method, App, AsyncResponder, FutureResponse, HttpRequest, HttpResponse};
use futures::{future::ok as fut_ok, Future};

const START_TIME: &'static str = "2000-01-01T00:00:00";
const END_TIME: &'static str = "9999-01-01T00:00:00";

lazy_static! {
    pub static ref DEFAULT_DATE_TIME: NaiveDateTime = NaiveDateTime::from_timestamp(1, 1);
}

pub fn configure(app: App<State>) -> App<State> {
    app.scope("nodes", |scope| {
        scope
            .resource("/{node_ip}/peer_counts", |r| {
                r.method(Method::GET).f(peer_counts)
            })
            .resource("/{node_ip}/recent_logs", |r| {
                r.method(Method::GET).f(recent_logs)
            })
            .resource("", |r| {
                r.method(Method::GET)
                    .f(|req| send_query(req, NodesQuery::Nodes))
            })
    })
}

fn peer_counts(req: &HttpRequest<State>) -> FutureResponse<HttpResponse> {
    let mut errors = Vec::new();
    let node_ip = req
        .match_info()
        .get("node_ip")
        .expect("node_ip should be available because the route matched")
        .to_string();
    let start_time: NaiveDateTime = req
        .query()
        .get("start_time")
        .unwrap_or(&START_TIME.to_string())
        .parse()
        .unwrap_or_else(|_e| {
            errors.push(format!(
                "Unable to parse 'start_time', please use the format: {}",
                START_TIME
            ));
            *DEFAULT_DATE_TIME
        });
    let end_time: NaiveDateTime = req
        .query()
        .get("end_time")
        .unwrap_or(&END_TIME.to_string())
        .parse()
        .unwrap_or_else(|_e| {
            errors.push(format!(
                "Unable to parse 'end_time', please use the format: {}",
                END_TIME
            ));
            *DEFAULT_DATE_TIME
        });
    match errors.is_empty() {
        true => send_query(
            req,
            NodesQuery::PeerCounts {
                node_ip,
                start_time,
                end_time,
            },
        ),
        false => {
            debug!("Error parsing peer count params - {:?}", errors);
            Box::new(fut_ok(
                HttpResponse::BadRequest().json(json!(errors)).into(),
            ))
        }
    }
}

fn recent_logs(req: &HttpRequest<State>) -> FutureResponse<HttpResponse> {
    let mut errors = Vec::new();
    let node_ip = req
        .match_info()
        .get("node_ip")
        .expect("node_ip should be available because the route matched")
        .to_string();
    let start_time: NaiveDateTime = req
        .query()
        .get("start_time")
        .unwrap_or(&START_TIME.to_string())
        .parse()
        .unwrap_or_else(|_e| {
            errors.push(format!(
                "Unable to parse 'start_time', please use the format: {}",
                START_TIME
            ));
            *DEFAULT_DATE_TIME
        });
    let end_time: NaiveDateTime = req
        .query()
        .get("end_time")
        .unwrap_or(&END_TIME.to_string())
        .parse()
        .unwrap_or_else(|_e| {
            errors.push(format!(
                "Unable to parse 'end_time', please use the format: {}",
                END_TIME
            ));
            *DEFAULT_DATE_TIME
        });
    let limit: Option<i32> = match req.query().get("limit") {
        Some(limit) => match limit.parse() {
            Ok(l) => Some(l),
            Err(_) => {
                errors.push("Unable to parse 'limit', please use the format: 100".to_string());
                None
            }
        },
        None => None,
    };
    match errors.is_empty() {
        true => send_query(
            req,
            NodesQuery::RecentLogs {
                node_ip,
                start_time,
                end_time,
                limit,
            },
        ),
        false => {
            debug!("Error parsing peer count params - {:?}", errors);
            Box::new(fut_ok(
                HttpResponse::BadRequest().json(json!(errors)).into(),
            ))
        }
    }
}

fn send_query(req: &HttpRequest<State>, query: NodesQuery) -> FutureResponse<HttpResponse> {
    req.state()
        .db
        .send(query)
        .from_err()
        .and_then(move |res| match res {
            Ok(r) => Ok(HttpResponse::Ok().json(r)),
            Err(e) => {
                error!("Could not complete stats query: {}", e);
                Ok(HttpResponse::InternalServerError().json(json!("Error while processing query")))
            }
        })
        .responder()
}
