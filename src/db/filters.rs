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

use chrono::NaiveDateTime;

//const TIME_FORMAT: &'static str = "2000-01-01T00:00:00";

// TODO implement validator derive
// #[macro_use]
// extern crate validator_derive;

#[derive(Deserialize, Default, Debug, Clone)]
pub struct Filters {
    pub start_time: Option<NaiveDateTime>,
    pub end_time: Option<NaiveDateTime>,
    pub max_age_s: Option<i64>,
    pub limit: Option<i32>,
    pub peer_id: Option<String>,
    pub target: Option<String>,
    pub msg: Option<String>,
}
