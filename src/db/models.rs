use crate::schema::substrate_logs;
use chrono::NaiveDateTime;
use serde_json::Value;

#[derive(Queryable, QueryableByName, Identifiable, Serialize, PartialEq, Debug)]
#[table_name = "substrate_logs"]
pub struct SubstrateLog {
    pub id: i32,
    pub created_at: NaiveDateTime,
    pub node_ip: String,
    pub logs: Value,
}

#[derive(Insertable, Debug, Serialize, Deserialize)]
#[table_name = "substrate_logs"]
pub struct NewSubstrateLog {
    pub node_ip: String,
    pub logs: Value,
}

#[derive(Queryable, QueryableByName, Identifiable, Serialize, PartialEq, Debug)]
#[table_name = "peers"]
pub struct Peer {
    pub id: i32,
    pub ip_addr: String,
    pub peer_id: String,
    pub created_at: NaiveDateTime,
}

#[derive(Insertable, Debug, Serialize, Deserialize)]
#[table_name = "peers"]
pub struct NewPeer {
    pub ip_addr: String,
    pub peer_id: String,
}
