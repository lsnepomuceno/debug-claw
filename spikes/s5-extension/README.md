# S5 PHP extension in Rust

Spike code for [S5](../../specs/00-spikes/requirements.md#s5-php-extension-in-rust). The report is [`reports/s5-extension.md`](../../specs/00-spikes/reports/s5-extension.md).

- `ext/`: the extension, `debug_claw_s5`, built with `ext-php-rs` and its `observer` feature. It observes `PDOStatement::execute` only and sends one JSON line per call (`pid`, `req`, `ns`, `sql`) to a TCP address.
- `tools/`: `listener` (the daemon stand-in, prints what it receives) and `bench` (a one-connection keep-alive load generator).
- `docker/`: the PHP images (the extension is built inside `php:8.5-fpm-trixie` and `php:8.3-fpm-trixie`, no Xdebug, one static FPM worker) and the tools image.
- `ini/`: one folder per configuration, picked with `S5_PROFILE_85` and `S5_PROFILE_83`.
- `app/`: scripts the PHP containers run from `/s5/app` (smoke test, memory report, CLI micro-benchmarks). The HTTP fixture app is the shared one in `../testbed/app`, mounted read-only.
- `scripts/`: the measurement runs. `results/` keeps their summaries; per-request timings go to `results/raw/`, which is not committed.

## Extension settings

All are `PHP_INI_SYSTEM`.

| Setting | Default | Meaning |
|---|---|---|
| `debug_claw.daemon` | `127.0.0.1:7781` | Address of the daemon, `IP:PORT` |
| `debug_claw.flush` | `request` | `request`: one write at the end of the request. `call`: one write per observed call |
| `debug_claw.retry_ms` | `1000` | After a failed connect or write, how long the extension skips the daemon |
| `debug_claw.connect_timeout_us` | `1000` | Connect timeout |
| `debug_claw.buffer_bytes` | `65536` | Cap of the per-worker buffer of unsent events; events past it are dropped and counted |

`debug_claw_s5_stats()` returns the worker's counters as JSON.

## Commands

Everything runs in Docker. From this folder:

```sh
export COMPOSE_PROJECT_NAME=testbed-s5
docker compose up -d --build

./scripts/smoke.sh     # T-24, T-25: both versions, CLI and FPM, results/smoke.txt
./scripts/overhead.sh php85 10 2000 200   # T-26 on one version, results/t26-php85.txt
./scripts/all.sh       # every T-26 and T-27 run of the report, about 10 minutes
./scripts/paired.sh php85                 # per-round differences of a T-26 run

# ZTS: build and load only
docker build -f docker/zts.Dockerfile -t testbed-s5-zts .
docker run --rm testbed-s5-zts

docker compose down
```

| Script | Writes | Measures |
|---|---|---|
| `overhead.sh SVC [ROUNDS N WARMUP] [PATH TAG]` | `results/t26-SVC[-TAG].txt` | Request time in five configurations (off, on, on-call, closed, gone), rounds in rotating order |
| `cli.sh SVC [ROUNDS]` | `results/cli-SVC.txt` | `app/percall.php` (ns per `execute`) and `app/calls.php` (ns per unobserved call) |
| `soak.sh SVC PROFILE [TOTAL STEP PATH]` | `results/t27-SVC-PROFILE.tsv` | Worker memory and counters every STEP requests |
| `smoke.sh` | `results/smoke.txt` | Build info, CLI smoke test, one FPM request, the listener's log |

The services are pinned to separate physical P-cores of the test machine (`S5_CPU_*` in `compose.yaml`); change the sets on another machine.
