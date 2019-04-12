use super::State;
use crate::db::stats::Query;

use actix_web::{http::Method, App, AsyncResponder, FutureResponse, HttpRequest, HttpResponse};
use futures::{future::ok as fut_ok, Future};

pub fn configure(app: App<State>) -> App<State> {
    app.scope("stats", |scope| {
        scope
            .resource("/nodes/{node_ip}/peer_counts", |r| {
                r.method(Method::GET).f(|req| {
                    stats_query(
                        req,
                        Query::PeerCounts {
                            node_ip: req
                                .match_info()
                                .get("node_ip")
                                .expect("node_ip should be available because the route matched")
                                .to_string(),
                        },
                    )
                })
            })
            .resource("/nodes", |r| {
                r.method(Method::GET)
                    .f(|req| stats_query(req, Query::Nodes))
            })
    })
}

fn stats_query(req: &HttpRequest<State>, query: Query) -> FutureResponse<HttpResponse> {
    req.state()
        .db
        .send(query)
        .from_err()
        .and_then(move |res| {
            let res = match res {
                Ok(r) => r,
                Err(e) => {
                    error!("Could not complete stats query: {}", e);
                    json!("Error while processing query")
                }
            };
            fut_ok(res)
        })
        .and_then(|res| Ok(HttpResponse::Ok().json(res)))
        .responder()
}
