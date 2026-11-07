# S2 logpoints: working notes

Raw notes for T-07 to T-10, kept so that `reports/s2-logpoints.md` (T-13) could be written from them. Criteria 6 and 7 (T-11, T-12) are answered in the report itself; the section "Criteria 6 and 7" below only lists how to rerun them. The raw output of every run is in `results/`.

## Machine and versions

All runs on 2026-11-07, on one laptop.

| Item | Value |
|---|---|
| CPU | 12th Gen Intel Core i7-1260P, 12 cores (4 P and 8 E), 16 threads (`nproc` 16), governor `powersave` |
| Memory | 32458124 kB (31 GiB) |
| Kernel | Linux 7.0.0-38-generic |
| Docker | 29.9.2 |
| PHP | 8.5.12 FPM (`php:8.5-fpm-trixie`), NTS, OPcache loaded with default settings |
| Xdebug | 3.5.3, `mode=debug`, `start_with_request=trigger` |
| nginx | 1.30-alpine |
| Rust | cargo 1.99.1 in the `toolchain` image (`rust:1.99-slim-trixie`) |

The S1 test bed (`testbed-s1`) was running on the same machine at the same time, so another agent's requests may have competed for CPU during some runs. Every number below is a median, and the spread inside each results file shows how much noise there was.

## Setup

Everything runs in the S2 copy of the test bed, with nothing installed on the host:

```sh
cd spikes/testbed
COMPOSE_PROJECT_NAME=testbed-s2 HTTP_PORT_85=8280 HTTP_PORT_83=8281 docker compose up -d --build
docker compose exec toolchain sh -c 'cd spikes/s2-logpoints && cargo test --release && cargo build --release'
```

(The last command needs the same three variables.) The listener is `/target/release/s2-logpoints`, built from `src/`, with no dependencies. It sets a line breakpoint on `file:///var/www/html/public/index.php` line 44 (`$sum += $item['square'];` inside `loop()`), and on each pause it reads the expressions with `property_get -n <expr> -m 1024`, stores them in a ring buffer (1000 captures by default), checks them against what the loop computes, and sends `run`. Before the breakpoint it sends `feature_set` for `max_depth=1`, `max_children=32` and `max_data=1024`. Each connection runs on its own thread.

The expressions are `$item` with `--exprs 1`, and `$item`, `$i`, `$sum`, `$n`, `$item['square']` with `--exprs 5`. `--exprs 0` pauses and continues without reading anything. `--no-break` opens the session and runs without a breakpoint. `--pipeline` writes the `property_get` commands and the `run` in one write, then reads the responses.

The scripts run on the host and use only `docker` and `curl`:

| Script | What it does |
|---|---|
| `profile.sh bridge\|local\|default` | Writes `ini/<profile>.ini` to `/usr/local/etc/php/conf.d/zzz-s2.ini` inside the php85 container, reloads FPM with `kill -USR2 1`, sends 5 untriggered warm-up requests. `default` removes the file. |
| `bench.sh MODE REPS N [args]` | Starts the listener for REPS sessions, fires REPS triggered `curl` requests to `/loop?n=N` and prints each HTTP code, `time_total` and whether the body has the right sum, then the median and the listener output. `PARALLEL=1` fires them at once. |
| `matrix.sh local\|bridge [REPS]` | The T-08 time matrix, written to `results/t08-<kind>.txt`. |
| `summarize.sh FILE` | Median per scenario, added time per hit, mean hold and gap per pause. |
| `cgroup-cpu.sh N [args]` | One bridge run with the toolchain container's `cpu.stat` read before and after. |
| `t09.sh`, `t10.sh` | T-09 and T-10. |

The two ini profiles turn the Xdebug log off (`xdebug.log=`), because the test bed default (`log_level=7`, every packet written to a file) adds its own cost per hit. One run with the default log is kept for comparison.

### Connection kinds

