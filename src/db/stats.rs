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

use actix::prelude::*;
use diesel::sql_types::*;
use diesel::{result::QueryResult, RunQueryDsl};
use failure::Error;
use serde_json::Value;

use super::DbExecutor;

/// Message to indicate what information is required
/// Response is always json
pub enum Query {
    Db,
}

impl Message for Query {
    type Result = Result<Value, Error>;
}

impl Handler<Query> for DbExecutor {
    type Result = Result<Value, Error>;

    fn handle(&mut self, msg: Query, _: &mut Self::Context) -> Self::Result {
        match msg {
            Query::Db => self.get_db_size(),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, QueryableByName)]
pub struct DbSize {
    #[sql_type = "Text"]
    relation: String,
    #[sql_type = "Text"]
    size: String,
}

impl DbExecutor {
    fn get_db_size(&self) -> Result<Value, Error> {
        match self.with_connection(|conn| {
            let query = "SELECT nspname || '.' || relname AS relation, \
                         pg_size_pretty(pg_relation_size(C.oid)) AS size \
                         FROM pg_class C \
                         LEFT JOIN pg_namespace N ON (N.oid = C.relnamespace) \
                         WHERE nspname NOT IN ('pg_catalog', 'information_schema') \
                         ORDER BY pg_relation_size(C.oid) DESC \
                         LIMIT 1000;";
            let result: QueryResult<Vec<DbSize>> = diesel::sql_query(query).get_results(conn);
            result
        }) {
            Ok(Ok(v)) => Ok(json!(v)),
            Ok(Err(e)) => Err(e.into()),
            Err(e) => Err(e.into()),
        }
    }
}
