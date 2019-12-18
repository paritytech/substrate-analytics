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

use super::get_filters;
use super::metrics::Metrics;
use crate::db::{benchmarks::*, models::*, DbExecutor};
use actix::prelude::*;
use actix_web::{HttpRequest, HttpResponse};
use futures::{Future, IntoFuture};

pub fn configure(cfg: &mut actix_web::web::ServiceConfig) {
    cfg.service(
        actix_web::web::scope("/benchmarks")
            .route("/example", actix_web::web::get().to_async(example))
            .route("/events", actix_web::web::post().to_async(new_event))
            .route("", actix_web::web::get().to_async(all))
            .route("", actix_web::web::post().to_async(new)),
    );
}

fn all(
    req: HttpRequest,
    db: actix_web::web::Data<Addr<DbExecutor>>,
    metrics: actix_web::web::Data<Metrics>,
) -> impl Future<Item = HttpResponse, Error = actix_web::Error> {
    metrics.inc_req_count();
    let filters = get_filters(&req);
    db.send(Query::All(filters))
        .from_err()
        .and_then(move |res| match res {
            Ok(r) => Ok(HttpResponse::Ok().json(json!(r))),
            Err(e) => {
                error!("Could not complete benchmarks query: {:?}", e);
                Ok(HttpResponse::InternalServerError().json(json!("Error while processing query")))
            }
        })
}

fn new(
    item: actix_web::web::Json<NewBenchmark>,
    db: actix_web::web::Data<Addr<DbExecutor>>,
    metrics: actix_web::web::Data<Metrics>,
) -> impl Future<Item = HttpResponse, Error = actix_web::Error> {
    metrics.inc_req_count();
    db.send(item.into_inner())
        .from_err()
        .and_then(move |res| match res {
            Ok(r) => Ok(HttpResponse::Ok().json(json!(r))),
            Err(e) => {
                error!("Could not create New Benchmark: {:?}", e);
                Ok(HttpResponse::InternalServerError().json(json!({"error": e.to_string()})))
            }
        })
}

fn new_event(
    item: actix_web::web::Json<NewBenchmarkEvent>,
    db: actix_web::web::Data<Addr<DbExecutor>>,
    metrics: actix_web::web::Data<Metrics>,
) -> impl Future<Item = HttpResponse, Error = actix_web::Error> {
    metrics.inc_req_count();
    db.send(item.into_inner())
        .from_err()
        .and_then(move |res| match res {
            Ok(r) => Ok(HttpResponse::Ok().json(json!(r))),
            Err(e) => {
                error!("Could not create New Benchmark: {:?}", e);
                Ok(HttpResponse::InternalServerError().json(json!({"error": e.to_string()})))
            }
        })
}

fn example(
    metrics: actix_web::web::Data<Metrics>,
) -> impl Future<Item = HttpResponse, Error = actix_web::Error> {
    metrics.inc_req_count();
    actix_web::web::HttpResponse::Ok()
        .json(NewBenchmark::example())
        .into_future()
}