| Kind | Listener | Xdebug `client_host` |
|---|---|---|
| local | `docker run --network container:testbed-s2-php85-1` with the toolchain image and the `testbed-s2_target` volume, listening on `127.0.0.1:9003` in php85's network namespace | `127.0.0.1` (`ini/local.ini`) |
| bridge | `docker compose exec toolchain`, listening on `0.0.0.0:9003` | `toolchain` (`ini/bridge.ini`) |

"local" stands in for a daemon on the same host as PHP, over loopback. "bridge" is a connection into a container across the Compose bridge network. A daemon on the host talking to PHP in a container was not measured, because nothing runs on the host.

## Criterion 1: capture on every hit (T-07)

```sh
./bench.sh bridge 3 100 --exprs 5 > results/t07.txt
./profile.sh local && ./bench.sh local 3 100 --exprs 5 >> results/t07.txt
```

All 6 requests returned HTTP 200 with the right `sum` in the body. Every session paused 100 times with 0 mismatches. The oldest and newest captures of one session:

```
#1[{index=0,square=0} | 0 | 0 | 100 | 0]
#100[{index=99,square=9801} | 99 | 318549 | 100 | 9801]
```

The listener checks every capture, not only these two: `$item` must be `{index=i,square=i*i}`, `$i` must be the pause number minus one, `$sum` the sum of squares below `i`, `$item['square']` equal to `i*i`, and `$n` the same on every pause. Across every run in `results/` (T-07 to T-10, 100 000 hits in the longest single request) there was no mismatch and no non-200 response.

The session ends cleanly. After the last `run`, Xdebug answers with `status="stopping"`, the listener sends `stop`, and the request completes with its normal body.

Result: pass.

## Criterion 2: added time per hit (T-08)

```sh
./matrix.sh local 5
./matrix.sh bridge 5
./summarize.sh results/t08-local.txt
./summarize.sh results/t08-bridge.txt
```

Method: `curl` on the host measures `time_total` for `/loop?n=N` through nginx. Each scenario runs 5 requests one after the other, and the median is kept. The added time per hit is `(median with breakpoint - median with --no-break) / N`, with both runs at the same N and the same connection kind. "untrig" is a request without the trigger cookie, so no debug session. Hold is the time the listener spends between reading the break and sending `run`. Gap is the time between sending `run` and reading the next break.

Added time per hit, in microseconds:

| Expressions | local, n=1000 | local, n=10000 | bridge, n=1000 | bridge, n=10000 |
|---|---|---|---|---|
| 0 (pause and continue) | 15.4 | 10.3 | 16.6 | 11.5 |
| 1 | 35.2 | 25.7 | 34.6 | 27.3 |
| 5 | 64.6 | 63.9 | 76.9 | 71.8 |
| 5, pipelined | 41.0 | 40.5 | 45.2 | 44.2 |

Medians of the whole request, in ms:

| Scenario | local, n=1000 | local, n=10000 | bridge, n=1000 | bridge, n=10000 |
|---|---|---|---|---|
| untrig | 1.16 | 3.17 | 1.72 | 2.94 |
| session, no breakpoint | 1.78 | 7.17 | 3.32 | 4.79 |
| 0 expressions | 17.18 | 110.39 | 19.95 | 119.61 |
| 1 expression | 37.03 | 264.10 | 37.96 | 277.61 |
| 5 expressions | 66.37 | 646.65 | 80.25 | 722.76 |
| 5, pipelined | 42.76 | 412.52 | 48.49 | 446.36 |

Listener side, at n=10000 with 5 expressions in sequence: mean hold 58.1 us (local) and 63.3 us (bridge), mean gap about 8 us. The p99 hold was at most 160 us in these runs, and the largest single hold was 1.1 ms. At n=10000 the pause alone costs 10 to 12 us, the first read (`$item`, an array with two children) about 15 us more, and each of the four further reads about 10 to 11 us.

With the test bed default log (`log_level=7`, `results/t08-log-on.txt`, bridge, n=1000, 5 expressions) the added time was 91.0 us per hit against 76.9 us with the log off.

