use crate::db::{
    filters::Filters,
    nodes::{NodeQueryType, NodesQuery},
};

use crate::db::*;
use actix::prelude::*;
use actix_web::{web as a_web, Error as AWError, HttpRequest, HttpResponse};
use futures::Future;

pub fn configure(cfg: &mut a_web::ServiceConfig) {
    cfg.service(
        a_web::scope("/nodes")
            .route("/{node_ip}/peer_counts", a_web::get().to_async(peer_counts))
            .route("/{node_ip}/logs/{msg_type}", a_web::get().to_async(logs))
            .route("/{node_ip}/logs", a_web::get().to_async(all_logs))
            .route("/{node_ip}/log_stats", a_web::get().to_async(log_stats))
            .route("/{node_ip}/log_stats", a_web::get().to_async(log_stats))
            .route("", a_web::get().to_async(all_nodes)),
    );
}

fn all_nodes(
    db: a_web::Data<Addr<DbExecutor>>,
) -> impl Future<Item = HttpResponse, Error = AWError> {
    send_query(NodesQuery::AllNodes, db)
}

fn log_stats(
    req: HttpRequest,
    db: a_web::Data<Addr<DbExecutor>>,
) -> impl Future<Item = HttpResponse, Error = AWError> {
    let node_ip = req
        .match_info()
        .get("node_ip")
        .expect("node_ip should be available because the route matched")
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
            node_ip,
            filters,
            kind: NodeQueryType::LogStats,
        },
        db,
    )
}

fn peer_counts(
    req: HttpRequest,
    db: a_web::Data<Addr<DbExecutor>>,
) -> impl Future<Item = HttpResponse, Error = AWError> {
    let node_ip = req
        .match_info()
        .get("node_ip")
        .expect("node_ip should be available because the route matched")
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
            node_ip,
            filters,
            kind: NodeQueryType::PeerInfo,
        },
        db,
    )
}

fn all_logs(
    req: HttpRequest,
    db: a_web::Data<Addr<DbExecutor>>,
) -> impl Future<Item = HttpResponse, Error = AWError> {
    let node_ip = req
        .match_info()
        .get("node_ip")
        .expect("node_ip should be available because the route matched")
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
            node_ip,
            filters,
            kind: NodeQueryType::AllLogs,
        },
        db,
    )
}

fn logs(
    req: HttpRequest,
    db: a_web::Data<Addr<DbExecutor>>,
) -> impl Future<Item = HttpResponse, Error = AWError> {
    let node_ip = req
        .match_info()
        .get("node_ip")
        .expect("node_ip should be available because the route matched")
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
            node_ip,
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
            error!("Could not complete stats query: {}", e);
            Ok(HttpResponse::InternalServerError().json(json!("Error while processing query")))
        }
    })
}
