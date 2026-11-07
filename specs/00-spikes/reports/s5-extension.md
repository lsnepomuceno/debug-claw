# S5 PHP extension in Rust: report

Spike code: [`spikes/s5-extension/`](../../../spikes/s5-extension/). Its [`README.md`](../../../spikes/s5-extension/README.md) lists the scripts, and `results/` holds the summary of every run quoted here.

Short answer: yes, with a cost that has to be planned for. `ext-php-rs` 0.16.4 exposes the Observer API through its `observer` feature, so the extension needed no raw bindings. One source tree builds and runs on PHP 8.3 and 8.5. After 10 000 requests the worker's memory is flat, and `memory_get_usage` is the same as without the extension. Only `PDOStatement::execute` gets a handler, which the extension's counters show. The 5% budget does not hold on the fixture's 100-query endpoint, whose queries run on in-memory SQLite in about 0.5 us each: the extension adds 118 us per request on PHP 8.5 (34%) with the daemon up and 32 to 35 us (9 to 10%) with it down. It is 142 us and 69 to 73 us on PHP 8.3. In absolute terms that is 0.8 to 0.95 us per observed call with the daemon up and 0.2 to 0.5 us with it down, far under the 0.1 ms per call of the dump client budget. The bigger finding is that a loaded observer extension slows down every function call, observed or not, by 2.6 to 4.3 ns on PHP 8.5 and 5.4 to 8.2 ns on 8.3: 7 to 9% and 15% on the call-heavy `/graph` endpoint, with no handler running. Recommendation: build it, as the last layer of ADR-0005, with a switch that keeps the observer unregistered unless the user turns it on, and with the budget restated in absolute terms.

## Machine and versions

All runs on 2026-11-07, on one laptop.

| Item | Value |
|---|---|
| CPU | 12th Gen Intel Core i7-1260P, 12 cores (4 P and 8 E), 16 threads, governor `powersave` |
| Memory | 31 GiB |
| Kernel | Linux 7.0.0-38-generic |
| Docker | 29.9.2 |
| PHP | 8.5.12 and 8.3.36, FPM (`php:8.5-fpm-trixie`, `php:8.3-fpm-trixie`), NTS, OPcache on, no Xdebug |
| PHP ZTS | 8.5.12 (`php:8.5-zts-trixie`), CLI only, for the build check |
| FPM pool | `pm = static`, `pm.max_children = 1`, `pm.max_requests = 0` |
| nginx | 1.30-alpine, the shared test bed's `default.conf` |
| ext-php-rs | 0.16.4 with the `observer` feature (bindgen 0.73.2) |
| Rust | rustc 1.99.1, cargo 1.99.1, Debian clang 19.1.7 for bindgen |

The extension binary is 716 704 bytes (release build, not stripped).

During the timed runs each process of the request path had its own physical P-core: the listener on CPU 0, the PHP worker on CPU 2, nginx on CPU 4 and the load generator on CPU 6 (each of these shares its core only with its hyperthread sibling). Other agents ran an egui spike and an MCP spike on the same machine at the same time, and the `testbed-s3` and `s4-egui` containers were up. Their load may be in any timing below. To limit that, every T-26 run has 10 rounds of 2000 requests (after 200 untimed ones) per configuration, the order of the configurations rotates from one round to the next, and the report gives medians of all requests plus the spread of the per-round medians. The "paired" difference subtracts the `off` median of the same round, which removes most of the drift between rounds.

## How to rerun

```sh
cd spikes/s5-extension
export COMPOSE_PROJECT_NAME=testbed-s5      # host ports 8580 and 8581
docker compose up -d --build
./scripts/smoke.sh                          # criteria 1 and 2, results/smoke.txt
./scripts/overhead.sh php85 10 2000 200     # criteria 3 and 5, results/t26-php85.txt
./scripts/all.sh                            # every other T-26 and T-27 run
docker build -f docker/zts.Dockerfile -t testbed-s5-zts . && docker run --rm testbed-s5-zts
docker compose down
```

The PHP images build the extension inside the official PHP image (which has `php-config` and the headers) and copy it into a clean copy of that image. The configurations are folders under `ini/`, picked with `S5_PROFILE_85` and `S5_PROFILE_83`, and switching one recreates the container, so each run starts on a fresh worker. The shared test bed's app and nginx config are mounted read-only, and no file under `spikes/testbed/` changed.