Budget: at most 5 ms per hit with 5 expressions on the same host. The worst measured figure is 76.9 us, about 1.5% of the budget. Pipelining the reads cuts the cost by about 35 to 40%, because Xdebug 3.5.3 handles several commands sent in one write.

Result: pass, on both connection kinds.

## Criterion 3: daemon CPU and memory over 10 000 hits (T-08)

```sh
./bench.sh bridge 1 10000 --exprs 5 --buffer-cap 1000 --mem-every 1000 --quiet > results/t08-cpu-mem-10k.txt
./bench.sh bridge 1 100000 --exprs 5 --buffer-cap 1000 --mem-every 10000 --quiet > results/t08-cpu-mem-100k.txt
./cgroup-cpu.sh 10000 --exprs 5 --buffer-cap 1000 --mem-every 5000 --quiet > results/t08-cpu-cgroup.txt
./cgroup-cpu.sh 10000 --exprs 5 --pipeline --buffer-cap 1000 --mem-every 5000 --quiet >> results/t08-cpu-cgroup.txt
```

Method: the listener reads `/proc/self/status` (`VmRSS`) after every M pauses and `/proc/self/stat` (`utime`, `stime`, 10 ms ticks, `CLK_TCK` is 100) at start and end. These are read inside the listener's container, by the listener itself, on hit counts, so nothing polls. `cgroup-cpu.sh` reads `/sys/fs/cgroup/cpu.stat` of the toolchain container before and after, as a second source for CPU time (it also counts the rest of the container, which is idle apart from `sleep infinity`).

RSS with a buffer of 1000 captures of 5 values, 10 000 hits:

| Pause | 1000 | 2000 | 3000 | 4000 | 5000 | 6000 | 7000 | 8000 | 9000 | 10000 |
|---|---|---|---|---|---|---|---|---|---|---|
| VmRSS (kB) | 2592 | 2596 | 2596 | 2600 | 2608 | 2608 | 2616 | 2620 | 2620 | 2620 |

The same with 100 000 hits in one request:

| Pause | 10k | 20k | 30k | 40k | 50k | 60k | 70k | 80k | 90k | 100k |
|---|---|---|---|---|---|---|---|---|---|---|
| VmRSS (kB) | 2560 | 2560 | 2568 | 2572 | 2576 | 2580 | 2584 | 2588 | 2588 | 2588 |

RSS at start was about 2.1 MB. The buffer is full from pause 1000 on. After that, RSS moves by 28 kB between pause 1000 and 10 000, and by 28 kB between pause 10 000 and 100 000, so the drift does not grow with the number of hits (ten times the hits, same drift). It looks like allocator settling.

`VmHWM` is not a usable peak here: at the end of the 10 000-hit run it reported 2444 kB, below the 2620 kB `VmRSS` read during the run. The kernel updates it lazily, so the per-pause `VmRSS` samples are the figure to use.

CPU time of the listener for 10 000 hits, 5 expressions:

| Run | /proc/self (user + sys) | cgroup `usage_usec` | Request time |
|---|---|---|---|
| sequential, `t08-cpu-mem-10k` | 410 ms (160 + 270, minus 20 at start) | not read | 0.760 s |
| sequential, `t08-cpu-cgroup` | 400 ms (170 + 240, minus 10 at start) | 450.4 ms (194.1 user, 256.3 sys) | 0.727 s |
| pipelined, `t08-cpu-cgroup` | 240 ms (140 + 110, minus 10 at start) | 289.9 ms (159.4 user, 130.5 sys) | 0.422 s |
| sequential, 100 000 hits | 4000 ms (1510 + 2500, minus 10 at start) | not read | 7.15 s |

That is about 40 us of listener CPU per hit in sequence (6 round trips per hit, more than half of it in the kernel) and about 24 us pipelined. CPU grows linearly with hits.

Result: data for the report. Memory stays flat once the buffer is full.

