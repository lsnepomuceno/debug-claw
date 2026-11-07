# syntax=docker/dockerfile:1
# Builds the S5 extension inside the test bed's PHP image (which has the
# PHP headers and php-config) and ships it in a clean copy of that image.
# Xdebug is not installed, so it cannot hide the extension's cost.
ARG PHP_VERSION=8.5

FROM rust:1.99-slim-trixie AS rust

FROM php:${PHP_VERSION}-fpm-trixie AS build
# ext-php-rs runs bindgen (libclang) on the PHP headers and compiles a small
# C wrapper; gcc links the cdylib.
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
    --mount=type=cache,id=s5-target-${PHP_VERSION},target=/src/target \
    cargo build --release --locked \
    && cp target/release/libdebug_claw_s5.so /debug_claw_s5.so \
    && { rustc --version; cargo --version; clang --version | head -n1; php-config --version; \
         php -r 'printf("ZTS=%d debug=%d\n", PHP_ZTS, PHP_DEBUG);'; } > /build-info.txt

FROM php:${PHP_VERSION}-fpm-trixie
# cgi-fcgi talks to FPM directly (for mem.php), pgrep finds the worker.
RUN apt-get update \
    && apt-get install -y --no-install-recommends libfcgi-bin procps \
    && rm -rf /var/lib/apt/lists/*
# PHP 8.5 always has OPcache; 8.3 ships it disabled. Enable it so both
# versions run the way production does.
RUN php -m | grep -q '^Zend OPcache$' || docker-php-ext-enable opcache
COPY --from=build /debug_claw_s5.so /usr/local/lib/php-s5/debug_claw_s5.so
COPY --from=build /build-info.txt /usr/local/lib/php-s5/build-info.txt
COPY docker/php.ini /usr/local/etc/php/conf.d/zz-s5.ini
COPY docker/pool.conf /usr/local/etc/php-fpm.d/zz-s5.conf
