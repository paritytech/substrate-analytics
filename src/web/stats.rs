use crate::db::{stats::Query, DbExecutor};
use actix::prelude::*;
use actix_web::{web as a_web, Error as AWError, HttpResponse};
use futures::Future;

pub fn configure(cfg: &mut a_web::ServiceConfig) {
    cfg.service(
        a_web::scope("stats")
            .service(a_web::resource("/db").route(a_web::get().to_async(send_query))),
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
