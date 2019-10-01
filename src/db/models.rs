use crate::schema::{peer_connections, substrate_logs};
use chrono::NaiveDateTime;
use serde_json::Value;

#[derive(Queryable, QueryableByName, Identifiable, Serialize, PartialEq, Clone, Debug)]
#[table_name = "substrate_logs"]
pub struct SubstrateLog {
    pub id: i32,
    pub created_at: NaiveDateTime,
    pub logs: Value,
    pub peer_connection_id: Option<i32>,
}

#[derive(Insertable, Debug, Serialize, Deserialize)]
#[table_name = "substrate_logs"]
pub struct NewSubstrateLog {
    pub logs: Value,
    pub peer_connection_id: i32,
    pub created_at: NaiveDateTime,
}

#[derive(Queryable, QueryableByName, Identifiable, Serialize, PartialEq, Clone, Debug)]
#[table_name = "peer_connections"]
pub struct PeerConnection {
    pub id: i32,
    pub ip_addr: String,
    pub peer_id: Option<String>,
    pub created_at: NaiveDateTime,
    pub audit: bool,
}

#[derive(Insertable, Debug, Serialize, Deserialize)]
#[table_name = "peer_connections"]
pub struct NewPeerConnection {
    pub ip_addr: String,
    pub peer_id: Option<String>,
    pub audit: bool,
}
