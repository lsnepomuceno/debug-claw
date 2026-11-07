# S2 Logpoints over DBGp: report

Spike code: [`spikes/s2-logpoints/`](../../../spikes/s2-logpoints/). The raw output of every run is in its `results/` folder. [`NOTES.md`](../../../spikes/s2-logpoints/NOTES.md) has the working notes for criteria 1 to 5 and the commands for every run.

Short answer: yes. A logpoint built as a line breakpoint, a few `property_get` reads and an immediate `run` costs 64 to 77 us per hit with 5 expressions sent one at a time, and 41 to 45 us with the reads pipelined, far under the 5 ms budget. The daemon's memory stays flat once the capture buffer is full, and concurrent requests do not wait for each other. Most logpoint expressions people write (variables, properties, array keys, statics through `$this`, constants) can be read without `eval`, but the daemon has to check the expression itself, because Xdebug accepts some invalid names without an error. The DBGp size limits bound a read only at `max_depth` 1. Deeper reads need a cap in the daemon.

## Machine and versions

All runs on 2026-11-07, on one laptop.

| Item | Value |
|---|---|
| CPU | 12th Gen Intel Core i7-1260P, 12 cores (4 P and 8 E), 16 threads, governor `powersave` |
| Memory | 31 GiB |
| Kernel | Linux 7.0.0-38-generic |
| Docker | 29.9.2 |
| PHP | 8.5.12 FPM (`php:8.5-fpm-trixie`), NTS, OPcache with default settings |
| Xdebug | 3.5.3, `mode=debug`, `start_with_request=trigger`, log off unless stated (`ini/bridge.ini`, `ini/local.ini`) |
| nginx | 1.30-alpine |
| Rust | cargo 1.99.1 in the `toolchain` image, no dependencies |

Only PHP 8.5 was used. The S1 spike's test bed (`testbed-s1`) ran on the same machine during the runs for criteria 1 to 5, and the S3 test bed (`testbed-s3`) during the runs for criteria 6 and 7. Their requests may have competed for CPU, so single timings are noisy. The time figures below are medians.

## How to rerun

```sh
cd spikes/testbed
export COMPOSE_PROJECT_NAME=testbed-s2 HTTP_PORT_85=8280 HTTP_PORT_83=8281
docker compose -f compose.yaml -f ../s2-logpoints/compose.override.yaml up -d --build
docker compose exec toolchain sh -c 'cd spikes/s2-logpoints && cargo test --release && cargo build --release'
cd ../s2-logpoints
./matrix.sh local 5          # criterion 2, sets the Xdebug profile itself
./matrix.sh bridge 5
./profile.sh bridge          # Xdebug log off, client_host=toolchain
./t09.sh 3                   # criterion 4
./t10.sh                     # criterion 5
```

The commands for criteria 1, 3, 6 and 7 are in `NOTES.md`. There are two binaries. `s2-logpoints` is the logpoint listener for criteria 1 to 5: it sets a breakpoint on `index.php:44` (`$sum += $item['square'];` inside `loop()`), reads 1 or 5 expressions per pause, checks every value against what the loop computes, keeps them in a ring buffer (1000 captures by default) and sends `run`. `probe` is used for criteria 6 and 7: it pauses once, sends the DBGp commands of a file under `scripts/` and prints each response's size, time and decoded properties. `compose.override.yaml` mounts `probe/visibility.php`, a CLI script with the private, protected and static properties and the constants that the fixture does not have. No file of the shared test bed changed.

## Criterion 1: a logpoint in a loop captures every hit

`results/t07.txt`: 3 requests to `GET /loop?n=100` over the bridge network and 3 over loopback, 5 expressions each (`$item`, `$i`, `$sum`, `$n`, `$item['square']`). All 6 returned HTTP 200 with the right `sum`, and every session paused 100 times with 0 mismatches:

```
#1[{index=0,square=0} | 0 | 0 | 100 | 0]
#100[{index=99,square=9801} | 99 | 318549 | 100 | 9801]
```

