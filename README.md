## Substrate Analytics

\* to connect to substrate-analytics you must whitelist your IP address in `deployment.template.yml`

Comprises a websocket server accepting incoming telemetry from multiple 
[Substrate](https://github.com/paritytech/substrate) nodes. Substrate-save is designed to be resilient (to network errors), 
performant and easily horizontally scalable by deploying more servers.

Telemetry is stored in a PostgreSQL database. Management of the database schema is via `diesel` migrations.

Stored data is purged from the DB according to `LOG_EXPIRY_HOURS`

For convenience there are also some JSON endpoints to make ad-hoc queries, although it is expected that 
the data is accessed directly from the database by a suitable dashboard (eg. Grafana).

The next phase of the project with be to parse and structure the incoming logs into 
appropriate (to be determined) database tables.

#### Routes:

- **`/`** 
  - incoming telemetry (with expiry as set by `LOG_EXPIRY_HOURS`) (ws) - set with this option in substrate cli: `--telemetry-url 'ws://127.0.0.1:8080 5'`
- **`/audit`** 
  - incoming telemetry with no expiry (ws) - set with this option in substrate cli: `--telemetry-url 'ws://127.0.0.1:8080/audit 5'`

JSON endpoints for convenience:
- **`/stats/db`** 
  - stats for db showing table / index sizes on disk
- **`/nodes`** 
  - list of logged nodes
- **`/nodes/{peer_id}/peer_counts`** 
  - peer count history (for the 
given peer_id)
- **`/nodes/{peer_id}/log_stats`** 
  - shows the quantity of each type of log message received
- **`/nodes/{peer_id}/logs`** 
  - recent log messages, unprocessed
- **`/nodes/{peer_id}/logs/{msg type}`** 
  - recent log messages, filtered by message type: `msg`

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
- `LOG_EXPIRY_HOURS`  (default: 280320)
- `MAX_PENDING_CONNECTIONS` (default: 8192)
- `WS_MAX_PAYLOAD` (default: 524_288)
- `NUM_THREADS` (default: CPUs * 3)
- `DATABASE_POOL_SIZE` (default: `NUM_THREADS`)
- `DB_BATCH_SIZE` (default: 1024) - batch size for insert
- `DB_SAVE_LATENCY_MS` (default: 100) - max latency (ms) for insert

To allow logging you must set:

- `RUST_LOG` to some log level

Log messages are batched together in each actor before `INSERT` 
\- up to 1024 messages or 100ms, whichever is reached sooner. 

---
