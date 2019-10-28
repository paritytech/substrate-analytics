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

use super::models::{HostSystem, NewHostSystem};
use super::DbExecutor;
use crate::db::filters::Filters;
use actix::prelude::*;
use diesel::RunQueryDsl;
use failure::Error;

pub enum Query {
    All(Filters),
}

impl Message for Query {
    type Result = Result<Vec<HostSystem>, Error>;
}

impl Handler<Query> for DbExecutor {
    type Result = Result<Vec<HostSystem>, Error>;

    fn handle(&mut self, msg: Query, _: &mut Self::Context) -> Self::Result {
        match msg {
            Query::All(filters) => self.get_host_systems(filters),
        }
    }
}
impl Message for NewHostSystem {
    type Result = Result<HostSystem, Error>;
}

impl Handler<NewHostSystem> for DbExecutor {
    type Result = Result<HostSystem, Error>;

    fn handle(&mut self, msg: NewHostSystem, _: &mut Self::Context) -> Self::Result {
        self.save_host_system(msg)
    }
}

impl DbExecutor {
    fn get_host_systems(&self, _filters: Filters) -> Result<Vec<HostSystem>, Error> {
        match self.with_connection(|conn| {
            use crate::schema::host_systems::dsl::*;
            host_systems.load::<HostSystem>(conn)
        }) {
            Ok(Ok(v)) => Ok(v),
            Ok(Err(e)) => Err(e.into()),
            Err(e) => Err(e.into()),
        }
    }

    fn save_host_system(&self, msg: NewHostSystem) -> Result<HostSystem, Error> {
        match self.with_connection(|conn| {
            use crate::schema::host_systems;
            diesel::insert_into(host_systems::table)
                .values(msg)
                .get_result(conn)
        }) {
            Ok(Ok(v)) => Ok(v),
            Ok(Err(e)) => Err(e.into()),
            Err(e) => Err(e.into()),
        }
    }
}

impl NewHostSystem {
    pub fn example() -> Self {
        NewHostSystem {
            description: "Any notes to go here".to_owned(),
            os: "freebsd".to_owned(),
            cpu_qty: 4,
            cpu_clock: 2600,
            ram_mb: 8192,
            disk_info: "NVME".to_owned(),
        }
    }
}