The listener checks every capture, not only the first and the last. Across all runs in `results/` (up to 100 000 hits in one request) there was no mismatch and no response other than HTTP 200. After the last `run` Xdebug answers `status="stopping"`, the listener sends `stop`, and the request ends with its normal body.

Result: pass.

## Criterion 2: added time per hit

`results/t08-local.txt` and `results/t08-bridge.txt`, from `matrix.sh`. `curl` on the host measures `time_total` through nginx, 5 requests per scenario, median kept. The added time per hit is the median with the breakpoint minus the median of a session without a breakpoint, divided by the number of hits. "local" is a listener in php85's network namespace on `127.0.0.1`. "bridge" is the listener in the `toolchain` container across the Compose network, which stands for a connection into a container.

Added time per hit, in microseconds:

| Expressions | local, n=1000 | local, n=10000 | bridge, n=1000 | bridge, n=10000 |
|---|---|---|---|---|
| 0 (pause and continue) | 15.4 | 10.3 | 16.6 | 11.5 |
| 1 | 35.2 | 25.7 | 34.6 | 27.3 |
| 5, one command at a time | 64.6 | 63.9 | 76.9 | 71.8 |
| 5, pipelined | 41.0 | 40.5 | 45.2 | 44.2 |

"Pipelined" writes the five `property_get` commands and the `run` in one write and reads the answers after. Xdebug 3.5.3 answers them in order, and this cuts the cost by 35 to 40%. On the listener side, at n=10000 with 5 reads in sequence, the mean time between the break and the `run` was 58.1 us (local) and 63.3 us (bridge); p99 at most 160 us, and the largest single value 1.1 ms. With the test bed's default Xdebug log (`log_level=7`) the bridge figure at n=1000 was 91.0 us instead of 76.9 us (`results/t08-log-on.txt`).

Budget: at most 5 ms per hit with 5 expressions on the same host. The worst figure, 76.9 us, is about 1.5% of it. A daemon on the host talking to PHP in a container was not measured, because nothing runs on the host.

Result: pass on both connection kinds.

## Criterion 3: daemon CPU and memory over 10 000 hits

`results/t08-cpu-mem-10k.txt`, `t08-cpu-mem-100k.txt` and `t08-cpu-cgroup.txt`. The listener reads its own `VmRSS` every 1000 (or 10 000) pauses and its `utime` and `stime` at start and end, so nothing polls. `cgroup-cpu.sh` reads the toolchain container's `cpu.stat` before and after as a second source.

RSS with a buffer of 1000 captures of 5 values:

| Pause | 1000 | 2000 | 4000 | 6000 | 8000 | 10 000 |
|---|---|---|---|---|---|---|
| VmRSS (kB), 10 000 hits | 2592 | 2596 | 2600 | 2608 | 2620 | 2620 |

| Pause | 10k | 20k | 40k | 60k | 80k | 100k |
|---|---|---|---|---|---|---|
| VmRSS (kB), 100 000 hits | 2560 | 2560 | 2572 | 2580 | 2588 | 2588 |

RSS at start was about 2.1 MB and the buffer is full from pause 1000 on. After that RSS moved by 28 kB between pause 1000 and 10 000, and by the same 28 kB between pause 10 000 and 100 000, so it does not grow with the number of hits. `VmHWM` is not usable here: the kernel updates it lazily, and it reported less than the `VmRSS` read during the run.

CPU time of the listener for 10 000 hits with 5 expressions:

| Run | /proc/self, user + sys | cgroup `usage_usec` | Request time |
|---|---|---|---|
| one command at a time | 400 to 410 ms | 450.4 ms | 0.73 to 0.76 s |
| pipelined | 240 ms | 289.9 ms | 0.42 s |
| one command at a time, 100 000 hits | 4000 ms | not read | 7.15 s |

That is about 40 us of daemon CPU per hit with 6 round trips (more than half of it in the kernel), and about 24 us pipelined. CPU grows linearly with hits.

