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

pub mod metrics;
pub mod nodes;
pub mod reputation;
pub mod root;
pub mod stats;

use crate::db::filters::Filters;

pub fn get_filters(req: &actix_web::HttpRequest) -> Filters {
    match actix_web::web::Query::<Filters>::from_query(&req.query_string()) {
        Ok(f) => f.clone(),
        Err(_) => {
            warn!("Error deserializing Filters from querystring");
            Filters::default()
        }
    }
}
