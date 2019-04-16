use super::State;
use crate::db::stats::Query;

use actix_web::{http::Method, App, AsyncResponder, FutureResponse, HttpRequest, HttpResponse};
use futures::Future;

pub fn configure(app: App<State>) -> App<State> {
    app.scope("stats", |scope| {
        scope.resource("/db", |r| {
            r.method(Method::GET).f(|req| send_query(req, Query::Db))
        })
    })
}

fn send_query(req: &HttpRequest<State>, query: Query) -> FutureResponse<HttpResponse> {
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
