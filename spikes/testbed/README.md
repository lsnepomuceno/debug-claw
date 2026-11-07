# Test bed

The shared fixture for the [milestone 00 spikes](../../specs/00-spikes/design.md#test-bed). Everything runs in Docker; nothing is installed on the host ([ADR-0011](../../specs/decisions/0011-docker-development.md)).

| Service | What it is |
|---|---|
| `toolchain` | Rust 1.99 (`rust:1.99-slim-trixie`), plus `nc` and `curl`. Spikes build and run here, as the host user. The cargo cache and `target/` are named volumes. |
| `php85` | PHP 8.5 FPM, with `app/` mounted at `/var/www/html` |
| `php83` | PHP 8.3 FPM, with the same `app/` mounted at `/srv/app` |
| `nginx` | `127.0.0.1:8080` serves `/var/www/html/public` through `php85` (entry script in `public/`). `127.0.0.1:8081` serves `/srv/app` through `php83` (project root as the document root). |

Both PHP services run Xdebug 3.5.3 with `mode=debug`, `start_with_request=trigger`, `client_host=toolchain` and `client_port=9003`, and log to `/var/log/xdebug/xdebug.log` inside the container.

Versions were picked on 2026-11-07 from php.net and xdebug.org. PHP 8.5 is the newest branch. PHP 8.3 is the oldest branch with security support past 2026 (8.2 ends on 2026-12-31). Xdebug 3.5.3 is the latest stable release and supports both.

## Use

```sh
cd spikes/testbed
docker compose up -d --build
docker compose exec toolchain cargo run --manifest-path spikes/s1-path-mapping/Cargo.toml
```

The repository is mounted at `/work` in `toolchain`, which is also the working directory. If your user ID or group ID is not 1000, export `HOST_UID` and `HOST_GID` before building.

To run a second copy from another git worktree, give it its own project name and ports:

```sh
COMPOSE_PROJECT_NAME=testbed-s2 HTTP_PORT_85=8180 HTTP_PORT_83=8181 docker compose up -d --build
```

The containers bind-mount the working tree. Switching branches in a way that deletes and recreates this folder leaves them on a stale mount (requests fail with "File not found"); run `docker compose up -d --force-recreate` after that.

## Starting a debug session

Only requests with the `XDEBUG_TRIGGER` cookie (or query parameter) start a session:

```sh
docker compose exec toolchain nc -l 9003           # a bare listener, prints the init packet
curl -b XDEBUG_TRIGGER=1 http://127.0.0.1:8080/loop
```

## Fixture application

Plain PHP, no framework and no Composer. `public/index.php` is the front controller and `src/` holds the classes.

| Route | What it does |
|---|---|
| `GET /loop?n=100` | A loop where `$item` changes on every iteration, for logpoints |
| `GET /graph?depth=6&width=3` | Builds a tree of `App\Node` objects with parent links, for variable trees |
| `GET /queries?n=100` | 50 inserts, then `n` prepared selects with PDO on in-memory SQLite |
| `GET /warning` | Raises an `E_WARNING`, for breakpoints on PHP errors |
| `POST /orders` with `customer_id=N` | The MCP reviewer scenario: every third customer gets HTTP 500, because `$order->total` is 0 when `OrderService::charge()` runs |