## Criterion 1: an observer on `PDOStatement::execute` reports SQL and duration

The extension (`ext/src/lib.rs`) implements `ext-php-rs`'s `FcallObserver` trait and registers it with `ModuleBuilder::fcall_observer`. PHP calls `should_observe` once for each function the first time it runs in a request, and the extension answers true only for the internal method `execute` of class `PDOStatement`. `begin` pushes an `Instant` on a per-worker stack, and `end` pops it, reads the statement's `queryString` property and appends one JSON line to a per-worker buffer:

```
{"pid":7,"req":1,"ns":10470,"sql":"INSERT INTO products (name, price) VALUES (?, ?)"}
```

The buffer is capped at 64 KiB, and an event that does not fit is dropped and counted. At the end of the request (RSHUTDOWN) the extension writes the buffer in one non-blocking `write` on a TCP connection it keeps open per worker. It connects lazily, with a 1 ms timeout. A failed connect or write marks the daemon down for 1 s, and while it is down `end` skips reading the SQL and building the event. With `debug_claw.flush=call` it writes after every call instead, which is only there to measure what that costs. `debug_claw_s5_stats()` returns the counters as JSON.

`./scripts/smoke.sh` ran `app/smoke.php` on the CLI and `GET /queries?n=3` on FPM, on both versions, with the stand-in listener printing every line. All 114 lines arrived well formed (`0 malformed` in the listener's log), including a statement whose SQL has quotes, a tab and a newline (`/* \"quoted\"\n\tnext line */`), and an `execute` that throws (a duplicate primary key): it still produced its `end` and its event, 4 begins and 4 ends for 4 calls. On FPM the 3-query request gave 53 events (50 inserts and 3 selects) on one connection.

## Criterion 7: Observer API in `ext-php-rs`, and the build matrix

`ext-php-rs` exposes the Observer API directly. The `observer` feature registers `zend_observer_fcall_register` at MINIT and passes `FcallInfo` (function name, class name, file, line, whether it is internal) to `should_observe`, with no copy of the names. `begin` and `end` get the `ExecuteData` and the return value. The spike used no raw `ffi` call. The module also uses `ext-php-rs`'s INI entries, startup and request-shutdown hooks, a `#[php_function]` and `ZendObject::get_property`.

What was built and run here:

| Target | Result |
|---|---|
| Linux x86_64, glibc, PHP 8.5.12 NTS | Builds, runs on CLI and FPM |
| Linux x86_64, glibc, PHP 8.3.36 NTS | Builds from the same source, runs on CLI and FPM |
| Linux x86_64, glibc, PHP 8.5.12 ZTS | Builds, loads and runs the smoke test on the CLI (one thread) |
| The 8.5 binary loaded in PHP 8.3 | Refused: `Module compiled with module API=20250925`, `PHP compiled with module API=20230831` |

So a binary is tied to one PHP minor version and one thread-safety mode, as for any PHP extension, on top of the OS, CPU architecture and C library.

What `ext-php-rs` documents and tests (its README and `.github/workflows/build.yml`, read on 2026-11-07): Linux, macOS and Windows, PHP 8.1 or later, clang 5 or later for bindgen. The `build` job runs on Ubuntu, macOS and Windows, PHP 8.1 to 8.6, thread-safe and not, Rust stable and nightly. Windows runs only on nightly Rust (the `vectorcall` ABI needs the unstable `abi_vectorcall` feature, so an extension crate needs `#![cfg_attr(windows, feature(abi_vectorcall))]`); PHP 8.6 is not built on Windows. A `build-musl` job covers Alpine with PHP 8.1 to 8.5 and 8.6-rc, TS and NTS. FrankenPHP (ZTS) is tested with PHP 8.2 to 8.5, and there are AddressSanitizer and embed jobs on PHP 8.5.

A release matrix for PHP 8.2 to 8.5 would then be 4 minor versions times 2 thread-safety modes times each platform (Linux glibc and musl, macOS, Windows, and the CPU architectures of each), which is the friction ADR-0005 expected.

## Criterion 2: oldest and newest PHP version

Built and run on 8.3.36 and 8.5.12 (`results/smoke.txt`). The same source, the same `Cargo.lock` and no `#[cfg]` for the PHP version. Each version needs its own build, as above. The other criteria were measured on both versions unless stated.

## Criterion 3: overhead on a request with about 100 queries

`GET /queries?n=100` opens an in-memory SQLite database, runs 50 inserts and 100 selects through prepared statements, so 150 `execute` calls. Each one takes about 0.5 us once warm (the `ns` field of the smoke run's events). Configurations: `off` (not loaded), `on` (listener up, one write per request), `on-call` (one write per call), `closed` (daemon port closed, so the connect is refused at once) and `gone` (listener container stopped, so the connect times out after 1 ms).

PHP 8.5 (`results/t26-php85.txt`), 20 000 requests per configuration:

| Config | Median us | p95 us | Round medians us | Paired difference us (range over rounds) | Difference % |
|---|---|---|---|---|---|
| off | 344.4 | 432.8 | 289.7 to 388.9 | | |
| on | 463.2 | 537.4 | 408.5 to 492.9 | +118.4 (91.1 to 142.5) | +34 |
| on-call | 1098.9 | 1249.9 | 971.5 to 1163.0 | +746.7 (681.8 to 812.6) | +217 |
| closed | 381.3 | 444.6 | 323.9 to 403.8 | +32.1 (1.7 to 58.2) | +9 |
| gone | 377.7 | 435.3 | 325.3 to 403.6 | +35.4 (-13.1 to 48.4) | +10 |

PHP 8.3 (`results/t26-php83.txt`):

| Config | Median us | p95 us | Round medians us | Paired difference us (range over rounds) | Difference % |
|---|---|---|---|---|---|
| off | 340.3 | 418.3 | 285.8 to 377.0 | | |
| on | 482.2 | 532.9 | 427.0 to 500.2 | +142.2 (117.6 to 151.5) | +42 |
| on-call | 1133.9 | 1275.4 | 1000.6 to 1189.7 | +794.1 (690.5 to 832.8) | +233 |
| closed | 413.7 | 466.2 | 358.8 to 436.1 | +72.7 (54.9 to 81.7) | +21 |
| gone | 410.2 | 480.8 | 377.9 to 457.7 | +68.7 (52.2 to 100.8) | +20 |

The percentage is the paired difference over the pooled `off` median. The draft budget is at most 5% of the request, about 17 us here, so the extension misses it with the daemon up on both versions, and with it down on both versions too.

Per observed call, the added time with the daemon up is 118.4 / 150 = 0.79 us on 8.5 and 0.95 us on 8.3. For 5% to hold at that cost, the same request would have to take at least 2.4 ms on 8.5 and 2.8 ms on 8.3, that is, about 16 to 19 us per query instead of 0.5. One write per call (`on-call`) costs about 5 us per call, so the event stream has to be batched.

The CLI micro-benchmark `app/percall.php` (`results/cli-*.txt`, median of 3 rounds of 9 repetitions of 20 000 executes of `SELECT ?`) separates the per-call work from the per-request work:

| Config | PHP 8.5 ns per execute | PHP 8.3 ns per execute |
|---|---|---|
| off | 252 | 244 |
| on: read `queryString`, build the event (the CLI never flushes, so the buffer fills and events drop) | 542 (+290) | 536 (+292) |
| on-call: also one write per call | 4641 (+4389) | 4436 (+4192) |
| closed, flush per call: daemon down after the first call | 335 (+83) | 330 (+86) |
| gone, flush per call | 319 (+67) | 323 (+79) |

On FPM, 150 times 0.29 us is about 44 us of the 118 us that `on` adds on 8.5. The rest comes from what happens once per request, mostly the write of about 12.5 KB (150 events of about 84 bytes) at RSHUTDOWN, which runs before FPM finishes the response. That split is an inference from the two measurements; the write was not timed on its own. Reading `queryString` through `ZendObject::get_property` allocates `zend_string`s on every call, and the JSON is built in the hot path. Copying the SQL pointer and the time and building the JSON at RSHUTDOWN would move most of that cost out of the call, but it was not tried.

For the fixed cost, `GET /` (no PDO, 6 functions) on 8.5 (`results/t26-php85-root.txt`): `off` median 89.1 us, and the paired differences of the other configurations are 4.1 to 5.9 us with ranges that include zero, so within noise.

## Criterion 5: added time with the daemon down

The rows `closed` and `gone` above. With the daemon down the extension adds 32 to 35 us per request on 8.5 and 69 to 73 us on 8.3, that is 0.21 to 0.24 us and 0.46 to 0.48 us per observed call, against 0.1 ms per call for the dump client. A stopped listener (connect timeout) and a closed port cost the same, because the extension tries to connect once and then skips the daemon for 1 s; the counters show 1 connect attempt per 2200-request run. The 5% budget is not met: 9 to 10% on 8.5 and 20 to 21% on 8.3.

Most of the down cost is not in the handler. The CLI shows 67 to 86 ns per execute with the daemon down, about 10 to 13 us for 150 calls. The rest is the general cost of a loaded observer, in criterion 6.

## Criterion 4: worker memory after 10 000 requests

`./scripts/soak.sh SERVICE PROFILE` recreates the worker and sends 10 000 `GET /queries?n=100` through nginx, sampling `app/mem.php` (run on the same worker over FastCGI) before the first request and after every 1000 (`results/t27-*.tsv`). With the extension the listener was up and received every event (1 500 000 per run).

| PHP, config | VmRSS kB after 1000 | VmRSS kB after 10 000 | RssAnon kB after 1000 / 10 000 | `memory_get_usage` after 1000 / 10 000 |
|---|---|---|---|---|
| 8.5 off | 17 944 | 17 952 | 6128 / 6136 | 415 008 / 415 008 |
| 8.5 on | 18 604 | 18 632 | 6288 / 6316 | 415 008 / 415 008 |
| 8.3 off | 16 524 | 16 536 | 5296 / 5308 | 341 056 / 341 056 |
| 8.3 on | 17 100 | 17 116 | 5416 / 5432 | 341 056 / 341 056 |

From 1000 to 10 000 requests the worker grows by 8 to 12 kB without the extension and by 16 to 28 kB with it. On 8.5 the whole 28 kB came between request 1000 and 2000, with no change in the last 8000. `memory_get_usage` and `memory_get_usage(true)` (2 MiB) are identical with and without the extension at every sample. So the worker's memory matches within noise. Loading the extension costs about 680 kB of RSS on 8.5 and 580 kB on 8.3 once, of which 180 kB and 124 kB are anonymous memory; the rest is the mapped binary. The 64 KiB event buffer is part of that, and it does not grow (`pending_capacity` stays 65 536).

## Criterion 6: unobserved functions get no handler

The extension counts every `should_observe` call (`init_calls`), every true answer (`init_observed`) and every `begin` and `end`. Over 10 000 requests of `/queries?n=100` on 8.5: `init_calls` grew by 18 011 per 1000 requests, `init_observed` by exactly 1000, and `begin_calls` and `end_calls` by exactly 150 000, the number of `execute` calls. On 8.3: 19 011, 1000 and 150 000. On `/graph` and `/` (no PDO), `init_observed`, `begin_calls` and `end_calls` stayed 0 while `init_calls` grew by 16 and 6 per request. So PHP asks once per function per request, the answer is no for every function but one, and no handler of the extension runs for them.

Not running a handler still costs something. PHP checks every call for observer handlers once any observer is registered. `app/calls.php` times calls of functions the extension does not observe (CLI, median of 3 rounds, ns per call):

| Call | 8.5 off | 8.5 loaded | 8.3 off | 8.3 loaded |
|---|---|---|---|---|
| User function | 15.05 | 19.03 | 14.38 | 22.55 |
| User method | 17.08 | 20.73 | 17.25 | 24.04 |
| Closure | 22.35 | 25.64 | 21.34 | 27.84 |
| `str_repeat` | 25.84 | 30.15 | 24.61 | 30.06 |
| `max` | 9.51 | 12.07 | 13.02 | 19.89 |

On a request that is mostly function calls, `GET /graph` (`results/t26-php85-graph.txt`, `results/t26-php83-graph.txt`), the extension adds 33.5 to 39.8 us on 8.5 (7 to 9% of a 447.5 us median) and 60.2 to 63.2 us on 8.3 (15% of 405.5 us), the same in every configuration, daemon up or down. Whether a C extension with the same no-handler observer costs the same was not measured, but the counters show no Rust code running for these calls.

## Not verified

- Windows, macOS, Alpine (musl) and any arm64 build. The matrix above for those comes from `ext-php-rs`'s documentation and CI, not from builds here.
- ZTS with more than one thread (FrankenPHP, a threaded SAPI). The per-worker state is thread-local, so it should hold, but only a one-thread CLI run was done.
- PHP 8.1, 8.2 and 8.4, and the JIT.
- More than one FPM worker, and concurrent requests.
- A real database (MySQL, PostgreSQL), where the query time would make the per-call cost a smaller share.
- Bindings, the caller's file and line, `PDO::query`, `PDO::exec` and mysqli. Only `PDOStatement::execute` and its SQL text were observed.
- The cost split between the event building and the RSHUTDOWN write on FPM, and the cheaper event building described under criterion 3.
- Whether the observer-check cost of criterion 6 is the same for an extension written in C.

## Recommendation

Build the extension, as the last layer of [ADR-0005](../../decisions/0005-layered-php-integration.md) and in the milestone order the ADR already gives it. The risks S5 was meant to rule out are ruled out: `ext-php-rs` gives the Observer API without raw bindings, one source builds for 8.3 and 8.5, memory does not grow per request, and a down daemon costs a fraction of a microsecond per call. Two findings shape the design:

- Loading the extension slows every function call down, by 2.6 to 4.3 ns on 8.5 and 5.4 to 8.2 ns on 8.3, even when nothing is observed and the daemon is down. The extension should read an INI switch at MINIT and register no observer unless the user turned it on, so a machine with the extension installed but not in use pays nothing for it.
- A relative budget on the fixture's in-memory SQLite endpoint cannot be met, because the extension's 0.8 us per call is more than the query. The budget should be absolute per call and per request, with the relative one measured on an endpoint whose queries take a realistic time.

## Spec changes

ADRs:

- Accept [ADR-0005](../../decisions/0005-layered-php-integration.md). Layer 3 is feasible. Its Consequences could add: one binary per PHP minor version, thread-safety mode and platform (Windows builds need nightly Rust); a loaded observer costs every function call a few nanoseconds, so the extension registers its observer only when an INI switch turns it on; events go out in one write per request, never one per call.

[OPEN.md](../../OPEN.md):

- O-02: the extension does not set the minimum PHP version. `ext-php-rs` supports PHP 8.1 and later, and one source builds for 8.3 and 8.5 with no version-specific code. Each supported minor version adds a binary per thread-safety mode and platform. PHP 8.3 pays about twice the observer cost of 8.5 (criteria 5 and 6).
- O-06: an observer-based profiler would put a Rust handler on every function. A handler that only reads the clock and checks a flag costs 67 to 86 ns per call here (the down path of criterion 3), on top of the 2.6 to 8.2 ns of the observer check, which is several times the cost of a PHP function call (14 to 25 ns). That argues for a sampling profiler (excimer, or sampling inside the extension, which S5 did not try) over observing every call, and for the observer only on chosen functions.
- O-08, numbers for the Extension row of the [budget table](../../../docs/performance.md#budgets), which today says at most 5% added request time on a request with about 100 PDO queries:
  - Measured on the 100-query fixture endpoint: 0.79 us (8.5) and 0.95 us (8.3) added per observed call with the daemon up; 0.21 to 0.24 us and 0.46 to 0.48 us with it down; about 680 kB and 580 kB of RSS once per worker; no growth of `memory_get_usage` per request.
  - Proposed: at most 1 us added per observed call with the daemon up and 0.5 us with it down, at most one write per request, and no memory growth per request (worker RSS within 64 kB from request 1000 to 10 000, `memory_get_usage` unchanged). Keep the 5% figure only for an endpoint with realistic query times, which the test bed does not have yet.
  - A new row: the extension installed but switched off adds no time per call. With the switch of the recommendation that should be zero; without it, it is 7 to 9% (8.5) and 15% (8.3) on `/graph`.

[docs/features.md](../../../docs/features.md):

| Row | Result |
|---|---|
| Database queries with bindings, duration and origin; duplicates (N+1) and slow queries marked | SQL text and duration per `PDOStatement::execute` work. Bindings, origin, `PDO::query`, `PDO::exec` and mysqli were not tried. |
| Request list: method, URI, status, duration, peak memory, web or CLI | Not measured, but the extension already has the per-request hooks (RSHUTDOWN) and one write per request where these fields would go. |
| New row: extension status (loaded, version, events sent and dropped, daemon reachable) in `status` | The counters exist in the spike (`debug_claw_s5_stats()`), and dropped events need to be visible because the buffer drops events when full and while the daemon is down. |