Result: memory stays flat once the buffer is full. CPU is about 40 us per hit, 24 us pipelined.

## Criterion 4: hit conditions as a cap

`results/t09.txt`, bridge, n=10000, 5 expressions, median of 3 requests:

| Breakpoint | Pauses | Median request (ms) |
|---|---|---|
| none (session only) | 0 | 5.35 |
| no condition | 10 000 | 683.38 |
| `-h 9991 -o >=` | 10 | 9.52 |
| `-h 5000 -o ==` | 1 | 10.14 |
| `-h 1000 -o %` | 10 | 11.83 |
| `-h 20000 -o ==` (never matches) | 0 | 9.09 |
| no condition, `breakpoint_remove` at the 10th pause | 10 | 8.82 |

Xdebug evaluates the hit condition itself and pauses only on matching hits. A breakpoint that does not pause still costs about 0.4 us per visit (9.09 ms against 5.35 ms for 10 000 visits). On short loops (n=100) with `breakpoint_get` at each pause, `>= 95` paused 6 times, `== 50` once and `% 10` ten times, and `hit_count` matched.

DBGp has no `<` or `<=` operator, so no hit condition stops a logpoint after its first K hits. `>=` captures everything from hit K on, `==` one hit, and `%` every K-th hit with no end. A cap of K captures works by removing the breakpoint at the K-th pause: one extra round trip, once, after which the request runs at the no-pause cost (8.82 ms). `breakpoint_get` is refused once the script is stopping (error 5), so hit counts have to be read while paused.

Result: hit conditions avoid pausing on iterations that do not match. A cap needs the daemon to remove the breakpoint at the K-th capture (`breakpoint_update -s disabled` should also work but was not tested).

## Criterion 5: concurrent requests

`results/t10.txt`, bridge, n=10000, 5 expressions. FPM in php85 runs `pm = dynamic` with `pm.max_children = 5` and `pm.start_servers = 2`.

| Run | Request times (s) | Sessions |
|---|---|---|
| sequential, 3 requests | 0.703, 0.701, 0.717 | one after the other |
| 2 at once | 0.869, 0.882 | start at 62.6 and 62.9 ms, end at 929.7 and 942.8 ms, workers 260 and 261 |
| 4 at once | 1.156, 1.159, 1.179, 1.382 | three start at 68.2 to 68.3 ms; the fourth at 384.9 ms in a new worker |
| long n=10000 and short n=100, short sent 100 ms later | long 0.731, short 0.010 | short ends 125 ms after the long one started |

Each session runs on its own thread with its own capture buffer, and all had 0 mismatches and HTTP 200. The sessions overlap: serialised, the second of two 0.7 s requests would take about 1.4 s, and the short request would have ended after the long one. Each parallel request took 17 to 25% longer than alone; the cause was not measured. With 4 at once only 3 FPM workers existed, and FPM spawned the fourth about 317 ms later; the listener accepted the others at once.

Result: pass.

## Criterion 6: expressions readable without eval

Probe runs (`results/t11-*.txt`), each at one pause:

- `t11-loop.txt`: `index.php:44`, third visit (`--hit 3`) of `GET /loop?n=5`, so `$i = 2`, `$item = ['index' => 2, 'square' => 4]`, `$sum = 1`, `$n = 5`.
- `t11-graph.txt`: `Graph.php:40` in `Graph::summary()`, `GET /graph?depth=6&width=3`.
- `t11-orders.txt`: `OrderService.php:44` in `charge()`, `POST /orders` with `customer_id=3`.
- `t11-visibility.txt`: `probe/visibility.php:33` in `Account::balance()`, PHP CLI.

`property_get -n NAME` reads by name from context 0 (locals) unless `-c` is given. A name it cannot resolve answers `error code="300"` ("can not get property").

