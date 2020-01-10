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
use crate::db::nodes::{NodeQueryType, NodesQuery};
use crate::db::*;
use actix::prelude::*;
use actix_web::{HttpRequest, HttpResponse};

pub fn configure(cfg: &mut actix_web::web::ServiceConfig) {
    cfg.service(
        actix_web::web::scope("/nodes")
            .route(
                "/{peer_id}/peer_counts",
                actix_web::web::get().to(peer_counts),
            )
            .route("/{peer_id}/logs/{msg_type}", actix_web::web::get().to(logs))
            .route("/{peer_id}/logs", actix_web::web::get().to(all_logs))
            .route("/{peer_id}/log_stats", actix_web::web::get().to(log_stats))
            .route("", actix_web::web::get().to(all_nodes)),
    );
}

async fn all_nodes(
    db: actix_web::web::Data<Addr<DbExecutor>>,
    metrics: actix_web::web::Data<Metrics>,
) -> Result<HttpResponse, actix_web::Error> {
    metrics.inc_req_count();
    send_query(NodesQuery::AllNodes, db).await
}

async fn log_stats(
    req: HttpRequest,
    db: actix_web::web::Data<Addr<DbExecutor>>,
    metrics: actix_web::web::Data<Metrics>,
) -> Result<HttpResponse, actix_web::Error> {
    metrics.inc_req_count();
    let peer_id = req
        .match_info()
        .get("peer_id")
        .expect("peer_id should be available because the route matched")
        .to_string();
    let filters = get_filters(&req);
    send_query(
        NodesQuery::Node {
            peer_id,
            filters,
            kind: NodeQueryType::LogStats,
        },
        db,
    )
    .await
}

async fn peer_counts(
    req: HttpRequest,
    db: actix_web::web::Data<Addr<DbExecutor>>,
    metrics: actix_web::web::Data<Metrics>,
) -> Result<HttpResponse, actix_web::Error> {
    metrics.inc_req_count();
    let peer_id = req
        .match_info()
        .get("peer_id")
        .expect("peer_id should be available because the route matched")
        .to_string();
    let filters = get_filters(&req);
    send_query(
        NodesQuery::Node {
            peer_id,
            filters,
            kind: NodeQueryType::PeerInfo,
        },
        db,
    )
    .await
}

async fn all_logs(
    req: HttpRequest,
    db: actix_web::web::Data<Addr<DbExecutor>>,
    metrics: actix_web::web::Data<Metrics>,
) -> Result<HttpResponse, actix_web::Error> {
    metrics.inc_req_count();
    let peer_id = req
        .match_info()
        .get("peer_id")
        .expect("peer_id should be available because the route matched")
        .to_string();
    let filters = get_filters(&req);
    send_query(
        NodesQuery::Node {
            peer_id,
            filters,
            kind: NodeQueryType::AllLogs,
        },
        db,
    )
    .await
}

async fn logs(
    req: HttpRequest,
    db: actix_web::web::Data<Addr<DbExecutor>>,
    metrics: actix_web::web::Data<Metrics>,
) -> Result<HttpResponse, actix_web::Error> {
    metrics.inc_req_count();
    let peer_id = req
        .match_info()
        .get("peer_id")
        .expect("peer_id should be available because the route matched")
        .to_string();
    let msg_type = req
        .match_info()
        .get("msg_type")
        .expect("msg_type should be available because the route matched")
        .to_string();
    let filters = get_filters(&req);
    send_query(
        NodesQuery::Node {
            peer_id,
            filters,
            kind: NodeQueryType::Logs(msg_type),
        },
        db,
    )
    .await
}

async fn send_query(
    query: NodesQuery,
    db: actix_web::web::Data<Addr<DbExecutor>>,
) -> Result<HttpResponse, actix_web::Error> {
    let res = db.send(query).await?;
    match res {
        Ok(r) => Ok(HttpResponse::Ok().json(r)),
        Err(e) => {
            error!("Could not complete query: {}", e);
            Ok(HttpResponse::InternalServerError().json(json!("Error while processing query")))
        }
    }
}
