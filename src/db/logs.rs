use crate::db::models::NewSubstrateLog;
use std::time::Instant;

pub struct LogBatch {
    pub rows: Vec<NewSubstrateLog>,
    pub last_saved: Instant,
}

impl LogBatch {
    pub fn new() -> Self {
        LogBatch {
            rows: Vec::with_capacity(128),
            last_saved: Instant::now(),
        }
    }
}