| Expression | Example tried | `property_get` answer |
|---|---|---|
| Local variable, function argument | `$i`, `$sum`, `$n`, `$local` | Value (`2`, `1`, `5`, `"balance is 10.5"`) |
| Array, string key | `$item['square']`, `$item["index"]` | `4`, `2`. Both quote styles work |
| Array, int key, nested keys | `$this->tags[7]`, `$this->nested['level1']['level2'][0]`, `$this->roots[0]->meta['tags'][1]` | `"seven"`, `"deep"`, `"b"` |
| Public property, chained | `$order->total`, `$this->roots[0]->children[1]->parent->name`, a path 6 nodes deep | `0`, `"root-0"`, `"root-2.2.2.2.2.2"` |
| Readonly property | `$order->customerId` | `3`; the tree shows `facet="public readonly"` |
| Private and protected property of the class | `$this->secret`, `$this->balance`, `$this->next->secret` | `"account-private"`, `10.5`, `"account-private"` |
| Private property of the parent class | `$this->baseSecret` | Error 300 |
| | `$this->*Base*baseSecret` (the `fullname` Xdebug gives it) | `"base-private"` |
| `$this` | `$this` | The object with its properties and statics |
| Static property through `$this` | `$this::currency`, `$this::registry`, `$this::instances` | `"EUR"`, the array, `1`. Public, private and parent protected statics all work |
| Static property by class | `self::$currency`, `self::currency`, `Account::$currency`, `\Account::$currency`, `static::currency` | Error 300 (S1 got the same) |
| Superglobal entry, context 0 | `$_GET['n']`, `$_SERVER['REQUEST_URI']` | Error 300 |
| Superglobal entry, context 1 | `-c 1 -n $_GET['n']`, `-c 1 -n $_SERVER['REQUEST_URI']` | `"5"`, `"/loop?n=5"` |
| `$GLOBALS` | `$GLOBALS`, `$GLOBALS['path']`, also with `-c 1` | Error 300 |
| Constant, context 0 | `PHP_VERSION`, `APP_MODE` | Error 300 |
| Constant, context 2 | `-c 2 -n` with `APP_MODE`, `APP_LEVEL`, `PHP_VERSION`, `PHP_INT_MAX`, `M_PI`, `E_ALL` | Every one, user and built-in, with `facet="constant"` |
| Class constant | `self::KIND`, `Account::KIND`, `$this::class`, `self::class` | Error 300 |
| Method and function calls | `$this->summary()`, `$this->discount(3)`, `count($item)`, `strlen('abc')` | Error 300 |
| Arithmetic, comparison, cast, `??` on a plain variable | `$i + 1`, `$i+1`, `$this->balance * 2`, `$order->total <= 0`, `(int)$i`, `$i ?? 0` | Error 300 |
| String interpolation | `"i=$i"`, `{$order->total}` | Error 300 |
| Missing variable, key or property | `$nothing`, `$item['missing']`, `$this->roots[9]`, `$this->roots[0]->missing` | Error 300 |

Two answers are wrong without an error, and they decide how a logpoint parser has to work:

- Text after a closing `]` is ignored. `$item['square'] * 2` returned `4` (eval of the same returns `8`), and `$item['square'] + 100`, `$item['square']garbage`, `$item['square'] . 'x'` and `$this->tags[7] . 'x'` each returned the plain element. Text after a name without brackets (`$this->balance * 2`, `$local garbage`) does answer error 300.
- An unquoted key gives a wrong value or an error. `$item[square]` and `$this->tags[x]` returned the whole array under the given name, and `$this->nested[level1]` answered error 300.

Error 300 also means "unset", so it cannot tell a missing value from an expression that is not a name. Every call, operator and interpolation above returned the right value through `eval`, for example `"i=$i sum=$sum"` gave `"i=2 sum=1"`, `self::KIND` gave `"base"` and `$this->discount($order->customerId)` gave `1` (a float). Across the four runs a single `property_get` took 11 to 263 us and a single `eval` 13 to 75 us. These are one sample each, too noisy to compare.

