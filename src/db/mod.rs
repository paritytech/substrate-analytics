pub mod logs;
pub mod models;

use ::actix::prelude::*;
use ::actix_web::*;
use diesel;
use diesel::pg::PgConnection;
use diesel::r2d2::{ConnectionManager, Pool, PoolError};
use diesel::RunQueryDsl;
use logs::{LogBatch, RawLog};
use std::time::{Duration, Instant};

use crate::db::models::NewSubstrateLog;
use crate::{DATABASE_POOL_SIZE, DATABASE_URL};

pub struct DbExecutor {
    pool: Pool<ConnectionManager<PgConnection>>,
    log_batch: LogBatch,
}

impl Actor for DbExecutor {
    type Context = SyncContext<Self>;
}

impl DbExecutor {
    // Execute query, returning result. Log error if any and return result.
    fn with_connection<F, R>(&self, f: F) -> Result<R, PoolError>
    where
        F: FnOnce(&PgConnection) -> R,
    {
        let result = self.pool.get().map(|conn| f(&conn));
        if let Err(e) = &result {
            error!("Couldn't get DB connection from pool: {}", e);
        }
        result
    }

    // Creates a new DbExecutor, with connection pool
    pub fn new(pool: Pool<ConnectionManager<PgConnection>>) -> Self {
        let log_batch = LogBatch::new();
        DbExecutor { pool, log_batch }
    }

    fn save_logs(&self, logs: Vec<String>) {
        let values: String = logs
            .into_iter()
            .map(|s| s)
            .collect::<Vec<String>>()
            .join(&",");
        let _ = self.with_connection(|conn| {
            let query = format!(
                "INSERT INTO substrate_logs (node_ip, logs) values {}",
                values
            );
            match diesel::sql_query(query).execute(conn) {
                Err(e) => error!("Error inserting logs: {:?}", e),
                Ok(n) => trace!("Inserted {} log records into database", n),
            }
        });
    }
}

impl Message for RawLog {
    type Result = Result<(), Error>;
}

impl Handler<RawLog> for DbExecutor {
    type Result = Result<(), Error>;

    fn handle(&mut self, msg: RawLog, _: &mut Self::Context) -> Self::Result {
        self.log_batch.push(msg);
        if self.log_batch.last_saved + Duration::from_millis(100) < Instant::now()
            || self.log_batch.rows.len() > 127
        {
            let rows_to_save = std::mem::replace(&mut self.log_batch.rows, Vec::with_capacity(128));
            self.save_logs(rows_to_save);
            self.log_batch.last_saved = Instant::now();
        }
        Ok(())
    }
}

pub struct CreateSubstrateLog {
    pub log: NewSubstrateLog,
}

impl Message for CreateSubstrateLog {
    type Result = Result<(), Error>;
}

impl Handler<CreateSubstrateLog> for DbExecutor {
    type Result = Result<(), Error>;

    fn handle(&mut self, msg: CreateSubstrateLog, _: &mut Self::Context) -> Self::Result {
        let _ = self.with_connection(|conn| {
            use crate::schema::substrate_logs;
            #[allow(unused_imports)]
            use crate::schema::substrate_logs::dsl::*;
            #[allow(unused_imports)]
            use diesel::prelude::*;
            let result = diesel::insert_into(substrate_logs::table)
                .values(msg.log)
                .execute(conn);
            if let Err(e) = result {
                error!("Error saving log: {:?}", e);
            }
        });
        Ok(())
    }
}

#[derive(Clone)]
pub struct PurgeLogs {
    pub hours_valid: u32,
}

impl Message for PurgeLogs {
    type Result = Result<(), Error>;
}

impl Handler<PurgeLogs> for DbExecutor {
    type Result = Result<(), Error>;

    fn handle(&mut self, msg: PurgeLogs, _: &mut Self::Context) -> Self::Result {
        let _ = self.with_connection(|conn| {
            let query = format!(
                "DELETE FROM substrate_logs WHERE created_at < now() - {} * interval '1 hour'",
                msg.hours_valid
            );
            info!("Cleaning up database");
            match diesel::sql_query(query).execute(conn) {
                Err(e) => error!("Error purging expired logs: {:?}", e),
                Ok(n) => info!("Purged {} records from database", n),
            }
        });
        Ok(())
    }
}

pub fn create_pool() -> Pool<ConnectionManager<PgConnection>> {
    let manager = ConnectionManager::new(DATABASE_URL.to_string());
    let pool = Pool::builder()
        .max_size(*DATABASE_POOL_SIZE)
        .build(manager)
        .expect("Failed to create pool");
    info!(
        "Database pool created with {} connections",
        *DATABASE_POOL_SIZE
    );
    pool
}
