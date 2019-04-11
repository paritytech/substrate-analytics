pub mod root;
pub mod stats;

use crate::db::DbExecutor;
use actix::prelude::*;

pub struct State {
    pub db: Addr<DbExecutor>,
}
