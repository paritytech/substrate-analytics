## Substrate Save

Comprises a websocket server accepting incoming telemetry from multiple 
[Substrate](https://github.com/paritytech/substrate) nodes. 
Telemetry will be stored in a SQL database. Management of the database schema is via diesel migrations.

Stored data is purged from the DB according to `LOG_EXPIRY_HOURS`

### Set up for development

- create a `.env` file in project root containing: (eg) 
    - `DATABASE_URL=postgres://username:password@localhost/save` 
    - `PORT=8080`
- install [Diesel cli](https://github.com/diesel-rs/diesel/tree/master/diesel_cli)
- you might need [additional packages](https://github.com/diesel-rs/diesel/blob/master/guide_drafts/backend_installation.md)
- run `diesel database setup` to create a local db for testing

Optionally specify the following environment variables:

- `HEARTBEAT_INTERVAL` (default: 5)
- `CLIENT_TIMEOUT` (default: 10)
- `PURGE_FREQUENCY` (default: 600)
- `LOG_EXPIRY_HOURS`  (default: 168)
- `MAX_PENDING_CONNECTIONS` (default: 8192)
- `DATABASE_POOL_SIZE` (default: 10)

There are some easy possibilities available for optimisation (eg. batching inserts).