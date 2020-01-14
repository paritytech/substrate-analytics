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
use diesel::RunQueryDsl;
use failure::Error;

pub enum Query {
    All(Filters),
}

impl Message for Query {
    type Result = Result<Vec<Benchmark>, Error>;
}

impl Handler<Query> for DbExecutor {
    type Result = Result<Vec<Benchmark>, Error>;

    fn handle(&mut self, msg: Query, _: &mut Self::Context) -> Self::Result {
        match msg {
            Query::All(filters) => self.get_benchmarks(filters),
        }
    }
}

impl Message for NewBenchmark {
    type Result = Result<Benchmark, Error>;
}

impl Handler<NewBenchmark> for DbExecutor {
    type Result = Result<Benchmark, Error>;

    fn handle(&mut self, msg: NewBenchmark, _: &mut Self::Context) -> Self::Result {
        self.save_benchmark(msg)
    }
}

impl Message for NewBenchmarkEvent {
    type Result = Result<BenchmarkEvent, Error>;
}

impl Handler<NewBenchmarkEvent> for DbExecutor {
    type Result = Result<BenchmarkEvent, Error>;

    fn handle(&mut self, msg: NewBenchmarkEvent, _: &mut Self::Context) -> Self::Result {
        self.save_benchmark_event(msg)
    }
}

impl DbExecutor {
    fn get_benchmarks(&self, _filters: Filters) -> Result<Vec<Benchmark>, Error> {
        match self.with_connection(|conn| {
            use crate::schema::benchmarks::dsl::*;
            benchmarks.load::<Benchmark>(conn)
        }) {
            Ok(Ok(v)) => Ok(v),
            Ok(Err(e)) => Err(e.into()),
            Err(e) => Err(e.into()),
        }
    }

    fn save_benchmark(&self, msg: NewBenchmark) -> Result<Benchmark, Error> {
        match self.with_connection(|conn| {
            use crate::schema::benchmarks;
            diesel::insert_into(benchmarks::table)
                .values(msg)
                .get_result(conn)
        }) {
            Ok(Ok(v)) => Ok(v),
            Ok(Err(e)) => Err(e.into()),
            Err(e) => Err(e.into()),
        }
    }

    fn save_benchmark_event(&self, msg: NewBenchmarkEvent) -> Result<BenchmarkEvent, Error> {
        match self.with_connection(|conn| {
            use crate::schema::benchmark_events;
            diesel::insert_into(benchmark_events::table)
                .values(msg)
                .get_result(conn)
        }) {
            Ok(Ok(v)) => Ok(v),
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
