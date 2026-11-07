FROM rust:1.99-slim-trixie

ARG UID=1000
ARG GID=1000

# netcat and curl are for poking at DBGp and HTTP by hand.
RUN apt-get update \
    && apt-get install -y --no-install-recommends netcat-openbsd curl ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Run as the host user, so files written to the working tree belong to
# them. The cargo cache and target dir are named volumes, created here
# with that owner so Docker copies the ownership into fresh volumes.
RUN (getent group "$GID" || groupadd -g "$GID" dev) \
    && (getent passwd "$UID" || useradd -m -u "$UID" -g "$GID" dev) \
    && mkdir -p /cargo /target \
    && chown "$UID:$GID" /cargo /target

ENV CARGO_HOME=/cargo \
    CARGO_TARGET_DIR=/target
