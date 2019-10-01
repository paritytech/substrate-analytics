pub mod filters;
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

use self::models::{NewPeerConnection, NewSubstrateLog, PeerConnection};
use crate::{DATABASE_POOL_SIZE, DATABASE_URL};

pub struct DbExecutor {
    pool: Pool<ConnectionManager<PgConnection>>,
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
        DbExecutor { pool }
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

pub struct LogBatch(pub Vec<NewSubstrateLog>);

impl Message for LogBatch {
    type Result = Result<(), &'static str>;
}

impl Handler<LogBatch> for DbExecutor {
    type Result = Result<(), &'static str>;

    fn handle(&mut self, msg: LogBatch, _: &mut Self::Context) -> Self::Result {
        use crate::schema::substrate_logs;
        #[allow(unused_imports)]
        use crate::schema::substrate_logs::dsl::*;
        let _ = self.with_connection(|conn| {
            match diesel::insert_into(substrate_logs::table)
                .values(msg.0)
                .execute(conn)
            {
                Err(e) => error!("Error inserting logs: {:?}", e),
                Ok(n) => debug!("Inserted {} substrate_logs", n),
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
                "DELETE FROM substrate_logs \
                 USING peer_connections \
                 WHERE peer_connections.id = peer_connection_id \
                 AND audit = false \
                 AND substrate_logs.created_at < now() - {} * interval '1 hour'",
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
