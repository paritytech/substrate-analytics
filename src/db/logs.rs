use std::time::Instant;

pub struct RawLog {
    pub ip_addr: String,
    pub json: String,
}

pub struct LogBatch {
    // ('ip_addr'), ('json')
    pub rows: Vec<String>,
    pub last_saved: Instant,
}

impl LogBatch {
    pub fn new() -> Self {
        LogBatch {
            rows: Vec::with_capacity(1024),
            last_saved: Instant::now(),
        }
    }

    pub fn push(&mut self, raw_log: RawLog) {
        let row = format!("('{}', '{}')", raw_log.ip_addr, raw_log.json);
        self.rows.push(row);
    }
}
