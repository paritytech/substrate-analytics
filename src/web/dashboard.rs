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

use crate::ASSETS_PATH;
use actix_files::Files;

pub fn configure(cfg: &mut actix_web::web::ServiceConfig) {
    cfg.service(
        Files::new(
            "dashboard/benchmarks",
            &format!("{}/benchmarks/", *ASSETS_PATH),
        )
        .index_file("index.html"),
    );
    cfg.service(
        Files::new(
            "dashboard/profiling",
            &format!("{}/profiling/", *ASSETS_PATH),
        )
        .index_file("index.html"),
    );
    cfg.service(
        Files::new(
            "dashboard/reputation",
            &format!("{}/reputation/", *ASSETS_PATH),
        )
        .index_file("index.html"),
    );
    cfg.service(
        Files::new("dashboard/health", &format!("{}/health/", *ASSETS_PATH))
            .index_file("index.html"),
    );
}
