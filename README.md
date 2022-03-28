## Substrate Analytics

\* to connect to substrate-analytics you must whitelist your IP address in `deployment.template.yml`

Comprises a websocket server accepting incoming telemetry from multiple
[Substrate](https://github.com/paritytech/substrate) nodes. substrate-analytics is designed to be resilient (to network errors),
performant and horizontally scalable by deploying more servers.

Telemetry is stored in a PostgreSQL database. Management of the database schema is via `diesel` migrations.

Stored data is purged from the DB according to `LOG_EXPIRY_H`

For convenience there are also some JSON endpoints to make ad-hoc queries, although it is expected that
the data is accessed directly from the database by a suitable dashboard (eg. Grafana).

### Routes

#### Data ingestion
`substrate-analytics` can work in one of two modes: with or without purging data after `LOG_EXPIRY_H` hours. The mode it operates under depends on which of the following two endpoints you send data to from your substrate nodes.
- **`/`**
  - incoming telemetry (with expiry as set by `LOG_EXPIRY_H`) (ws) - set with this option in substrate cli: `--telemetry-url 'ws://127.0.0.1:8080 5'`
- **`/audit`**
  - incoming telemetry with no expiry (ws) - set with this option in substrate cli: `--telemetry-url 'ws://127.0.0.1:8080/audit 5'`

#### JSON endpoints
`subtrate-analytics` includes a few convenience endpoints to query for common data.
- **`/stats/db`**
  - statistics about the postgres db, showing table and index sizes on disk
- **`/nodes`**
  - list of logged nodes
- **`/nodes/log_stats?peer_id=Qmd5K38Yti1NStacv7fjJwsXDCUZcf1ioKcAuFkq88RKtx`**
  - shows the quantity of each type of log message received
- **`/nodes/logs?peer_id=Qmd5K38Yti1NStacv7fjJwsXDCUZcf1ioKcAuFkq88RKtx&limit=1&msg=tracing.profiling&target=pallet_babe&start_time=2020-03-25T13:17:09.008533`**
  - recent log messages. Required params: `peer_id`, Optional params: `msg, target, start_time, end_time, limit`.

    `msg`: String. Type of log message received, e.g. `block.import`. See [./telemetry_messages.json](telemetry_messages.json) for the current list of message types.

    `target`: String. Origin of the message, e.g. `NetworkInitialSync`

    `start_time`: String. Include entries more recent than this; format: `2019-01-01T00:00:00`. Default: `NOW`.

    `end_time`: String. Include entries less recent than this; format: `2019-01-01T00:00:00`. Default: `NOW`.

    `limit`: Number. Don't include more results than this. Default: `100`
- **`/reputation/{peer_id}`**
  - reported reputation for `peer_id` from the POV of other nodes.
- **`/reputation/logged`**
  - reported reputation for all peers from the POV of all logged (past/present) nodes
- **`/reputation`**
  - reported reputation for all peers unfiltered 
  (note that this can contain many entries that are not even part of the network)


`reputation` routes take the following optional parameters (with sensible defaults if not specified):
- `max_age_s` in the format: `10`
- `limit` in the format: `100`

#### Self-monitoring

Substrate Analytics provides a `/metrics` endpoint for Prometheus to useful to monitor the analytics instance itself. Visit the endpoint in a browser to see what metrics are available.

### Set up for development and deployment
- [Install Postgres](https://www.postgresql.org/docs/current/tutorial-install.html)
- For development, create a `.env` file in the project root containing:
    - `DATABASE_URL=postgres://username:password@localhost/substrate-analytics`
    - `PORT=8080`
    - any other settings from the list of environment variables below
- Next, install [Diesel cli](https://github.com/diesel-rs/diesel/tree/master/diesel_cli)
  - You might need [additional packages](https://github.com/diesel-rs/diesel/blob/master/guide_drafts/backend_installation.md)
- Run `diesel database setup` to initialise the postgres DB
- You must `diesel migration run` after any changes to the database schema

Optionally specify the following environment variables:

- `HEARTBEAT_INTERVAL` (default: 5)
- `CLIENT_TIMEOUT_S` (default: 10)
- `PURGE_INTERVAL_S` (default: 600)
- `LOG_EXPIRY_H`  (default: 280320)
- `MAX_PENDING_CONNECTIONS` (default: 8192)
- `WS_MAX_PAYLOAD` (default: 524_288)
- `NUM_THREADS` (default: CPUs * 3)
- `DB_POOL_SIZE` (default: `NUM_THREADS`)
- `DB_BATCH_SIZE` (default: 1024) - batch size for insert
- `DB_SAVE_LATENCY_MS` (default: 100) - max latency (ms) for insert
- `CACHE_UPDATE_TIMEOUT_S` (default: 15) - seconds before timeout warning - aborts update after 4* timeout
- `CACHE_UPDATE_INTERVAL_MS` (default: 1000) - time interval (ms) between updates
- `CACHE_EXPIRY_S` (default: 3600) - expiry time (s) of log messages
- `ASSETS_PATH` (default: `./static`) - static files path

Include `RUST_LOG` in your `.env` file to make `substrate-analytics` log to stdout. A good development setting is `RUST_LOG = debug`.

Substrate log messages are batched together before they are sent off for storage in the postgres DB by the actor for `INSERT`. Batches include up to `DB_BATCH_SIZE` messages or `DB_SAVE_LATENCY_MS`, whichever is reached sooner.

#### Benchmarking

Substrate-analytics has endpoints to define benchmarks and host systems that run the benchmarks. This is
designed to be cross-referenced with telemetry data to provide insights into the node and system under test.

JSON endpoints:

- **`/host_systems`**: the server machines we're benchmarking
  - `GET` to list all; `POST` to create new using the format (returns object with newly created `id`):
```json
{
   "cpu_clock":2600,
   "cpu_qty":4,
   "description":"Any notes to go here",
   "disk_info":"NVME",
   "os":"freebsd",
   "ram_mb":8192
}
```
- **`/benchmarks`**:
  - `GET` to list all, `POST` to create new using the format (returns object with newly created `id`):
```json
{
   "benchmark_spec":{
      "tdb":"tbd"
   },
   "chain_spec":{
      "name":"Development",
      "etc": "more chain spec stuff"
   },
   "description":"notes",
   "host_system_id":2,
   "ts_end":"2019-10-28T14:05:27.618903",
   "ts_start":"1970-01-01T00:00:01"
}
```

