## Substrate Analytical and Visual Environment - Incoming telemetry

This repo  contains Websocket server using PostgreSQL database backend. 

All DB migrations (DB structure) are maintained here.

TODO: Add tests, more endpoints? optimise performance, eg. bulk-inserts, maybe change timestamp from default, to either system generated or parsed, depending on if it's to be used purely for expiring old records or another purpose.

### Set up for development

- create a `.env` file in project root containing: (eg) 
    - `DATABASE_URL=postgres://username:password@localhost/save` 
    - `PORT=8080`
- install [Diesel cli](https://github.com/diesel-rs/diesel/tree/master/diesel_cli)
- you might need [additional packages](https://github.com/diesel-rs/diesel/blob/master/guide_drafts/backend_installation.md)
- run `diesel database setup` to create a local db for testing

Optionally specify:

- `HEARTBEAT_INTERVAL`
- `CLIENT_TIMEOUT`
- `PURGE_FREQUENCY`
- `LOG_EXPIRY_HOURS`
- `MAX_PENDING_CONNECTIONS`
- `DATABASE_POOL_SIZE`

### Performance expectations

Performance has not been optimised yet.

2 Benchmarks carried out on Heroku platform with following config:

- 1x Standard dyno ($25)
- Standard Postgres ($50)

V2 (~30 min) - Standard json payload, approx 250KB/s network load- , 10 websocket clients
 
Summary report @ 17:22:05(+0000) 2019-02-14
  - Scenarios launched:  10 (clients connected)
  - Scenarios completed: 10
  - Requests completed:  1800000
  - RPS sent: 924.9
  - Request latency:
    - min: 0
    - max: 17.3
    - median: 0.2
    - p95: 0.3
    - p99: 0.4
  - Scenario counts:
    - 0: 10 (100%)
  - Codes:
    - 0: 1800000
    
Dyno load ave 2.5

V3 (~30 min) large json payload: 11.5MB/s network load - dyno load average around 2 in first half, suddenly jumped to ave 6 in second half. memory stable.

Summary report @ 18:01:14(+0000) 2019-02-14
  - Scenarios launched:  10 (clients connected)
  - Scenarios completed: 10
  - Requests completed:  1800000
  - RPS sent: 846.46
  - Request latency:
    - min: 0.1
    - max: 18.6
    - median: 0.5
    - p95: 1
    - p99: 1.1
  - Scenario counts:
    - 0: 10 (100%)
  - Codes:
    - 0: 1800000
    
Also tested with peak loads up to 10,000 RPS for 10 minutes (V2 config with 100 clients, each with 100RPS), which peaked dyno load at 12x. Databse throughput was less than 50% of that (ie. < 50% records inserted at conclusion of 10k RPS test), but the backlog of around 500k inserts eventually caught up. 0 error responses in all cases and all rows accounted for in DB.

To keep stable performance long-term we should aim for a load average around 1 per 1x Standard Dyno which means around 350RPS. Short/medium-term peaks are fine, as shown above.

Some easy possibilities available for optimisation (eg. batching inserts).

