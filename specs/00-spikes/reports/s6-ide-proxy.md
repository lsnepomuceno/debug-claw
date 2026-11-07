# S6 Coexistence with PhpStorm: report

Spike code: [`spikes/s6-ide-proxy/`](../../../spikes/s6-ide-proxy/). The raw output of every run is in its `results/` folder.

Short answer: yes, with PhpStorm 2026.3. A session relayed through the proxy behaves in PhpStorm as it should: PhpStorm finds its server configuration, maps the file to the local path, stops on the breakpoint, shows the expected values, steps and finishes. While PhpStorm holds each pause, an agent on the proxy's control port reads the stack and four variables, PhpStorm still shows the same values, and the request ends with the same response. The proxy keeps transaction IDs apart by giving every command its own ID and putting the original back in the response. It adds 12 to 38 us per round trip at the median and at most 173 us at p99, against a budget of 1 ms.

The spike first ran against an IDE emulator, because PhpStorm was not open. The run with PhpStorm captured its real command sequence, which differed from the emulator's in ways that matter to the daemon (a pause PhpStorm makes for itself, its own `eval` calls, `detach` at the end). The emulator now replays the captured sequence, and every emulator result below comes from that version.

## Machine and versions

All runs on 2026-11-07, on one laptop.

| Item | Value |
|---|---|
| CPU | 12th Gen Intel Core i7-1260P, 12 cores (4 P and 8 E), 16 threads, governor `powersave` |
| Memory | 31 GiB |
| Kernel | Linux 7.0.0-38-generic |
| Docker | 29.9.2 |
| PHP | 8.5.12 FPM (`php:8.5-fpm-trixie`), NTS, OPcache with default settings |
| Xdebug | 3.5.3, `mode=debug`, `start_with_request=trigger`, `client_host=toolchain`, `client_port=9003`, log level 7 |
| nginx | 1.30-alpine |
| Rust | cargo 1.99.1 in the `toolchain` image, no dependencies |
| PhpStorm | 2026.3 (build 263.7307.215), on the host, listening on its default Xdebug port 9003 |

Only PHP 8.5 was used. The emulator runs and the benchmark below were repeated at 23:19 UTC with no other spike running and PhpStorm open but idle. The first emulator runs, done while other spikes ran on the same machine, gave figures in the same range. The figures are medians and p99 over thousands of round trips, repeated three times.

## How to rerun

```sh
cd spikes/testbed
export COMPOSE_PROJECT_NAME=testbed-s6 HTTP_PORT_85=8680 HTTP_PORT_83=8681
docker compose -f compose.yaml -f ../s6-ide-proxy/compose.override.yaml up -d --build
cd ../s6-ide-proxy
./build.sh     # unit tests and release build in the toolchain container
./t29.sh       # criteria 1 and 2: transcripts with and without the proxy
./bench.sh     # criterion 4: ROUNDS=2000 REPS=3 by default
IDE_PORT=9003 ./phpstorm-run.sh proxy        # criterion 1 with PhpStorm
IDE_PORT=9003 ./phpstorm-run.sh proxy-agent  # criterion 2 with PhpStorm
```

The scripts run on the host and need only `docker` and `curl`. `compose.override.yaml` adds an `ide` service (the toolchain image again, so the emulator can sit across the Compose bridge from the proxy, as PhpStorm on the host would), `host.docker.internal` for the toolchain container, and `nginx-phpstorm.conf` for nginx. That file is the test bed's nginx config plus `fastcgi_param PHP_IDE_CONFIG "serverName=debug-claw-testbed"` and `fastcgi_read_timeout 600s` on port 80. No file of the shared test bed changed.

There are three binaries:

- `s6-ide-proxy` listens on `0.0.0.0:9003` for Xdebug, opens one connection to the IDE per session (`--ide HOST:PORT`) and takes agent commands on `127.0.0.1:9100`. At the end of each session it prints how long it held each packet it relayed, from the end of the read to the end of the write.
- `ide_emu` is the IDE emulator. It listens on 9010 (or on 9003 for a direct run), maps the breakpoint path with its own mapping, checks `$i`, `$sum` and `$item` at each pause against what `loop()` computes, and writes a transcript of every command and packet.
- `agent` connects to the control port, waits for each pause and injects reads.

`run.sh MODE NAME` runs one scenario. In `direct` mode Xdebug connects to the emulator on `toolchain:9003`. In `loop` mode the proxy is on `toolchain:9003` and the emulator on `127.0.0.1:9010` in the same container. In `bridge` mode the emulator is in the `ide` container, so every packet crosses the bridge twice, as with PhpStorm on the host. Every run sends one untriggered request first, so OPcache is warm and Xdebug reports the breakpoint the same way each time. `COLD=1` restarts PHP-FPM and skips that request.

The request is `GET /loop?n=3` on the fixture, with a breakpoint on `public/index.php:44` (`$sum += $item['square'];` inside `loop()`). It returns `200 {"iterations":3,"sum":5}` in every run below.

### What PhpStorm sends, and the emulator with it

The proxy log of the PhpStorm runs (`results/phpstorm-proxy-agent-proxy.log`) holds PhpStorm 2026.3's real command sequence for this session, 42 commands:

1. `eval` of `(string)(ini_get('xdebug.coverage_enable').';'.ini_get('xdebug.profiler_enable').';'...)`, which reads Xdebug 2 settings.
2. `feature_set` for `show_hidden 1`, `max_depth 1`, `max_children 100`, `extended_properties 1`, `notify_ok 1`, `resolved_breakpoints 1` and `breakpoint_include_return_value 1`. No `max_data`.
3. `stdout -c 1`, `status`, then `step_into`. The script pauses on its first line (`public/index.php:5`).
4. During that pause, `eval getenv('PHP_IDE_CONFIG')!=false` and `eval (string)(getenv('PHP_IDE_CONFIG'))`, which is how PhpStorm finds its server configuration. Then `breakpoint_set -t line` on the mapped remote path, `stack_get` and `run`.
5. At each pause on the breakpoint: `stack_get`, `context_names` (first pause only), `context_get -d 0 -c 0` twice, `context_get -d 0 -c 1` and `context_get -d 0 -c 2`. No `property_get`.
6. `step_over` after the first pause, `run` after the others, and `detach` when Xdebug reports `stopping`.

The first emulator, written from public logs before the capture, sent four `eval` reads of `$_SERVER`, `max_data 1024` and `breakpoint_details`, no `step_into`, a `property_get` per pause, and `stop` at the end. `ide_emu` now sends the captured sequence. Its transcript blanks the values that change between two runs of the same request, which now include `REMOTE_PORT`, `REQUEST_TIME` and `REQUEST_TIME_FLOAT`, since PhpStorm reads the superglobals at every pause.

The session pauses 5 times: the first-line pause from `step_into`, line 44 with `$i` 0, line 42 after the step, then line 44 with `$i` 1 and 2.

## Criterion 1: a session through the proxy works as without it

Met, with PhpStorm and with the emulator.

### With PhpStorm

PhpStorm 2026.3 opened the repository with a server `debug-claw-testbed` (`localhost:8680`) mapping `spikes/testbed/app` to `/var/www/html`, and a line breakpoint on `public/index.php:44`. `IDE_PORT=9003 ./phpstorm-run.sh proxy` fired `GET /loop?n=3` through the proxy, which relayed the session to `host.docker.internal:9003`. Step Over and Resume were sent to PhpStorm's window as F8 and F9, and the Variables pane was captured at each pause (`results/phpstorm-proxy-vars.png`).

