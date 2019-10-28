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
use crate::db::{reputation::Query, DbExecutor};
use actix::prelude::*;
use actix_web::{HttpRequest, HttpResponse};
use futures::Future;
//use rand::seq::index::IndexVec::USize;

pub fn configure(cfg: &mut actix_web::web::ServiceConfig) {
    cfg.service(
        actix_web::web::scope("/reputation")
            .route("/logged", actix_web::web::get().to_async(logged))
            .route("/mock/{qty}", actix_web::web::get().to_async(mock))
            .route("/{peer_id}", actix_web::web::get().to_async(single))
            .route("", actix_web::web::get().to_async(all)),
    );
}

fn logged(
    req: HttpRequest,
    db: actix_web::web::Data<Addr<DbExecutor>>,
    metrics: actix_web::web::Data<Metrics>,
) -> impl Future<Item = HttpResponse, Error = actix_web::Error> {
    metrics.inc_req_count();
    let filters = get_filters(&req);
    db.send(Query::Logged(filters))
        .from_err()
        .and_then(move |res| match res {
            Ok(r) => Ok(HttpResponse::Ok().json(json!(r))),
            Err(e) => {
                error!("Could not complete stats query: {:?}", e);
                Ok(HttpResponse::InternalServerError().json(json!("Error while processing query")))
            }
        })
}

fn single(
    req: HttpRequest,
    db: actix_web::web::Data<Addr<DbExecutor>>,
    metrics: actix_web::web::Data<Metrics>,
) -> impl Future<Item = HttpResponse, Error = actix_web::Error> {
    metrics.inc_req_count();
    let peer_id = req
        .match_info()
        .get("peer_id")
        .expect("peer_id should be available because the route matched")
        .to_string();
    let filters = get_filters(&req);
    db.send(Query::Selected(vec![peer_id], filters))
        .from_err()
        .and_then(move |res| match res {
            Ok(r) => Ok(HttpResponse::Ok().json(json!(r))),
            Err(e) => {
                error!("Could not complete stats query: {:?}", e);
                Ok(HttpResponse::InternalServerError().json(json!("Error while processing query")))
            }
        })
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
                error!("Could not complete stats query: {:?}", e);
                Ok(HttpResponse::InternalServerError().json(json!("Error while processing query")))
            }
        })
}

fn mock(
    req: HttpRequest,
    db: actix_web::web::Data<Addr<DbExecutor>>,
    metrics: actix_web::web::Data<Metrics>,
) -> impl Future<Item = HttpResponse, Error = actix_web::Error> {
    metrics.inc_req_count();
    let qty: usize = match req
        .match_info()
        .get("qty")
        .expect("qty should be available because the route matched")
        .to_string()
        .parse()
    {
        Ok(v) => v,
        _ => std::usize::MAX,
    };
    db.send(Query::Mock(qty))
        .from_err()
        .and_then(move |res| match res {
            Ok(r) => Ok(HttpResponse::Ok().json(json!(r))),
            Err(e) => {
                error!("Could not complete stats query: {:?}", e);
                Ok(HttpResponse::InternalServerError().json(json!("Error while processing query")))
            }
        })
}
