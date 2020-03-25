FROM rust:slim as builder

RUN apt-get update && \
    DEBIAN_FRONTEND=noninteractive apt-get upgrade -y && \
    DEBIAN_FRONTEND=noninteractive apt-get install -y libpq-dev


# build diesel first as there may be no changes and caching will be used
RUN echo "building diesel-cli" && \
  cargo install diesel_cli --root /substrate-analytics --bin diesel --force --no-default-features --features postgres


WORKDIR /substrate-analytics

# speed up docker build using pre-build dependencies
# http://whitfin.io/speeding-up-rust-docker-builds/
RUN USER=root cargo init --bin

# copy over your manifests
COPY ./Cargo.lock ./Cargo.lock
COPY ./Cargo.toml ./Cargo.toml

# this build step will cache your dependencies
RUN cargo build --release
RUN rm -rf ./src ./target/release/deps/substrate_analytics-*

# copy your source tree
COPY ./src ./src


# ADD ./ ./

RUN echo "building substrate-analytics" && \
  cargo build --release




FROM debian:stretch-slim
# metadata
LABEL maintainer="devops-team@parity.io" \
  vendor="Parity Technologies" \
  name="parity/substrate-analytics" \
  description="Substrate Analytical and Visual Environment - Incoming telemetry" \
  url="https://github.com/paritytech/substrate-analytics/" \
  vcs-url="./"


RUN apt-get update && \
    DEBIAN_FRONTEND=noninteractive apt-get upgrade -y && \
    DEBIAN_FRONTEND=noninteractive apt-get install -y libpq5 && \
    DEBIAN_FRONTEND=noninteractive apt-get autoremove -y && \
    apt-get clean && \
    find /var/lib/apt/lists/ -type f -not -name lock -delete

RUN useradd -m -u 1000 -U -s /bin/sh -d /analytics analytics

COPY --from=builder /substrate-analytics/target/release/substrate-analytics /usr/local/bin/
COPY --from=builder /substrate-analytics/static /srv/substrate-analytics
COPY --from=builder /substrate-analytics/bin/diesel /usr/local/bin/

COPY ./migrations /analytics/migrations

WORKDIR /analytics
USER analytics
ENV RUST_BACKTRACE 1


ENTRYPOINT [ "/bin/sh", "-x", "-c", "/usr/local/bin/diesel migration run && exec /usr/local/bin/substrate-analytics"]