The rule a logpoint parser can use: send `property_get` only when the whole expression matches the grammar below, and otherwise treat it as `eval` (refused while eval is off, [O-05](../../OPEN.md#o-05-eval-policy)). The check runs in the daemon when the logpoint is set, not by sending the name and waiting for error 300.

```
path      = base segment*
base      = "$" identifier                         ; includes $this; context 0
          | superglobal                            ; context 1: $_GET, $_POST, $_COOKIE,
                                                   ;   $_SERVER, $_FILES, $_ENV, $_REQUEST, $_SESSION
segment   = "->" identifier
          | "->*" class-name "*" identifier        ; parent private, as Xdebug names it
          | "::" identifier                        ; static property, after $this only
          | "[" integer "]"
          | "[" "'" chars "'" "]" | "[" '"' chars '"' "]"
constant  = identifier                             ; no "$": property_get -c 2
```

Anything else goes to `eval`: calls, operators, casts, `??`, interpolation, class constants, `Class::$static`, `self::`, `static::`, `::class` and `$GLOBALS`. A message template such as `total={$order->total} sku={$order->items[0]['sku']}` splits into literal text and path expressions, and needs no eval. Only `$_GET` and `$_SERVER` of the superglobals were tried, and a static read through another object (`$this->next::currency`, a `fullname` the tree reports) was not read by name.

Result: variables, properties of any visibility, quoted array keys, statics through `$this`, superglobal entries (context 1) and constants (context 2) need no eval. Calls, arithmetic, interpolation and class constants do.

## Criterion 7: response sizes and default limits

Probe runs `results/t12-depth.txt`, `t12-children.txt`, `t12-zero.txt` and `t12-data.txt`; `t12-table.sh` turns each into one line per read. Sizes are the DBGp XML packet in bytes. Times are the median of 5 reads (3 for the largest), from writing the command to reading the end of the answer, over the bridge network.

The deepest fixture object is `$this` in `Graph::summary()` on `GET /graph?depth=6&width=3`: a `Graph` with 3 roots and 1092 `Node` objects. Each node takes two levels of the property tree (the object, then its `children` array), and each node has a `parent` link. Xdebug follows the parent links without a cycle check (`$this->roots[0]->children[0]->parent` is expanded again in full), so with no depth limit the tree never ends, and only `max_depth` stops it.

`property_get -n $this` by `max_depth`, with Xdebug's defaults for the rest (`max_children` 32, `max_data` 1024), and with children and data not limited (`max_children` 100000, `max_data` 0):

| `max_depth` | Bytes, defaults | Time (us) | Properties | Bytes, children and data not limited | Time (us) |
|---|---|---|---|---|---|
| 1 | 547 | 40 | 3 | 551 | 49 |
| 2 | 942 | 48 | 6 | | |
| 3 | 2 616 | 92 | 18 | | |
| 4 | 5 187 | 155 | 36 | 5 231 | 137 |
| 5 | 12 240 | 286 | 78 | | |
| 6 | 26 791 | 472 | 168 | | |
| 7 | 54 556 | 812 | 321 | | |
| 8 | 107 383 | 1 906 | 609 | 108 123 | 1 953 |
| 10 | | | 2 121 | 409 703 | 5 152 |
| 12 | | | 7 035 | 1 475 693 | 19 230 |
| 14 | | | 19 995 | 4 541 745 | 57 431 |
| 16 | | | 36 519 | 8 661 405 | 163 156 |
| 18 | | | 50 127 | 12 313 452 | 221 056 |
| 20 | | | 55 230 | 13 759 059 | 265 838 |

(The properties column from depth 10 on is for the unlimited reads.) `context_get -c 0` at the same pause has only `$this` and is 11 bytes larger than `property_get -n $this` at every depth. The size about doubles with each level, and at `max_depth` 20 one read is 13.8 MB and takes 266 ms with the request paused. In this tree the width is 3, so `max_children` 32 changes nothing, and the names are at most 16 characters, so `max_data` changes almost nothing (`max_depth` 6: 26 756 bytes with `max_data` 4, 26 792 with 64 or 0).

A wide object, `GET /graph?depth=3&width=50` (50 roots, 50 children per node), `property_get -n $this->roots`, `max_data` 1024:

| `max_depth` | `max_children` 10 | `max_children` 32 | `max_children` 100 |
|---|---|---|---|
| 1 | 1 564 B, 11 props | 4 336 B, 33 props | 6 605 B, 51 props |
| 2 | 7 154 B, 51 props | 22 400 B, 161 props | 34 925 B, 251 props |
| 3 | 25 523 B, 181 props | 181 919 B, 1 281 props, 2.5 ms | 412 991 B, 2 901 props, 5.1 ms |

Long strings, which the fixture does not have, built with `eval` at `index.php:44` (`eval` results honour the same limits):

| Value | Limits (depth, children, data) | Bytes |
|---|---|---|
| `str_repeat('x', 100000)` | 1, 32, 1024 | 1 626 |
| `str_repeat('x', 100000)` | 1, 32, 0 | 133 594 |
| `array_fill(0, 32, str_repeat('x', 5000))` | 1, 32, 1024 | 46 816 |
| same | 1, 32, 512 | 24 928 |
| same | 1, 32, 256 | 14 048 |
| same | 1, 8, 256 | 3 705 |
| same | 1, 32, 0 | 216 416 |
| `array_fill(0, 32, array_fill(0, 32, str_repeat('x', 5000)))` | 2, 32, 512 | 808 608 |

Strings travel base64 encoded, so a string costs about 4/3 of `max_data` plus about 100 bytes of attributes. The superglobals context at the defaults (`context_get -c 1` on `/loop`) is 13 256 bytes, mostly `$_SERVER` with 43 entries cut to 32.

How truncation is signalled:

- Children: `numchildren` is always the full count. The requested property carries `page` and `pagesize` (equal to `max_children`) and at most `pagesize` children. A property at the depth limit carries `numchildren` and no children. The client knows a list is cut when `numchildren` is larger than the number of child elements.
- Paging: `property_get -n NAME -p N` returns page N of the requested property's children. On 50 roots with `max_children` 32, `-p 0` gave `$this->roots[0]` to `[31]`, `-p 1` gave `[32]` to `[49]`, and `-p 2` gave the property with no children. Paging applies only to the property named in the command; a nested child is reached by reading its `fullname` (`$this->roots[45]->name` answered `"root-45"`).
- Strings: `size` is the full length and the value is cut to `max_data` bytes, with no other mark. `-m 0` on `property_get` or `property_value` returns the full value for one command.
- Zero: `max_depth` 0 returns the property with no children, `max_children` 0 returns no children (`pagesize="0"`), and `max_data` 0 means no limit.

Proposed defaults, sent with `feature_set` at the start of every session:

| Setting | Value | Why |
|---|---|---|
| `max_depth` | 1 | The only depth at which the DBGp limits bound a read. Deeper levels are fetched on demand by `fullname`. |
| `max_children` | 32 | Xdebug's default, and the page size for `-p`. |
| `max_data` | 512 | The worst measured read at depth 1 (32 strings of 5000 characters) is 24 928 bytes, under 32 KB. Xdebug's default of 1024 gives 46 816 bytes, over it. |

For logpoint captures, which stay in a buffer of many entries, `max_children` 8 and `max_data` 256 per value, sent as `-m 256` on each `property_get`: the worst measured value is 3 705 bytes.

These limits bound one read only at `max_depth` 1. Any deeper read, a `context_get` with many locals, or a superglobals context can pass 32 KB (808 608 bytes at depth 2 with 32 children). The daemon therefore also has to cut its MCP result at 32 KB after converting it, mark where it cut, and offer `-p` pages for the children of the top property and `fullname` reads for anything deeper. The figures above are DBGp XML. The MCP result will be JSON with decoded strings and shorter names, and its size was not measured.

Result: the defaults are `max_depth` 1, `max_children` 32 and `max_data` 512, plus a 32 KB cut in the daemon for anything deeper.

## Not verified

- PHP 8.3, and Xdebug versions other than 3.5.3. Criteria 6 and 7 ran on PHP 8.5 only.
- A daemon on the host talking to PHP in a container (nothing runs on the host), and Windows or macOS hosts.
- Objects with `__get`, `ArrayAccess` or `__debugInfo`: whether `property_get` reads the stored properties or calls PHP code.
- Superglobals other than `$_GET` and `$_SERVER`, and `$_SESSION` in particular.
- `breakpoint_update -s disabled` as the capture cap.
- The size of the MCP result after conversion to JSON.
- The cause of the 17 to 25% slowdown when two requests run at once.

## Spec changes

ADRs:

- Write an ADR for logpoints over DBGp: a line breakpoint whose reads are pipelined with the `run` in one write; expressions limited to the path grammar of criterion 6 while eval is off; a cap of K captures done by removing the breakpoint at the K-th pause, with `>=`, `==` and `%` hit conditions used for "from hit K", "hit K only" and sampling; the per-value limits of criterion 7; a capture buffer with a fixed cap.
- [ADR-0008](../../decisions/0008-resource-budgets.md) stays as is. Its rule on payload limits at the source holds, with the note that DBGp limits bound a read only at `max_depth` 1.

[OPEN.md](../../OPEN.md):

- O-05: logpoint expressions that match the path grammar need no eval. Superglobal entries (context 1) and constants (context 2) need none either. Calls, operators, interpolation and class constants need eval. The daemon has to validate the grammar itself, because Xdebug ignores text after `]` without an error.
- O-08, numbers for the [budget table](../../../docs/performance.md#budgets):
  - Logpoint, one hit with 5 expressions on the same host: measured 64 to 77 us one command at a time, 41 to 45 us pipelined. The 5 ms target can drop to 0.2 ms, which leaves room for a slower machine.
  - A new row for daemon CPU per logpoint hit: measured about 40 us with one command at a time and 24 us pipelined.
  - MCP default tool result: 32 KB stays. With `max_depth` 1, `max_children` 32 and `max_data` 512 a single DBGp read fits under it.
- Add a question: the cap of the logpoint capture buffer, in captures or in bytes. With the proposed per-value limits a capture of 5 values can reach about 18.5 KB, so 1000 captures can reach about 18.5 MB, while with 1000 small captures RSS grew by about 0.5 MB over its start value.
- Add a question: whether snapshot points (every local of the frame "to a fixed depth") need a cycle check in the daemon. Xdebug has none, and a parent link doubles the size with each level.

[docs/features.md](../../../docs/features.md):

| Row | Result |
|---|---|
| Logpoint: a message template with expressions, captured without pausing | The worker does pause on every hit, for 41 to 77 us with 5 expressions, and continues at once. The row can say "pauses briefly and continues". Templates of path expressions need no eval. |
| Snapshot point: capture every local of the frame to a fixed depth, then continue | The depth has to stay small: `$this` of the fixture graph is 26.8 KB at depth 6 and 13.8 MB at depth 20, because parent links are followed. |
| Variables per frame: locals, superglobals and user constants | Context 2 also answers built-in constants by name (`PHP_VERSION`, `E_ALL`), although `context_get -c 2` lists only user constants. Superglobal entries are read only with `-c 1`. |
| `$this`, class statics and class constants, as far as Xdebug exposes them | Private and protected properties are readable. A parent's private property is named `$this->*Base*baseSecret`. Statics read only through `$this::name`. Class constants only through eval, as S1 found. |
| Watches: expressions read at every pause | Same rule as logpoints: path expressions without eval, anything else under O-05. |
| Copy a value as a PHP path expression (`$order->items[3]->price`), JSON or `var_export` | Xdebug's `fullname` is such a path and `property_get` reads it back, except the `->*Class*name` form, which is not valid PHP. |