| Pause | Line | Variables in PhpStorm |
|---|---|---|
| 1 | 44 | `$i` 0, `$item` [0, 0], `$sum` 0 |
| after Step Over | 42 | `$i` 1, `$item` [0, 0], `$sum` 0 |
| 2 | 44 | `$i` 1, `$item` [1, 1], `$sum` 0 |
| 3 | 44 | `$i` 2, `$item` [2, 4], `$sum` 1 |

The last Resume ended the request with `200 {"iterations":3,"sum":5}`. PhpStorm matched the session to its server through `PHP_IDE_CONFIG` with no "Incoming connection" dialog, opened the local file and showed no warning. The proxy relayed 42 commands and 50 packets and found no response with an unknown transaction ID. The first attempt ended with HTTP 504 after 60 s, because the pause outlived nginx's default `fastcgi_read_timeout`; PHP kept the session, and the proxy and PhpStorm behaved the same. The S6 nginx config raises the timeout to 600 s.

### With the emulator

`t29.sh` runs the same session several ways and compares each transcript with `cmp`. Results from `results/t29-diff.txt`:

| Run | Compared with | Result |
|---|---|---|
| `t29-direct-b`, direct again | `t29-direct-a`, direct | identical, 93 lines |
| `t29-loop`, proxy in the same container | `t29-direct-a` | identical, 93 lines |
| `t29-bridge`, proxy across the bridge | `t29-direct-a` | identical, 93 lines |
| `t29-cold-bridge`, cold OPcache through the proxy | `t29-cold-direct`, cold OPcache direct | identical, 94 lines, 1 `notify` |
| `t30-inject`, proxy with the agent reading (criterion 2) | `t29-direct-hold`, direct with the same 200 ms hold | identical, 109 lines |
| `t30-flood`, proxy with the agent flooding reads (criterion 2) | `t29-direct-a` | identical, 93 lines |

In every run the emulator reports 4 pauses on its breakpoint, 3 value checks ok out of 3, 0 transaction ID mismatches and 7 stream packets. The path mapping is the IDE's own: the proxy never reads or rewrites file URIs. With a cold OPcache, Xdebug sends a `breakpoint_resolved` notify, which reaches the IDE unchanged.

Two failure cases, also in `t29.sh`:

- `t30-ide-drop`: the emulator closes the connection at the first pause without `stop` or `detach`. The proxy sends `detach` to Xdebug and the request finishes normally (`200 {"iterations":3,"sum":5}`).
- `t30-no-ide`: nothing listens on the IDE port. The proxy closes the Xdebug socket and the request runs without the debugger (`200`, 2.9 ms).

## Criterion 2: reads for an agent while the IDE holds the pause

Met, with PhpStorm and with the emulator.

The agent sends `x_wait`, which blocks until the session pauses. At each pause it reads `stack_get`, `property_get` of `$i`, `$sum` and `$item`, and `property_get -c 1 -n $_SERVER['REQUEST_URI'] -m 5`. The last read has its own `max_data` of 5 bytes and returns `/loop`.

### With PhpStorm

`IDE_PORT=9003 PAUSES=5 ./phpstorm-run.sh proxy-agent`, stepping in PhpStorm as in criterion 1 and waiting about two seconds at each pause (`results/phpstorm-proxy-agent.txt`):

```
  property_get -n $i: error Some(300)
  property_get -n $sum: error Some(300)
  property_get -n $item: error Some(300)
pause 1 at file:///var/www/html/public/index.php:5: i=None sum=None square=None REQUEST_URI(-m 5)=Some("/loop") check=n/a (not the breakpoint line)
  unsafe commands refused: 18, not refused: 0
pause 2 at file:///var/www/html/public/index.php:44: i=Some(0) sum=Some(0) square=Some(0) REQUEST_URI(-m 5)=Some("/loop") check=ok
pause 3 at file:///var/www/html/public/index.php:42: i=Some(1) sum=Some(0) square=Some(0) REQUEST_URI(-m 5)=Some("/loop") check=n/a (not the breakpoint line)
pause 4 at file:///var/www/html/public/index.php:44: i=Some(1) sum=Some(0) square=Some(1) REQUEST_URI(-m 5)=Some("/loop") check=ok
pause 5 at file:///var/www/html/public/index.php:44: i=Some(2) sum=Some(1) square=Some(4) REQUEST_URI(-m 5)=Some("/loop") check=ok
agent: checks ok=3 bad=0, tid mismatches=0, unsafe refused=18 not refused=0, flood reads=0
agent injected reads: n=22 us median=172.6 p99=378.1 min=87.6 max=378.1
## request
200 12.073478s {"iterations":3,"sum":5}
```

