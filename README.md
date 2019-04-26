## Substrate Save

Comprises a websocket server accepting incoming telemetry from multiple 
[Substrate](https://github.com/paritytech/substrate) nodes. 
Telemetry will be stored in a SQL database. Management of the database schema is via diesel migrations.

Stored data is purged from the DB according to `LOG_EXPIRY_HOURS`

#### Routes:

- base is just for incoming telemetry (ws) - set with this option: `--telemetry-url 'ws://127.0.0.1:8080 5'`
- **/stats/db** - stats for db showing table / index sizes on disk
- **/nodes** - list of logged nodes
- **/nodes/{node ip address}/peer_counts** - peer count history for the 
given node ip address (will also match beginning of ip address, eg: w/wo port)
- **/nodes/{node ip address}/log_stats** - shows the quantity of each type of log message received
- **/nodes/{node ip address}/logs** - recent log messages, unprocessed
- **/nodes/{node ip address}/logs/{msg type}** - recent log messages, filtered by message type: `msg`

`peer_counts` and `logs` routes take the following optional parameters (with sensible defaults if not specified):
- `start_time` in the format: `2019-01-01T00:00:00`
- `end_time` in the format: `2019-01-01T00:00:00`
- `limit` in the format: `100`

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

Limitations:

Log messages are batched together before `INSERT` 
\- up to 128 messages or 100ms, whichever is reached sooner. 
However, an insert is only triggered on receipt of a log message, 
(if the check on the above conditions is true). 
In a worst case scenario, there may be up to 127 logs in memory 
that are not persisted to the DB until another message is received, 
(only if they all arrived in under 100ms 
and no more messages were sent subsequently).

---
