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

use crate::schema::{benchmarks, host_systems, peer_connections, substrate_logs};
use chrono::NaiveDateTime;
use serde_json::Value;

#[derive(Queryable, Identifiable, PartialEq, Serialize, Debug)]
#[table_name = "benchmarks"]
pub struct Benchmark {
    pub id: i32,
    pub ts_start: NaiveDateTime,
    pub ts_end: NaiveDateTime,
    pub description: Option<String>,
    pub chain_spec: Option<Value>,
    pub benchmark_spec: Option<Value>,
    pub host_system_id: i32,
}

#[derive(Insertable, Debug, Serialize, Deserialize)]
#[table_name = "benchmarks"]
pub struct NewBenchmark {
    pub ts_start: NaiveDateTime,
    pub description: Option<String>,
    pub chain_spec: Option<Value>,
    pub benchmark_spec: Option<Value>,
    pub host_system_id: i32,
}

#[derive(Insertable, Debug, Serialize, Deserialize)]
#[table_name = "host_systems"]
pub struct NewHostSystem {
    pub description: String,
    pub os: String,
    pub cpu_qty: i32,
    pub cpu_clock: i32,
    pub ram_mb: i32,
    pub disk_info: String,
}

/// HostSystem is a way to indentify the hardware that the node
/// is running on.
#[derive(Queryable, Identifiable, PartialEq, Serialize, Debug)]
#[table_name = "host_systems"]
pub struct HostSystem {
    pub id: i32,
    pub description: String,
    pub os: String,
    pub cpu_qty: i32,
    pub cpu_clock: i32,
    pub ram_mb: i32,
    pub disk_info: String,
}

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
