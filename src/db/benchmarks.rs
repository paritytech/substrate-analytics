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

use super::models::{Benchmark, BenchmarkEvent, NewBenchmark, NewBenchmarkEvent};
use super::DbExecutor;
use crate::db::filters::Filters;
use actix::prelude::*;
use diesel::prelude::*;
use diesel::sql_query;
use diesel::sql_types::*;
use failure::Error;
use serde_json::Value;

#[derive(Serialize, Deserialize, Debug, QueryableByName)]
pub struct Targets {
    #[sql_type = "Text"]
    target: String,
}

pub enum Query {
    All(Filters),
    /// Targets for benchmark id
    Targets(i32),
    /// Events for benchmark id
    Events(i32),
}

impl Message for Query {
    type Result = Result<Value, Error>;
}

impl Handler<Query> for DbExecutor {
    type Result = Result<Value, Error>;

    fn handle(&mut self, msg: Query, _: &mut Self::Context) -> Self::Result {
        match msg {
            Query::All(filters) => self.get_benchmarks(filters),
            Query::Events(id) => self.get_events(id),
            Query::Targets(id) => self.get_targets(id),
        }
    }
}

impl Message for NewBenchmark {
    type Result = Result<Value, Error>;
}

impl Handler<NewBenchmark> for DbExecutor {
    type Result = Result<Value, Error>;

    fn handle(&mut self, msg: NewBenchmark, _: &mut Self::Context) -> Self::Result {
        self.save_benchmark(msg)
    }
}

impl Message for NewBenchmarkEvent {
    type Result = Result<Value, Error>;
}

impl Handler<NewBenchmarkEvent> for DbExecutor {
    type Result = Result<Value, Error>;

    fn handle(&mut self, msg: NewBenchmarkEvent, _: &mut Self::Context) -> Self::Result {
        self.save_benchmark_event(msg)
    }
}

impl DbExecutor {
    fn get_benchmarks(&self, _filters: Filters) -> Result<Value, Error> {
        match self.with_connection(|conn| {
            use crate::schema::benchmarks::dsl::*;
            benchmarks.order(created_at.desc()).load::<Benchmark>(conn)
        }) {
            Ok(Ok(v)) => Ok(json!(v)),
            Ok(Err(e)) => Err(e.into()),
            Err(e) => Err(e.into()),
        }
    }

    fn get_targets(&self, id: i32) -> Result<Value, Error> {
        match self.with_connection(|conn| {
            let query = sql_query(
                "SELECT DISTINCT logs->>'target' as target from substrate_logs \
                WHERE logs->>'target' IS NOT NULL \
                AND peer_connection_id = ANY (\
                    SELECT id from peer_connections WHERE peer_id = \
                        (SELECT setup->'substrate'->>'peerId' as peer_id FROM benchmarks WHERE id = $1)\
                ) ORDER BY target ASC",
            )
            .bind::<Integer, _>(id);
            debug!(
                "get_targets query: {}",
                diesel::debug_query::<diesel::pg::Pg, _>(&query)
            );
            let result: QueryResult<Vec<Targets>> = query.get_results(conn);
            result
        }) {
            Ok(Ok(v)) => Ok(json!(v)),
            Ok(Err(e)) => Err(e.into()),
            Err(e) => Err(e.into()),
        }
    }

    fn get_events(&self, bm_id: i32) -> Result<Value, Error> {
        match self.with_connection(|conn| {
            use crate::schema::benchmark_events::dsl::*;
            benchmark_events
                .filter(benchmark_id.eq(bm_id))
                .order(name.asc())
                .load::<BenchmarkEvent>(conn)
        }) {
            Ok(Ok(v)) => Ok(json!(v)),
            Ok(Err(e)) => Err(e.into()),
            Err(e) => Err(e.into()),
        }
    }

    fn save_benchmark(&self, msg: NewBenchmark) -> Result<Value, Error> {
        match self.with_connection(|conn| {
            use crate::schema::benchmarks;
            diesel::insert_into(benchmarks::table)
                .values(msg)
                .get_result::<Benchmark>(conn)
        }) {
            Ok(Ok(v)) => Ok(json!(v)),
            Ok(Err(e)) => Err(e.into()),
            Err(e) => Err(e.into()),
        }
    }

    fn save_benchmark_event(&self, msg: NewBenchmarkEvent) -> Result<Value, Error> {
        match self.with_connection(|conn| {
            use crate::schema::benchmark_events;
            diesel::insert_into(benchmark_events::table)
                .values(msg)
                .get_result::<BenchmarkEvent>(conn)
        }) {
            Ok(Ok(v)) => Ok(json!(v)),
            Ok(Err(e)) => Err(e.into()),
            Err(e) => Err(e.into()),
        }
    }
}

impl NewBenchmark {
    pub fn example() -> Self {
        NewBenchmark {
            setup: serde_json::Value::default(),
        }
    }
}
