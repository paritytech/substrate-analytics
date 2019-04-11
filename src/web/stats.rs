use super::State;
use crate::db::stats::Query;

use actix_web::{http::Method, App, AsyncResponder, FutureResponse, HttpRequest, HttpResponse};
use futures::{future::ok as fut_ok, Future};

pub fn configure(app: App<State>) -> App<State> {
    app.scope("stats", |scope| {
        scope
            .resource("/peer_history/{node_ip}", |r| {
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
            .resource("/peer_list", |r| {
                r.method(Method::GET)
                    .f(|req| stats_query(req, Query::ListNodes))
            })
    })
}

fn stats_query(req: &HttpRequest<State>, query: Query) -> FutureResponse<HttpResponse> {
    req.state()
        .db
        .send(query)
        .from_err()
        .and_then(move |res| fut_ok(res.expect("Couldn't unwrap res"))) // TODO deal with this
        .and_then(|res| Ok(HttpResponse::Ok().json(res)))
        .responder()
}
