# S6 Coexistence with PhpStorm: report

Spike code: [`spikes/s6-ide-proxy/`](../../../spikes/s6-ide-proxy/). The raw output of every run is in its `results/` folder.

Short answer: yes against an IDE emulator, and not yet checked with PhpStorm itself. PhpStorm was not running during the spike, so a program that sends an approximation of PhpStorm's commands stood in for it. Through the proxy, the emulator's transcript of a session matches the transcript of the same session without the proxy, byte for byte after the run-specific values are blanked. While the emulator holds a pause, an agent on the proxy's control port reads the stack and four variables, and the emulator still reads correct values and continues. The proxy keeps transaction IDs apart by giving every command its own ID and putting the original back in the response. It adds 11 to 33 us per round trip at the median and at most 163 us at p99, against a budget of 1 ms. Criteria 1 and 2 are verified against the emulator, pending a run with PhpStorm. The steps for that run are under [Pending: run with PhpStorm](#pending-run-with-phpstorm).

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

Only PHP 8.5 was used. The test beds of other spikes (`testbed-s3` and the S4 UI container) ran on the same machine at the same time, so their work may have competed for CPU and added noise to the timings. The figures below are medians and p99 over thousands of round trips, repeated three times.

## How to rerun

```sh
cd spikes/testbed
export COMPOSE_PROJECT_NAME=testbed-s6 HTTP_PORT_85=8680 HTTP_PORT_83=8681
docker compose -f compose.yaml -f ../s6-ide-proxy/compose.override.yaml up -d --build
cd ../s6-ide-proxy
./build.sh     # unit tests and release build in the toolchain container
./t29.sh       # criteria 1 and 2: transcripts with and without the proxy
./bench.sh     # criterion 4: ROUNDS=2000 REPS=3 by default
```

The scripts run on the host and need only `docker` and `curl`. `compose.override.yaml` adds an `ide` service (the toolchain image again, so the emulator can sit across the Compose bridge from the proxy, as PhpStorm on the host would) and `host.docker.internal` for the toolchain container. No file of the shared test bed changed.

There are three binaries:

- `s6-ide-proxy` listens on `0.0.0.0:9003` for Xdebug, opens one connection to the IDE per session (`--ide HOST:PORT`) and takes agent commands on `127.0.0.1:9100`. At the end of each session it prints how long it held each packet it relayed, from the end of the read to the end of the write.
- `ide_emu` is the IDE emulator. It listens on 9010 (or on 9003 for a direct run), maps the breakpoint path with its own mapping, checks `$i`, `$sum` and `$item` at each pause against what `loop()` computes, and writes a transcript of every command and packet.
- `agent` connects to the control port, waits for each pause and injects reads.

`run.sh MODE NAME` runs one scenario. In `direct` mode Xdebug connects to the emulator on `toolchain:9003`. In `loop` mode the proxy is on `toolchain:9003` and the emulator on `127.0.0.1:9010` in the same container. In `bridge` mode the emulator is in the `ide` container, so every packet crosses the bridge twice, as with PhpStorm on the host. Every run sends one untriggered request first, so OPcache is warm and Xdebug reports the breakpoint the same way each time. `COLD=1` restarts PHP-FPM and skips that request.

The request is `GET /loop?n=3` on the fixture, with a breakpoint on `public/index.php:44` (`$sum += $item['square'];` inside `loop()`). It returns `200 {"iterations":3,"sum":5}` in every run below.

### What the emulator sends