PhpStorm showed the same values as in criterion 1 at every pause (`results/phpstorm-proxy-agent-vars.png`) and stepped the same way. The first pause is the one PhpStorm makes for itself with `step_into` before it sets its breakpoint. The user never sees it, but the agent's `x_wait` does, and the loop's variables do not exist there yet, so error 300 is the right answer. The agent now accepts error 300 at pauses outside line 44 and counts it as a failure on line 44. A first run with the earlier agent counted those three errors as failures and exited with status 1; PhpStorm's view was the same in both runs.

### With the emulator

`t30-inject` (`--hold-ms 200`, `--pauses 4 --unsafe`): the emulator holds each pause 200 ms, then reads the stack and `$sum` again and checks that nothing changed. The agent's checks pass at both breakpoint pauses it sees (`ok=2 bad=0`), all 18 unsafe commands are refused, and the emulator's transcript is identical to the direct run with the same hold.

`t30-flood` (no hold, `--pauses 4 --flood`): at each pause on line 44 the agent reads `$i` in a loop until the proxy refuses a read, so its reads race the emulator's commands on the same Xdebug socket. The agent made 35 reads, all correct, and reads that arrived after the emulator continued were refused with 901 (not paused) or 903 (paused again since `x_wait`). The transcript is identical to the direct run. In `bench.sh` the same flood runs while the emulator sends 6000 commands in one pause: the agent made 5447 to 5890 reads in that pause and every one returned the expected `$i`.

The proxy packet log of an injected run (`results/t30-inject-packets.log`) shows the two streams of commands interleaved on the Xdebug socket, each with its own IDs, and Xdebug's responses carrying the proxy's IDs before they are restored.

## Criterion 3: transaction IDs, notifications and unsafe commands

### Transaction IDs

