use crate::db::{stats::Query, DbExecutor};
use actix::prelude::*;
use actix_web::{
    http::StatusCode, web as a_web, Error as AWError, HttpResponse, Result as AWResult,
};
use futures::Future;

lazy_static! {
    static ref VERSION_INFO: String = format!(
        "{}<br\\>{}<br\\>{}",
        env!("CARGO_PKG_NAME"),
        env!("CARGO_PKG_VERSION"),
        env!("CARGO_PKG_DESCRIPTION")
    );
}

pub fn configure(cfg: &mut a_web::ServiceConfig) {
    cfg.service(
        a_web::scope("/stats")
            .route("/db", a_web::get().to_async(send_query))
            .route("/version", a_web::get().to(version)),
    );
}

fn send_query(
    db: a_web::Data<Addr<DbExecutor>>,
) -> impl Future<Item = HttpResponse, Error = AWError> {
    db.send(Query::Db)
        .from_err()
        .and_then(move |res| match res {
            Ok(r) => Ok(HttpResponse::Ok().json(r)),
            Err(e) => {
                error!("Could not complete stats query: {}", e);
                Ok(HttpResponse::InternalServerError().json(json!("Error while processing query")))
            }
        })
}

fn version() -> AWResult<HttpResponse> {
    Ok(HttpResponse::build(StatusCode::OK)
        .content_type("text/html; charset=utf-8")
        .body(&*VERSION_INFO))
}