The sequence approximates what PhpStorm sends. It comes from public Xdebug logs of PhpStorm sessions, not from a capture on this machine, and it has to be checked against a real Xdebug log from PhpStorm (see [Pending](#pending-run-with-phpstorm)).

1. After `init`: four `eval` commands that read `PHP_IDE_CONFIG`, `SERVER_NAME`, `SERVER_PORT` and `REQUEST_URI` from `$_SERVER`, which PhpStorm uses to find its server configuration.
2. `feature_set` for `show_hidden 1`, `max_depth 1`, `max_children 100`, `max_data 1024`, `extended_properties 1`, `notify_ok 1`, `resolved_breakpoints 1` and `breakpoint_details 1`.
3. `stdout -c 1`.
4. `breakpoint_set -t line` on the remote path, mapped from `/work/spikes/testbed/app/public/index.php` with the emulator's own mapping `/work/spikes/testbed/app=/var/www/html`.
5. `run`, then at each pause `stack_get`, `context_names -d 0`, `context_get -d 0 -c 0` and `property_get -d 0 -c 0 -n $item`.
6. `step_over` after the first pause, `run` after the others, and `stop` when Xdebug reports `stopping`.

That is 36 commands and 44 packets back (36 responses, the `init`, 7 `stream` packets and no `notify` when OPcache is warm). The session pauses 4 times: line 44 with `$i` 0, line 42 after the step, then line 44 with `$i` 1 and 2.

## Criterion 1: a session through the proxy works as without it

Against the emulator: met. Pending a run with PhpStorm.

`t29.sh` runs the same session several ways and compares each transcript with `cmp`. The transcript blanks only the values that change between two runs of the same request: `appid` and `xdebug:ctrl_socket` in `init`, `address` attributes, and breakpoint IDs (Xdebug numbers them per FPM worker, so two direct runs differ there too). Results from `results/t29-diff.txt`:

| Run | Compared with | Result |
|---|---|---|
| `t29-direct-b`, direct again | `t29-direct-a`, direct | identical, 81 lines |
| `t29-loop`, proxy in the same container | `t29-direct-a` | identical, 81 lines |
| `t29-bridge`, proxy across the bridge | `t29-direct-a` | identical, 81 lines |
| `t29-cold-bridge`, cold OPcache through the proxy | `t29-cold-direct`, cold OPcache direct | identical, 82 lines, 1 `notify` |
| `t30-inject`, proxy with the agent reading (criterion 2) | `t29-direct-hold`, direct with the same 200 ms hold | identical, 97 lines |
| `t30-flood`, proxy with the agent flooding reads (criterion 2) | `t29-direct-a` | identical, 81 lines |

In every run the emulator reports 4 pauses, 3 value checks ok out of 3, 0 transaction ID mismatches and 7 stream packets. The path mapping is the emulator's own: the proxy never reads or rewrites file URIs, so the IDE's mapping applies unchanged. With a cold OPcache, Xdebug sends a `breakpoint_resolved` notify and answers `breakpoint_set` with `resolved="resolved"`; with a warm one it answers `resolved="unresolved"` and still stops on the line. Both reach the emulator unchanged.

Two failure cases, also in `t29.sh`:

- `t30-ide-drop`: the emulator closes the connection at the first pause without `stop` or `detach`. The proxy sends `detach` to Xdebug and the request finishes normally (`200 {"iterations":3,"sum":5}`).
- `t30-no-ide`: nothing listens on the IDE port. The proxy logs `IDE at ide:9011 unreachable (Connection refused)`, closes the Xdebug socket and the request runs without the debugger (`200`, 3.1 ms).

## Criterion 2: reads for an agent while the IDE holds the pause

Against the emulator: met. Pending a run with PhpStorm.

The agent sends `x_wait`, which blocks until the session pauses. At each pause it reads `stack_get`, `property_get` of `$i`, `$sum` and `$item`, and `property_get -c 1 -n $_SERVER['REQUEST_URI'] -m 5`. The last read has its own `max_data` of 5 bytes and returns `/loop`. The IDE's later reads still come back with its own limit of 1024, as the identical transcripts show.

`t30-inject` (`--hold-ms 200`, `--pauses 4 --unsafe`): the emulator holds each pause 200 ms, then reads the stack and `$sum` again and checks that nothing changed. The agent's 20 reads all happen during the holds.

```
pause 1 at file:///var/www/html/public/index.php:44: i=Some(0) sum=Some(0) square=Some(0) REQUEST_URI(-m 5)=Some("/loop") check=ok
  unsafe commands refused: 18, not refused: 0
pause 2 at file:///var/www/html/public/index.php:42: ... check=n/a (not the breakpoint line)
pause 3 at file:///var/www/html/public/index.php:44: i=Some(1) sum=Some(0) square=Some(1) REQUEST_URI(-m 5)=Some("/loop") check=ok
pause 4 at file:///var/www/html/public/index.php:44: i=Some(2) sum=Some(1) square=Some(4) REQUEST_URI(-m 5)=Some("/loop") check=ok
agent: checks ok=3 bad=0, tid mismatches=0, unsafe refused=18 not refused=0, flood reads=0
```

The emulator's checks pass at every pause, and its transcript is identical to the direct run with the same hold.

`t30-flood` (no hold, `--pauses 4 --flood`): the agent reads `$i` in a loop from the moment the pause starts until the proxy refuses a read, so its reads race the emulator's commands on the same Xdebug socket. The agent's checks pass, and reads that arrive after the emulator has continued are refused with 901 (not paused) or 903 (paused again since `x_wait`), as designed. The transcript is identical to the direct run. In `bench.sh` the same flood runs while the emulator sends 6000 commands in one pause: the agent made 5525 to 5785 reads in that pause and every one returned the expected `$i`.

The proxy packet log of one injected run (`results/t30-inject-packets.log`, milliseconds since start) shows the two streams of commands interleaved on the Xdebug socket, each with its own IDs:

```
159.213 s1 ide>xd   stack_get -i 16
159.310 s1 agent>xd stack_get -i 2
159.362 s1 xd>ide   ... command="stack_get" transaction_id="16" ...
159.386 s1 xd>agent ... command="stack_get" transaction_id="17" ...
159.503 s1 ide>xd   context_names -i 17 -d 0
159.568 s1 xd>ide   ... command="context_names" transaction_id="18" ...
159.594 s1 agent>xd property_get -i 3 -n $i
159.634 s1 ide>xd   context_get -i 18 -d 0 -c 0
159.662 s1 xd>agent ... command="property_get" transaction_id="19" ...
159.746 s1 xd>ide   ... command="context_get" transaction_id="20" ...
```

The log shows the original IDs on the command lines and the proxy's IDs on Xdebug's responses before they are restored. Each agent read's response reached the proxy 65 to 95 us after the proxy sent the command. The agent itself measured 103 to 916 us per read (median 477 us) in this run and 104 to 431 us (median 188 us) in the flood run. That figure includes the agent's own process and the control-port hop. It does not count against the IDE budget, and its spread was not investigated.

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

`bench.sh` makes the emulator send 2000 rounds of `stack_get`, `context_get -d 0 -c 0` and `property_get -n $item` at the first pause (6000 round trips) and time each one from the write of the command to the end of the response. It runs direct, through the proxy in the same container (`loop`), through the proxy across the bridge (`bridge`), and across the bridge with the agent flooding reads in the same pause (`bridge-flood`), three times each. Round trip in microseconds as the emulator sees it, from `results/bench-summary.txt`:

| Mode | Median | p99 | Max |
|---|---|---|---|
| direct | 13.6, 13.8, 15.8 | 58.3, 59.3, 72.4 | 249 to 370 |
| loop | 38.3, 44.3, 47.7 | 135.4, 182.4, 195.9 | 558 to 610 |
| bridge | 25.6, 34.6, 46.7 | 154.8, 169.7, 226.4 | 488 to 608 |
| bridge-flood | 31.9, 34.7, 36.0 | 74.5, 112.7, 163.1 | 291 to 477 |

Compared with the mean of the three direct runs (14.4 us median, 63.3 us p99), the proxy adds 11 to 33 us at the median and 11 to 163 us at p99. The slowest single round trip through the proxy took 610 us. The proxy's own time per packet, from the end of the read to the end of the write, was 3.7 to 7.1 us at the median and 14.9 to 23.8 us at p99 in each direction, with a maximum of 186 us. The rest of the added time is presumably the extra hop: one more pair of socket reads and writes and two thread wake-ups per round trip. The agent's flood did not slow the emulator down in this test: the `bridge-flood` medians fall inside the `bridge` range.

Direct round trips take 14 to 16 us at the median, so the proxy roughly doubles or triples a round trip in relative terms. In absolute terms an IDE that sends 20 commands per pause loses well under 1 ms per pause.

## Pending: run with PhpStorm

Criteria 1 and 2 are verified against the emulator, pending a run with PhpStorm. The owner runs this later, by hand. `phpstorm-run.sh` does the container side and keeps the logs.

Setup:

1. Start the test bed and build, as in [How to rerun](#how-to-rerun).
2. In PhpStorm, open the repository root as the project.
3. Settings > PHP > Debug: set the Xdebug debug port to 9010 only (not 9003, which the proxy uses in the container) and allow external connections. Turn on "Start Listening for PHP Debug Connections".
4. Make sure the host accepts TCP connections on 9010 from the Docker networks (the session peer addresses in `results/` are in 192.168.0.0/24 on this machine). Run `./phpstorm-run.sh check`. It prints `ok: toolchain reaches host.docker.internal:9010` when the container reaches PhpStorm.
5. Settings > PHP > Servers: the fixture sets no `PHP_IDE_CONFIG` and its `SERVER_NAME` is empty (`SERVER_PORT` is 80), so PhpStorm will probably show its "Incoming connection from Xdebug" dialog on the first run. Accept it, then in the server it creates map `<repo>/spikes/testbed/app` to `/var/www/html`. Write down the server name and host PhpStorm chose.
6. Put a line breakpoint on `spikes/testbed/app/public/index.php` line 44 (`$sum += $item['square'];`).

Run 1, criterion 1: `./phpstorm-run.sh proxy`. The script fires `GET /loop?n=3` with the trigger cookie through the proxy. Expected in PhpStorm:

- It stops on line 44 of the local file, with `$i` = 0, `$sum` = 0 and `$item` = `['index' => 0, 'square' => 0]` in Variables.
- Step Over goes to line 42. Resume stops on line 44 with `$i` = 1, `$sum` = 0, `square` = 1. Resume again: `$i` = 2, `$sum` = 1, `square` = 4. Resume again ends the request.
- The script prints `200 {"iterations":3,"sum":5}` and exit status 0. PhpStorm shows no error or warning about the connection.

Run 2, criterion 2: `./phpstorm-run.sh proxy-agent`, with the same steps. Wait about a second at each pause before stepping, so the agent's reads land inside the pause. Expected: PhpStorm shows the same values as in run 1 and steps the same way. The `## agent` section of `results/phpstorm-proxy-agent.txt` shows `check=ok` at the three pauses on line 44, `unsafe refused=18 not refused=0` and `tid mismatches=0`.

Logs to keep, for each run: `results/phpstorm-MODE.txt` (proxy, agent and request output), `results/phpstorm-MODE-proxy.log` (every packet the proxy relayed, cut at 2 KB each) and `results/phpstorm-MODE-xdebug.log` (Xdebug's own log at level 7, emptied by the script before the request). The Xdebug log is the first capture of PhpStorm's real command sequence. Compare it with the emulator's in `results/t29-direct-a.transcript`; if PhpStorm sends commands the emulator does not, add them to `ide_emu` and rerun `t29.sh` and `bench.sh`.

Then record in this report whether each expectation held, the server name PhpStorm chose, and whether PhpStorm sent anything the proxy refused or did not expect (a response with an unknown transaction ID would show in the proxy log).

## Not verified

- PhpStorm itself: its command sequence, its path mapping, its pause button (`break` while running) and its handling of the empty `SERVER_NAME`.
- PhpStorm on the host reached from the container. Only the Compose bridge was crossed (`bridge` mode); the host gateway and the host firewall were not.
- Two requests debugged at the same time. The proxy relays every session it accepts, but the control port follows only the latest one, and the agent cannot pick a session.
- The proxy's memory and CPU. Its buffers are capped (64 MB per packet, 1 MB per command, 1024 pending responses, 2 KB per logged packet, 2^20 timing samples), but RSS was not measured.
- PHP 8.3 and Xdebug versions other than 3.5.3.
- Xdebug's control socket (`xdebug:ctrl_socket` in `init`) as a way to pause a running request while the IDE drives.

## Spec changes

ADRs:

- Write an ADR for proxy mode: the daemon listens on 9003 and the IDE on another port; the daemon owns the transaction IDs on the Xdebug socket and restores the IDE's; `init`, `notify` and `stream` packets go to the IDE unchanged; agent reads are allowed only from the allowlist of criterion 3, only while the IDE holds a pause, and only for the pause the agent waited for; the daemon sends `detach` when the IDE closes mid-session and drops the session when the IDE cannot be reached.
- [ADR-0008](../../decisions/0008-resource-budgets.md) stays as is.

[OPEN.md](../../OPEN.md):

- O-08, the IDE proxy row of the [budget table](../../../docs/performance.md#budgets): measured 11 to 33 us added at the median and up to 163 us at p99 per round trip, with 610 us as the slowest single round trip. The 1 ms target can become "at most 0.5 ms added per round trip at p99", which leaves room for a slower machine.
- Add a question: how the daemon finds the IDE (a configured host and port, and which default), and what it does when the IDE is not listening. The spike drops the session; prior art's `auto` mode would have the daemon drive the session itself instead.
- Add a question: whether an agent may set its own breakpoints or logpoints while the IDE drives. The spike refuses them, because the IDE would get pauses for breakpoints it does not know. Allowing them means the daemon has to hide those pauses and notifications from the IDE.
- Add a question: whether the agent should see the `stream` packets (stdout) of an IDE-driven session, which the daemon could copy.
- Add a question: which session the agent reads when several requests are debugged at once through the IDE.
- Add an item: run S6 criteria 1 and 2 with PhpStorm, following the steps in this report, before the proxy mode ADR is accepted.

[docs/features.md](../../../docs/features.md):

| Row | Result |
|---|---|
| Proxy mode: PhpStorm keeps driving the session while debug-claw reads it | Works against an emulator of PhpStorm; a run with PhpStorm is pending. The agent can read the stack and variables only while PhpStorm holds a pause, and cannot step, set breakpoints, change features or run code. The row can say "read only, during PhpStorm's pauses". |