## Criterion 4: hit conditions as a cap (T-09)

```sh
./t09.sh 3
```

Method: bridge, n=10000, 5 expressions, 3 requests per scenario, median of `time_total`. Then short loops (n=100) with `--bp-get`, which reads `hit_count` with `breakpoint_get` on each pause.

| Breakpoint | Pauses | Median request (ms) |
|---|---|---|
| none (session only) | 0 | 5.35 |
| no condition | 10000 | 683.38 |
| `-h 9991 -o >=` | 10 | 9.52 |
| `-h 5000 -o ==` | 1 | 10.14 |
| `-h 1000 -o %` | 10 | 11.83 |
| `-h 20000 -o ==` (never matches) | 0 | 9.09 |
| no condition, `breakpoint_remove` at the 10th pause (`--cap 10`) | 10 | 8.82 |

On the short loops, `>= 95` paused 6 times (hits 95 to 100), `== 50` once, `% 10` 10 times, with `hit_count` 100, 50 and 100 at the last pause. The captured `$i` matched the expected hit in every pause (0 mismatches in all T-09 sessions, 24 requests all HTTP 200).

Xdebug evaluates the condition itself and pauses only on matching hits. A breakpoint that does not pause still costs something: 9.09 ms against 5.35 ms for 10 000 visits of the line, about 0.4 us per visit.

DBGp has no `<=` or `<` operator, so none of the three conditions caps a logpoint at its first K hits. `>=` captures everything from hit K on, `==` captures one hit, and `%` samples every K-th hit without a limit. A cap of K captures works by removing the breakpoint from the listener at the K-th pause: one extra round trip, once, and the request time drops to the no-pause level (8.82 ms). `breakpoint_update -s disabled` should do the same but was not tested.

Result: hit conditions avoid pausing on non-matching iterations. A cap needs the client to remove or disable the breakpoint.

## Criterion 5: concurrent requests (T-10)

```sh
./t10.sh
```

FPM settings in php85 (`www.conf`): `pm = dynamic`, `pm.max_children = 5`, `pm.start_servers = 2`, `pm.min_spare_servers = 1`, `pm.max_spare_servers = 3`.

Method: bridge, n=10000, 5 expressions. A sequential reference run, then requests fired at the same time with `PARALLEL=1`. The listener prints each session's start and end in ms from its own start, and the `appid` from the `init` packet (the FPM worker PID).

| Run | Request times (s) | Sessions |
|---|---|---|
| sequential, 3 requests | 0.703, 0.701, 0.717 | one after the other |
| 2 at once | 0.869, 0.882 | start 62.6 and 62.9 ms, end 929.7 and 942.8 ms, workers 260 and 261 |
| 2 at once, again | 0.824, 0.880 | start 63.8 and 65.0 ms, end 941.3 and 887.5 ms, workers 275 and 260 |
| 4 at once | 1.156, 1.159, 1.179, 1.382 | three start at 68.2 to 68.3 ms in workers 260, 261 and 275; the fourth starts at 384.9 ms in a new worker, 276 |
| long n=10000 and short n=100, short sent 100 ms later | long 0.731, short 0.010 | short ends 125 ms after the long one started, long ends at 754 ms |

Every session had its own capture buffer and 0 mismatches, and every request returned HTTP 200 with the right sum. The sessions overlap in time. If they were serialised, two 0.7 s requests would take about 1.4 s for the second one, and the short request would have finished after the long one. Running in parallel, each request took 17 to 25% longer than alone (0.82 to 0.88 s against 0.70 s); the cause was not measured.

With 4 at once, only 3 workers were free (2 start servers plus the one spawned earlier), and FPM's dynamic mode added a fourth about 317 ms later. The delay is on the FPM side: the listener accepted the other three sessions at once, and the fourth ran normally once its worker connected.

Result: pass.

## Criteria 6 and 7: how to rerun (T-11, T-12)

