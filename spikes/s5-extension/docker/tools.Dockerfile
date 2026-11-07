# syntax=docker/dockerfile:1
# The socket listener and the load generator (std only, no crates).
FROM rust:1.99-slim-trixie AS build
WORKDIR /src
COPY tools/ ./
RUN --mount=type=cache,id=s5-tools-target,target=/src/target \
    cargo build --release --locked \
    && cp target/release/listener target/release/bench /usr/local/bin/

FROM debian:trixie-slim
COPY --from=build /usr/local/bin/listener /usr/local/bin/bench /usr/local/bin/
