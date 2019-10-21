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

use super::metrics::Metrics;
use crate::db::{stats::Query, DbExecutor};
use actix::prelude::*;
use actix_web::{http::StatusCode, HttpResponse, Result as AWResult};
use futures::Future;

lazy_static! {
    static ref VERSION_INFO: String = format!(
        "{}<br\\>{}<br\\>{}",
        env!("CARGO_PKG_NAME"),
        env!("CARGO_PKG_VERSION"),
        env!("CARGO_PKG_DESCRIPTION")
    );
}

pub fn configure(cfg: &mut actix_web::web::ServiceConfig) {
    cfg.service(
        actix_web::web::scope("/stats")
            .route("/db", actix_web::web::get().to_async(send_query))
            .route("/version", actix_web::web::get().to(version)),
    );
}

fn send_query(
    db: actix_web::web::Data<Addr<DbExecutor>>,
    metrics: actix_web::web::Data<Metrics>,
) -> impl Future<Item = HttpResponse, Error = actix_web::Error> {
    metrics.inc_req_count();
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

fn version(metrics: actix_web::web::Data<Metrics>) -> AWResult<HttpResponse> {
    metrics.inc_req_count();
    Ok(HttpResponse::build(StatusCode::OK)
        .content_type("text/html; charset=utf-8")
        .body(&*VERSION_INFO))
}
