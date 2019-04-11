use actix::prelude::*;
use chrono::NaiveDateTime;
use diesel::sql_types::*;
use diesel::{sql_query, RunQueryDsl};
use failure::Error;
use serde_json::Value;

use super::DbExecutor;

pub enum Query {
    PeerCounts { node_ip: String },
    ListNodes,
}

//pub struct GetTopTenHottestYears;
impl Message for Query {
    type Result = Result<Value, Error>;
}

impl Handler<Query> for DbExecutor {
    type Result = Result<Value, Error>;

    fn handle(&mut self, msg: Query, _: &mut Self::Context) -> Self::Result {
        match msg {
            Query::PeerCounts { node_ip } => self.get_peer_counts(node_ip),
            Query::ListNodes => self.list_nodes(),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, QueryableByName)]
pub struct Node {
    #[sql_type = "Text"]
    node_ip: String,
}

#[derive(Serialize, Deserialize, Debug, QueryableByName)]
pub struct PeerCount {
    #[sql_type = "Timestamp"]
    ts: NaiveDateTime,
    #[sql_type = "Integer"]
    peer_count: i32,
}

impl DbExecutor {
    fn get_peer_counts(&self, node_ip: String) -> Result<Value, Error> {
        let result = self.with_connection(|conn| {
            let query = sql_query(
                "SELECT CAST (logs->>'peers' as INTEGER) as peer_count, \
                 CAST (logs->>'ts' as TIMESTAMP) as ts \
                 FROM substrate_logs \
                 WHERE logs->> 'msg' = 'system.interval' \
                 AND node_ip LIKE $1",
            )
            .bind::<Text, _>(format!("{}%", node_ip));
            let result: diesel::result::QueryResult<Vec<PeerCount>> = query.get_results(conn);
            result.expect("")
        });
        Ok(json!(result.expect("")))
    }

    fn list_nodes(&self) -> Result<Value, Error> {
        let result = self.with_connection(|conn| {
            let query = "SELECT DISTINCT node_ip FROM substrate_logs";
            let result: diesel::result::QueryResult<Vec<Node>> =
                diesel::sql_query(query).get_results(conn);
            result.expect("")
        });
        Ok(json!(result.expect("")))
    }
}
