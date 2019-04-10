FROM rust:slim as builder

RUN apt-get update && \
    DEBIAN_FRONTEND=noninteractive apt-get upgrade -y && \
    DEBIAN_FRONTEND=noninteractive apt-get install -y libpq-dev

WORKDIR /substrate-save

ADD . .

RUN echo "building substrate-save" && \
  cargo build --release
RUN echo "building diesel-cli" && \
  cargo install diesel_cli --root /substrate-save --bin diesel --force --no-default-features --features postgres




FROM debian:stretch-slim
# metadata
LABEL maintainer="devops-team@parity.io" \
  vendor="Parity Technologies" \
  name="parity/substrate-save" \
  description="Substrate Analytical and Visual Environment - Incoming telemetry" \
  url="https://github.com/paritytech/substrate-save/" \
  vcs-url="./"


RUN apt-get update && \
    DEBIAN_FRONTEND=noninteractive apt-get upgrade -y && \
    DEBIAN_FRONTEND=noninteractive apt-get install -y libpq5 && \
    DEBIAN_FRONTEND=noninteractive apt-get autoremove -y && \
    apt-get clean && \
    find /var/lib/apt/lists/ -type f -not -name lock -delete

RUN useradd -m -u 1000 -U -s /bin/sh -d /save save

COPY --from=builder /substrate-save/target/release/save /usr/local/bin/
COPY --from=builder /substrate-save/migrations /save/migrations
COPY --from=builder /substrate-save/bin/diesel /usr/local/bin/

USER save
ENV RUST_BACKTRACE 1


ENTRYPOINT [ "/bin/sh", "-c", "/usr/local/bin/diesel migration run && exec /usr/local/bin/save"]
