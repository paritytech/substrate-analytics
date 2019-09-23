use super::metrics::Metrics;
use crate::db::*;
use crate::db::{
    filters::Filters,
    nodes::{NodeQueryType, NodesQuery},
};
use actix::prelude::*;
use actix_web::{web as a_web, Error as AWError, HttpRequest, HttpResponse};
use futures::Future;

pub fn configure(cfg: &mut a_web::ServiceConfig) {
    cfg.service(
        a_web::scope("/nodes")
            .route("/{peer_id}/peer_counts", a_web::get().to_async(peer_counts))
            .route("/{peer_id}/logs/{msg_type}", a_web::get().to_async(logs))
            .route("/{peer_id}/logs", a_web::get().to_async(all_logs))
            .route("/{peer_id}/log_stats", a_web::get().to_async(log_stats))
            .route("/{peer_id}/log_stats", a_web::get().to_async(log_stats))
            .route("", a_web::get().to_async(all_nodes)),
    );
}

fn all_nodes(
    db: a_web::Data<Addr<DbExecutor>>,
    metrics: a_web::Data<Metrics>,
) -> impl Future<Item = HttpResponse, Error = AWError> {
    metrics.inc_req_count();
    send_query(NodesQuery::AllNodes, db)
}

fn log_stats(
    req: HttpRequest,
    db: a_web::Data<Addr<DbExecutor>>,
    metrics: a_web::Data<Metrics>,
) -> impl Future<Item = HttpResponse, Error = AWError> {
    metrics.inc_req_count();
    let peer_id = req
        .match_info()
        .get("peer_id")
        .expect("peer_id should be available because the route matched")
        .to_string();
    let filters = match a_web::Query::<Filters>::from_query(&req.query_string()) {
        Ok(f) => f.clone(),
        Err(_) => {
            warn!("Error deserializing Filters from querystring");
            Filters::default()
        }
    };
    send_query(
        NodesQuery::Node {
            peer_id,
            filters,
            kind: NodeQueryType::LogStats,
        },
        db,
    )
}

fn peer_counts(
    req: HttpRequest,
    db: a_web::Data<Addr<DbExecutor>>,
    metrics: a_web::Data<Metrics>,
) -> impl Future<Item = HttpResponse, Error = AWError> {
    metrics.inc_req_count();
    let peer_id = req
        .match_info()
        .get("peer_id")
        .expect("peer_id should be available because the route matched")
        .to_string();
    let filters = match a_web::Query::<Filters>::from_query(&req.query_string()) {
        Ok(f) => f.clone(),
        Err(_) => {
            warn!("Error deserializing Filters from querystring");
            Filters::default()
        }
    };
    send_query(
        NodesQuery::Node {
            peer_id,
            filters,
            kind: NodeQueryType::PeerInfo,
        },
        db,
    )
}

fn all_logs(
    req: HttpRequest,
    db: a_web::Data<Addr<DbExecutor>>,
    metrics: a_web::Data<Metrics>,
) -> impl Future<Item = HttpResponse, Error = AWError> {
    metrics.inc_req_count();
    let peer_id = req
        .match_info()
        .get("peer_id")
        .expect("peer_id should be available because the route matched")
        .to_string();
    let filters = match a_web::Query::<Filters>::from_query(&req.query_string()) {
        Ok(f) => f.clone(),
        Err(_) => {
            warn!("Error deserializing Filters from querystring");
            Filters::default()
        }
    };
    send_query(
        NodesQuery::Node {
            peer_id,
            filters,
            kind: NodeQueryType::AllLogs,
        },
        db,
    )
}

fn logs(
    req: HttpRequest,
    db: a_web::Data<Addr<DbExecutor>>,
    metrics: a_web::Data<Metrics>,
) -> impl Future<Item = HttpResponse, Error = AWError> {
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
    let query = a_web::Query::<Filters>::from_query(&req.query_string());
    let filters = match query {
        Ok(f) => f.clone(),
        Err(_) => {
            warn!("Error deserializing Filters from querystring");
            Filters::default()
        }
    };
    send_query(
        NodesQuery::Node {
            peer_id,
            filters,
            kind: NodeQueryType::Logs(msg_type),
        },
        db,
    )
}

fn send_query(
    query: NodesQuery,
    db: a_web::Data<Addr<DbExecutor>>,
) -> impl Future<Item = HttpResponse, Error = AWError> {
    db.send(query).from_err().and_then(move |res| match res {
        Ok(r) => Ok(HttpResponse::Ok().json(r)),
        Err(e) => {
            error!("Could not complete query: {}", e);
            Ok(HttpResponse::InternalServerError().json(json!("Error while processing query")))
        }
    })
}
