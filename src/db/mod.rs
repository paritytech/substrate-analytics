pub mod filters;
pub mod logs;
pub mod models;
pub mod nodes;
pub mod stats;

use actix::prelude::*;
use diesel;
use diesel::pg::PgConnection;
use diesel::prelude::*;
use diesel::r2d2::{ConnectionManager, Pool, PoolError};
use diesel::result::QueryResult;
use diesel::RunQueryDsl;
use logs::LogBatch;
use std::time::{Duration, Instant};

use self::models::{NewPeerConnection, NewSubstrateLog, PeerConnection};
use crate::{DATABASE_POOL_SIZE, DATABASE_URL};

pub struct DbExecutor {
    pool: Pool<ConnectionManager<PgConnection>>,
    log_batch: LogBatch,
}

impl Actor for DbExecutor {
    type Context = SyncContext<Self>;
}

impl DbExecutor {
    // Execute query, log error if any and return result.
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

    // Creates a new DbExecutor
    pub fn new(pool: Pool<ConnectionManager<PgConnection>>) -> Self {
        let log_batch = LogBatch::new();
        DbExecutor { pool, log_batch }
    }

    fn save_logs(&self, new_logs: &[NewSubstrateLog]) {
        use crate::schema::substrate_logs;
        #[allow(unused_imports)]
        use crate::schema::substrate_logs::dsl::*;
        let _ = self.with_connection(|conn| {
            match diesel::insert_into(substrate_logs::table)
                .values(new_logs)
                .execute(conn)
            {
                Err(e) => error!("Error inserting logs: {:?}", e),
                Ok(n) => trace!("Inserted {} log records into database", n),
            }
        });
    }
}

impl Message for NewPeerConnection {
    type Result = Result<PeerConnection, String>;
}

impl Handler<NewPeerConnection> for DbExecutor {
    type Result = Result<PeerConnection, String>;

    fn handle(&mut self, msg: NewPeerConnection, _: &mut Self::Context) -> Self::Result {
        use crate::schema::peer_connections;
        #[allow(unused_imports)]
        use crate::schema::peer_connections::dsl::*;
        let pc: Result<Result<PeerConnection, _>, _> = self.with_connection(|conn| {
            let result: QueryResult<PeerConnection> = diesel::insert_into(peer_connections::table)
                .values(&msg)
                .get_result(conn);
            result
        });
        if let Ok(pcr) = pc {
            if let Ok(p) = pcr {
                return Ok(p);
            }
        };
        Err(format!(
            "Error inserting PeerConnection, for ip: {}",
            msg.ip_addr
        ))
    }
}

impl Message for PeerConnection {
    type Result = Result<(), String>;
}

impl Handler<PeerConnection> for DbExecutor {
    type Result = Result<(), String>;

    fn handle(&mut self, msg: PeerConnection, _: &mut Self::Context) -> Self::Result {
        //use crate::schema::peer_connections;
        #[allow(unused_imports)]
        use crate::schema::peer_connections::dsl::*;
        let msg_id = msg.id;
        let result = self.with_connection(|conn| {
            diesel::update(peer_connections.filter(id.eq(msg.id)))
                .set((peer_id.eq(msg.peer_id), ip_addr.eq(msg.ip_addr)))
                .execute(conn)
        });
        if let Ok(ir) = result {
            if let Ok(_) = ir {
                return Ok(());
            }
        };
        Err(format!("Error updating PeerConnection, id: {}", msg_id))
    }
}

impl Message for NewSubstrateLog {
    type Result = Result<(), &'static str>;
}

impl Handler<NewSubstrateLog> for DbExecutor {
    type Result = Result<(), &'static str>;

    fn handle(&mut self, msg: NewSubstrateLog, _: &mut Self::Context) -> Self::Result {
        self.log_batch.rows.push(msg);
        if self.log_batch.last_saved + Duration::from_millis(100) < Instant::now()
            || self.log_batch.rows.len() > 127
        {
            let rows_to_save = std::mem::replace(&mut self.log_batch.rows, Vec::with_capacity(128));
            self.save_logs(&rows_to_save);
            self.log_batch.last_saved = Instant::now();
        }
        Ok(())
    }
}

pub struct CreateSubstrateLog {
    pub log: NewSubstrateLog,
}

impl Message for CreateSubstrateLog {
    type Result = Result<(), &'static str>;
}

impl Handler<CreateSubstrateLog> for DbExecutor {
    type Result = Result<(), &'static str>;

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
    type Result = Result<(), &'static str>;
}

impl Handler<PurgeLogs> for DbExecutor {
    type Result = Result<(), &'static str>;

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
