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

use crate::schema::{benchmark_events, benchmarks, peer_connections, substrate_logs};
use chrono::NaiveDateTime;
use serde_json::Value;

#[derive(Queryable, Identifiable, PartialEq, Serialize, Debug)]
#[table_name = "benchmark_events"]
pub struct BenchmarkEvent {
    pub id: i32,
    pub benchmark_id: i32,
    pub name: String,
    pub phase: String,
    pub created_at: NaiveDateTime,
}

#[derive(Insertable, Debug, Serialize, Deserialize)]
#[table_name = "benchmark_events"]
pub struct NewBenchmarkEvent {
    pub benchmark_id: i32,
    pub name: String,
    pub phase: String,
    pub created_at: NaiveDateTime,
}

#[derive(Queryable, Identifiable, PartialEq, Serialize, Debug)]
#[table_name = "benchmarks"]
pub struct Benchmark {
    pub id: i32,
    pub setup: Value,
    pub created_at: NaiveDateTime,
}

#[derive(Insertable, Debug, Serialize, Deserialize)]
#[table_name = "benchmarks"]
pub struct NewBenchmark {
    pub setup: Value,
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
