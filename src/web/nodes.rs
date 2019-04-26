use super::State;
use crate::db::{
    filters::Filters,
    nodes::{NodeQueryType, NodesQuery},
};

use actix_web::{http::Method, App, AsyncResponder, FutureResponse, HttpRequest, HttpResponse};
use futures::{future::ok as fut_ok, Future};

pub fn configure(app: App<State>) -> App<State> {
    app.scope("nodes", |scope| {
        scope
            .resource("/{node_ip}/peer_counts", |r| {
                r.method(Method::GET).f(peer_counts)
            })
            .resource("/{node_ip}/logs/{msg_type}", |r| {
                r.method(Method::GET).f(logs)
            })
            .resource("/{node_ip}/logs", |r| r.method(Method::GET).f(all_logs))
            .resource("/{node_ip}/log_stats", |r| {
                r.method(Method::GET).f(log_stats)
            })
            .resource("", |r| {
                r.method(Method::GET)
                    .f(|req| send_query(req, NodesQuery::AllNodes))
            })
    })
}

fn log_stats(req: &HttpRequest<State>) -> FutureResponse<HttpResponse> {
    let node_ip = req
        .match_info()
        .get("node_ip")
        .expect("node_ip should be available because the route matched")
        .to_string();
    match Filters::from_hashmap(req.query()) {
        Ok(filters) => send_query(
            req,
            NodesQuery::Node {
                node_ip,
                filters,
                kind: NodeQueryType::LogStats,
            },
        ),
        Err(errors) => {
            debug!("Error parsing peer count params - {:?}", errors);
            Box::new(fut_ok(
                HttpResponse::BadRequest().json(json!(errors)).into(),
            ))
        }
    }
}

fn peer_counts(req: &HttpRequest<State>) -> FutureResponse<HttpResponse> {
    let node_ip = req
        .match_info()
        .get("node_ip")
        .expect("node_ip should be available because the route matched")
        .to_string();
    match Filters::from_hashmap(req.query()) {
        Ok(filters) => send_query(
            req,
            NodesQuery::Node {
                node_ip,
                filters,
                kind: NodeQueryType::PeerInfo,
            },
        ),
        Err(errors) => {
            debug!("Error parsing peer count params - {:?}", errors);
            Box::new(fut_ok(
                HttpResponse::BadRequest().json(json!(errors)).into(),
            ))
        }
    }
}

fn all_logs(req: &HttpRequest<State>) -> FutureResponse<HttpResponse> {
    let node_ip = req
        .match_info()
        .get("node_ip")
        .expect("node_ip should be available because the route matched")
        .to_string();
    match Filters::from_hashmap(req.query()) {
        Ok(filters) => send_query(
            req,
            NodesQuery::Node {
                node_ip,
                filters,
                kind: NodeQueryType::AllLogs,
            },
        ),
        Err(errors) => {
            debug!("Error parsing peer count params - {:?}", errors);
            Box::new(fut_ok(
                HttpResponse::BadRequest().json(json!(errors)).into(),
            ))
        }
    }
}

fn logs(req: &HttpRequest<State>) -> FutureResponse<HttpResponse> {
    let node_ip = req
        .match_info()
        .get("node_ip")
        .expect("node_ip should be available because the route matched")
        .to_string();
    let msg_type = req
        .match_info()
        .get("msg_type")
        .expect("msg_type should be available because the route matched")
        .to_string();
    match Filters::from_hashmap(req.query()) {
        Ok(filters) => send_query(
            req,
            NodesQuery::Node {
                node_ip,
                filters,
                kind: NodeQueryType::Logs(msg_type),
            },
        ),
        Err(errors) => {
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
