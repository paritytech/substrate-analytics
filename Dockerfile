FROM rustlang/rust:nightly-slim as builder

WORKDIR /substrate-save


RUN cargo build --release



FROM debian:stretch-slim

COPY --from=builder /substrate-save/target/release/save /usr/local/bin/


# metadata
LABEL maintainer="devops-team@parity.io" \
  vendor="Parity Technologies" \
  name="parity/substrate-save" \
  description="Substrate Analytical and Visual Environment - Incoming telemetry" \
  url="https://github.com/paritytech/substrate-save/" \
  vcs-url="./"

ENTRYPOINT ["/usr/local/bin/save"]