T-11 and T-12 use a second binary, `/target/release/probe` (`src/bin/probe.rs`). It accepts one session, sets one line breakpoint (optionally `--hit N`), and at the pause sends the DBGp commands of a file under `scripts/`, printing for each one the response size in bytes, the time from write to the end of the response, and the decoded property tree. `probe.sh` starts it and fires the request. The fixture has no private, protected or static properties and no constants, so `probe/visibility.php` adds them; `compose.override.yaml` mounts it at `/probe` in php85, and it runs with the PHP CLI. The test bed must be started with the override for that one:

```sh
cd spikes/testbed
COMPOSE_PROJECT_NAME=testbed-s2 HTTP_PORT_85=8280 HTTP_PORT_83=8281 \
  docker compose -f compose.yaml -f ../s2-logpoints/compose.override.yaml up -d --build
cd ../s2-logpoints && ./profile.sh bridge
./probe.sh /var/www/html/public/index.php:44 t11-loop.txt 'url:/loop?n=5' --hit 3 > results/t11-loop.txt
./probe.sh /var/www/html/src/Graph.php:40 t11-graph.txt 'url:/graph?depth=6&width=3' > results/t11-graph.txt
./probe.sh /var/www/html/src/OrderService.php:44 t11-orders.txt 'post:/orders:customer_id=3' > results/t11-orders.txt
./probe.sh /probe/visibility.php:33 t11-visibility.txt 'cli:/probe/visibility.php' > results/t11-visibility.txt
./probe.sh /var/www/html/src/Graph.php:40 t12-depth.txt 'url:/graph?depth=6&width=3' --lines 6 > results/t12-depth.txt
./probe.sh /var/www/html/src/Graph.php:40 t12-children.txt 'url:/graph?depth=3&width=50' --lines 4 > results/t12-children.txt
./probe.sh /var/www/html/src/Graph.php:40 t12-zero.txt 'url:/graph?depth=6&width=3' --lines 8 > results/t12-zero.txt
./probe.sh /var/www/html/public/index.php:44 t12-data.txt 'url:/loop?n=3' --lines 2 > results/t12-data.txt
./t12-table.sh results/t12-depth.txt   # one line per read, with the limits in force
```

During these runs the S3 test bed (`testbed-s3`) was up on the same machine.

## Seen along the way, for criteria 6 and 7

- `property_get -n` read `$item['square']` (an array element) and `$n` (a function argument) directly, with no `eval`. `$item` came back as an array with its two children at `max_depth=1`.
- `feature_set` for `max_depth`, `max_children` and `max_data` returned `success="1"` on Xdebug 3.5.3. `property_get` also takes `-m` (max data) per command, and the spike sends it on every read.
- Integers come back as CDATA text. Strings come back base64 encoded; the listener decodes them (`src/xml.rs`), but the fixture loop has no strings, so that path ran only in the unit test.
- `breakpoint_get` is refused once the script is stopping: `error code="5"`, "command is not available". Hit counts have to be read while paused.
- Xdebug answers commands sent in one write in order. Pipelining is a real option for the daemon (criterion 2 numbers above).
- The test bed default `log_level=7` adds about 14 us per hit. Product measurements should state the log setting.
- `S2_TRACE=1` (passed through by `bench.sh`) prints every DBGp packet, which is the starting point for measuring response sizes in T-12.

## Deviations and caveats

- The ini profiles are written into the php85 container's writable layer (`/usr/local/etc/php/conf.d/zzz-s2.ini`) with `docker compose exec`, and FPM is reloaded. No file under `spikes/testbed/` changed. Recreating the container drops the profile.
- Early on, a `php -v` run as root inside php85 created `/var/log/xdebug/xdebug.log` owned by root, which stops the FPM workers (www-data) from writing it. It was fixed with `chown www-data:www-data` before the log-on run. The other runs had the log off anyway.
- The "local" listener runs in a container that shares php85's network namespace. It is loopback, but still a containerised process.
- S1's test bed shared the machine during the runs.