The proxy owns the transaction IDs on the Xdebug socket. Every command it writes to Xdebug, from the IDE, from the agent or its own, gets the next number of a per-session counter. A table maps that number to where the response goes: to the IDE with the IDE's original `-i` value, to the agent connection with the agent's `-i` value, or nowhere (the proxy's own `detach`). When a response arrives, the proxy looks up its `transaction_id`, removes the entry and writes the response with the original ID in place of its own. Only the attribute value changes: the packet is written as three slices (before the ID, the ID, after it) with a recomputed length, so the body is not copied. A command without `-i` gets one, and a response whose ID is not in the table goes to the IDE unchanged. The table holds at most 1024 entries.

The rewrite is part of the 4 to 7 us the proxy spends per packet, and it works whatever IDs the IDE uses. The alternatives were to give the agent a range of IDs the IDE never uses, which depends on how PhpStorm counts, or to read only when the IDE has no command in flight, which would make the agent wait on the IDE.

### Notifications, stream packets and init

`init`, `notify` (for example `breakpoint_resolved`) and `stream` packets carry no transaction ID. The proxy relays them to the IDE unchanged and in order, because the IDE asked for them (`notify_ok`, `stdout -c 1`) and expects them. The agent does not see them. The cold OPcache pair and the 7 stream packets in every transcript show they arrive as they do without the proxy.

### When the agent may read

The proxy tracks the session state from what the IDE sends and what Xdebug answers. A continuation command from the IDE (`run`, `step_into`, `step_over`, `step_out`, `stop`, `detach`) sets the state to running before it is written. The response to it sets `break`, `stopping` or `stopped` from its `status` attribute, and each `break` counts a new pause. An agent read is written only when the state is `break` and the pause number is still the one the agent's `x_wait` returned. The check and the write happen under the same lock the IDE's commands take, so a continuation from the IDE either goes first (and the read is refused) or after the read (and Xdebug answers the read while still paused). The refusals are DBGp error responses with codes outside Xdebug's range: 900 unsafe command, 901 not paused, 902 no session or session ended, 903 the session has moved to another pause.

### Unsafe commands

The control port allows only these commands, which read and change nothing: `stack_depth`, `stack_get`, `context_names`, `context_get`, `property_get`, `property_value`, `typemap_get`, `source`, `status`, `feature_get`, `breakpoint_get` and `breakpoint_list`. Per-command options such as `-m`, `-p`, `-d` and `-c` stay allowed, because they apply to one response only.

Everything else is refused with error 900. These are unsafe while the IDE drives:

| Commands | Why |
|---|---|
| `run`, `step_into`, `step_over`, `step_out`, `break` | They change execution state. The IDE would get a response it did not ask for, or find the script somewhere it did not step to. |
| `stop`, `detach` | They end the session under the IDE. |
| `feature_set` | Session-wide: `max_depth`, `max_children` or `max_data` set by the agent would change what the IDE's later reads return. |
| `breakpoint_set`, `breakpoint_update`, `breakpoint_remove` | The IDE's breakpoint list would no longer match Xdebug's, and the IDE would get pauses and `breakpoint_resolved` notifications for breakpoints it does not know. |
| `stdout`, `stderr` | They change the output redirection the IDE set up. |
| `property_set` | It changes program state the IDE is showing. |
| `eval`, `expr`, `exec`, `interact` | They run PHP code, which can change any state. |

The `--unsafe` run sends all 18 of these at the first pause and all 18 are refused. Commands not in either list, such as Xdebug's `xcmd_` extensions, are refused too, because the list is an allowlist.

## Criterion 4: latency the proxy adds per round trip

Budget ([docs/performance.md](../../../docs/performance.md#budgets)): at most 1 ms added per DBGp round trip. Met with a wide margin.

`bench.sh` makes the emulator send 2000 rounds of `stack_get`, `context_get -d 0 -c 0` and `property_get -n $item` at its first breakpoint pause (6000 round trips) and time each one from the write of the command to the end of the response. It runs direct, through the proxy in the same container (`loop`), through the proxy across the bridge (`bridge`), and across the bridge with the agent flooding reads in the same pause (`bridge-flood`), three times each. Round trip in microseconds as the emulator sees it, from `results/bench-summary.txt`:

| Mode | Median | p99 | Max |
|---|---|---|---|
| direct | 13.1, 11.7, 16.1 | 76.4, 26.5, 86.5 | 198 to 461 |
| loop | 25.1, 26.2, 43.3 | 177.5, 144.6, 188.8 | 536 to 704 |
| bridge | 39.6, 51.3, 43.6 | 123.4, 236.3, 150.9 | 465 to 702 |
| bridge-flood | 34.8, 41.3, 31.9 | 163.4, 189.5, 86.3 | 346 to 726 |

Compared with the mean of the three direct runs (13.6 us median, 63.1 us p99), the proxy adds 12 to 38 us at the median and 23 to 173 us at p99. The slowest single round trip through the proxy took 726 us. In the bridge runs, the proxy's own time per packet, from the end of the read to the end of the write, was 4.8 to 8.4 us at the median and 14.3 to 27.1 us at p99 in each direction, with a maximum of 178 us. The rest of the added time is presumably the extra hop: one more pair of socket reads and writes and two thread wake-ups per round trip. The agent's flood did not slow the emulator down: the `bridge-flood` medians fall inside the `bridge` range.

In the PhpStorm runs, with 42 commands per session and seconds between them, the proxy's own time per packet was 25 to 37 us at the median and at most 177 us, higher than in the benchmark, where packets follow each other and the threads stay awake. Either way an IDE that sends 20 commands per pause loses well under 1 ms per pause.

## Run with PhpStorm: how it was done

PhpStorm was not open when the spike started, and the owner was away. It was then run on the host with the project as follows, with nothing installed on the host:

1. The test bed and the S6 override were started and built as in [How to rerun](#how-to-rerun).
2. The repository's `.idea/` (ignored by git) was written by hand before PhpStorm opened it: `workspace.xml` with `PhpDebugGeneral listening_started="true"`, the `debug-claw-testbed` server with its path mapping, and the line breakpoint (stored 0-based as line 43). The repository folder was added to PhpStorm's trusted paths for the run, and that change was reverted afterwards.
3. PhpStorm was started from the JetBrains Toolbox script and listened on its default port 9003 on the host. `IDE_PORT=9003 ./phpstorm-run.sh check` printed `ok: toolchain reaches host.docker.internal:9003`.
4. A throwaway container with `xdotool` and ImageMagick, given the host's X11 socket, sent F8 and F9 to PhpStorm's window and captured that window. The captures kept in `results/` are crops of the debugger's Variables pane.
5. PhpStorm was closed with SIGTERM after the runs.

Logs: `results/phpstorm-MODE.txt` (proxy, agent and request output), `results/phpstorm-MODE-proxy.log` (every packet the proxy relayed, cut at 2 KB each) and `results/phpstorm-MODE-xdebug.log` (Xdebug's log at level 7), for `proxy` and `proxy-agent`. A person can repeat the run by opening the project in PhpStorm with the same server, mapping and breakpoint, and stepping by hand.

## Not verified

- PhpStorm's pause button while the script runs, which would need Xdebug's control socket from the host.
- Two requests debugged at the same time. The proxy relays every session it accepts, but the control port follows only the latest one, and the agent cannot pick a session.
- The proxy's memory and CPU. Its buffers are capped (64 MB per packet, 1 MB per command, 1024 pending responses, 2 KB per logged packet, 2^20 timing samples), but RSS was not measured.
- PHP 8.3, Xdebug versions other than 3.5.3, other PhpStorm versions and other IDEs (VS Code with PHP Debug).
- The host firewall rules beyond this machine, where the Docker bridge reached the host on 9003 without changes.

## Spec changes

ADRs:

- Accept ADR-0016 (proxy mode): criteria 1 and 2 passed with PhpStorm. Its consequences gain the captured facts: PhpStorm uses `eval` itself (three calls per session), makes a first-line pause with `step_into` before setting breakpoints, and ends with `detach`.
- [ADR-0008](../../decisions/0008-resource-budgets.md) stays as is.

[OPEN.md](../../OPEN.md):

- O-08, the IDE proxy row of the [budget table](../../../docs/performance.md#budgets): measured 12 to 38 us added at the median and up to 173 us at p99 per round trip, with 726 us as the slowest single round trip. The 1 ms target can become "at most 0.5 ms added per round trip at p99", which leaves room for a slower machine.
- O-18 gains a question: whether the agent's wait skips the pauses an IDE makes for itself, such as PhpStorm's first-line pause, which the user never sees.
- O-05: the eval policy covers the daemon and the agent. In proxy mode the IDE's own `eval` calls pass through unchanged, because PhpStorm needs them to find its server.
- O-21: a paused request behind nginx gets HTTP 504 after nginx's `fastcgi_read_timeout` (60 s by default) while PHP stays paused. The setup help should say how to raise it.

[docs/features.md](../../../docs/features.md):

| Row | Result |
|---|---|
| Proxy mode: PhpStorm keeps driving the session while debug-claw reads it | Works with PhpStorm 2026.3. The agent can read the stack and variables only while PhpStorm holds a pause, and cannot step, set breakpoints, change features or run code. |
