## Substrate Save

Comprises a websocket server accepting incoming telemetry from multiple 
[Substrate](https://github.com/paritytech/substrate) nodes. 
Telemetry will be stored in a SQL database. Management of the database schema is via diesel migrations.

Stored data is purged from the DB according to `LOG_EXPIRY_HOURS`

#### Routes:

- **/** - incoming telemetry (ws)
- **/stats/nodes** - list of logged nodes
- **/stats/nodes/{node ip address}/peer_counts** - peer count history for the 
given node ip address (will also match beginning of ip address, eg: w/wo port)

### Set up for development and deployment

- (for development) create a `.env` file in project root containing: (eg) 
    - `DATABASE_URL=postgres://username:password@localhost/save` 
    - `PORT=8080`
- install [Diesel cli](https://github.com/diesel-rs/diesel/tree/master/diesel_cli)
- you might need [additional packages](https://github.com/diesel-rs/diesel/blob/master/guide_drafts/backend_installation.md)
- run `diesel database setup` to initialise DB
- after any changes to the schema via migrations, you must `diesel migration run`

Optionally specify the following environment variables:

- `HEARTBEAT_INTERVAL` (default: 5)
- `CLIENT_TIMEOUT` (default: 10)
- `PURGE_FREQUENCY` (default: 600)
- `LOG_EXPIRY_HOURS`  (default: 168)
- `MAX_PENDING_CONNECTIONS` (default: 8192)
- `DATABASE_POOL_SIZE` (default: 10)

To allow logging you must set:

- `RUST_LOG` to some log level

---
