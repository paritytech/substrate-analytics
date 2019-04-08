FROM rust:slim AS builder
RUN apt-get update
RUN apt-get install -y libpq-dev

WORKDIR /substrate-save
COPY . /substrate-save

RUN cargo install diesel_cli --root ./ --no-default-features --features "postgres"

RUN cargo build --release

FROM debian:stretch-slim

RUN apt-get update
RUN apt-get install -y libpq-dev

COPY --from=builder /substrate-save/target/release/save /usr/local/bin/
COPY --from=builder /substrate-save/bin/diesel /usr/local/bin/

# metadata
LABEL maintainer="devops-team@parity.io" \
  vendor="Parity Technologies" \
  name="parity/substrate-save" \
  description="Substrate Analytical and Visual Environment - Incoming telemetry" \
  url="https://github.com/paritytech/substrate-save/" \
  vcs-url="./"

CMD ./diesel migration run
ENTRYPOINT ["/usr/local/bin/save"]
