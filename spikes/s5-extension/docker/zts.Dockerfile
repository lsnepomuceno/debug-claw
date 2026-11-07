# syntax=docker/dockerfile:1
# Builds the S5 extension against a thread-safe (ZTS) PHP, only to check
# that it compiles and loads there. The FPM test bed is NTS.
#
#   docker build -f docker/zts.Dockerfile -t testbed-s5-zts .
#   docker run --rm testbed-s5-zts
ARG PHP_VERSION=8.5

FROM rust:1.99-slim-trixie AS rust

FROM php:${PHP_VERSION}-zts-trixie
RUN apt-get update \
    && apt-get install -y --no-install-recommends clang libclang-dev gcc libc6-dev \
    && rm -rf /var/lib/apt/lists/*
COPY --from=rust /usr/local/cargo /usr/local/cargo
COPY --from=rust /usr/local/rustup /usr/local/rustup
ENV CARGO_HOME=/usr/local/cargo RUSTUP_HOME=/usr/local/rustup PATH=/usr/local/cargo/bin:$PATH
WORKDIR /src
COPY ext/ ./
ARG PHP_VERSION
RUN --mount=type=cache,id=s5-cargo-registry,target=/usr/local/cargo/registry \
    --mount=type=cache,id=s5-target-${PHP_VERSION}-zts,target=/src/target \
    cargo build --release --locked \
    && cp target/release/libdebug_claw_s5.so /debug_claw_s5.so
COPY app/smoke.php /smoke.php
# The listener is not running here, so the smoke test shows the down path.
CMD ["sh", "-c", "php -r 'printf(\"PHP %s ZTS=%d\\n\", PHP_VERSION, PHP_ZTS);' && php -d extension=/debug_claw_s5.so -d debug_claw.daemon=127.0.0.1:7781 /smoke.php"]
