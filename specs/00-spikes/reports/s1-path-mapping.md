# S1 Path mapping without configuration: report

Spike code: [`spikes/s1-path-mapping/`](../../../spikes/s1-path-mapping/). Transcripts of every run are in its `results/` folder, and the sections below quote them.

## Versions

| Container | PHP | Xdebug | Entry script in `init` |
|---|---|---|---|
| `php85` (port 8180) | 8.5.12 NTS | 3.5.3 | `file:///var/www/html/public/index.php` |
| `php83` (port 8181) | 8.3.36 NTS | 3.5.3 | `file:///srv/app/index.php` |

Both images are Debian trixie. The listener is built with cargo 1.99.1 in the `toolchain` container, has no dependencies, and listens on `toolchain:9003`. The test bed's Xdebug settings were not changed (`mode=debug`, `start_with_request=trigger`). Both `init` packets are in `results/init-php85.txt` and `results/init-php83.txt`.

## How to rerun

From `spikes/s1-path-mapping/`, with the test bed up under the project name `testbed-s1`:

```sh
export COMPOSE_PROJECT_NAME=testbed-s1 HTTP_PORT_85=8180 HTTP_PORT_83=8181
docker compose -f ../testbed/compose.yaml -f compose.override.yaml up -d
docker compose -f ../testbed/compose.yaml -f compose.override.yaml exec -T toolchain \
    cargo build --manifest-path spikes/s1-path-mapping/Cargo.toml
docker compose -f ../testbed/compose.yaml -f compose.override.yaml exec -T toolchain \
    cargo test --manifest-path spikes/s1-path-mapping/Cargo.toml
./run.sh layouts.txt "map --local /work/spikes/testbed/app --break src/Graph.php:17 --break public/index.php:43 --sessions 4" \
    "curl -s -b XDEBUG_TRIGGER=1 'http://127.0.0.1:8181/graph?depth=2&width=1'" \
    "curl -s -b XDEBUG_TRIGGER=1 'http://127.0.0.1:8181/loop?n=2'" \
    "curl -s -b XDEBUG_TRIGGER=1 'http://127.0.0.1:8180/graph?depth=2&width=1'" \
    "curl -s -b XDEBUG_TRIGGER=1 'http://127.0.0.1:8180/loop?n=2'"
./ambiguous.sh   # criterion 2, writes results/case-*.txt
./probes.sh      # criterion 4, writes results/probe-*.txt
```

`compose.override.yaml` adds read-only mounts for the fixtures and the probe scripts. The shared test bed files are unchanged. `/work/spikes/testbed/app` inside `toolchain` stands in for the project on the host.

## How the listener infers the mapping

1. Read `fileuri` from `init` and turn it into a path. Xdebug builds the URI in `xdebug_path_to_url` (`src/lib/usefulstuff.c`): `file://` plus the path on POSIX, `file:///C:/...` for a drive, `file://server/share/...` for UNC, backslashes turned into `/`, and every byte except letters, digits, `-._:` and `/` percent encoded.
2. Walk the local project for files with the same base name. The walk does not follow symlinks, skips `.git`, `.hg`, `.svn`, `node_modules` and `.idea`, and stops at 200,000 entries.
3. Keep the candidate with the longest common suffix of path components. The remote prefix is what remains of the remote path, and the local root is what remains of the local one.
4. If two candidates tie, or if there is one, read the entry script through DBGp (`source -f URI`) and compare it with the local file. A tie that content resolves is accepted. A tie that content does not resolve becomes a question for the user.
5. For each breakpoint, translate the local path, check the remote file and line with `source -f URI -b L -e L`, then send `breakpoint_set`. Everything happens before the first `run`.

The check in step 5 is needed because Xdebug accepts `breakpoint_set` for a file that does not exist: it answers with an id and no error, and `resolved_breakpoints` is `0`, so the response carries no `resolved` attribute either.

## Criterion 1: a breakpoint on a host path pauses the request in two layouts

Passed in both layouts, on both PHP versions. `results/layouts.txt` holds four sessions, all `PASS`:

| Session | Layout | Inference | Breakpoint sent | Paused at |
|---|---|---|---|---|
| 1, 8181 `/graph` | Root (`/srv/app/index.php`) | Tie between `index.php` and `public/index.php` (1 component each), broken by content: `source` read 183 bytes in 5.0 ms | `file:///srv/app/src/Graph.php:17` | `file:///srv/app/src/Graph.php:17` |
| 2, 8181 `/loop` | Root | Reused, entry script checked by content, no walk | `file:///srv/app/public/index.php:43` | same |
| 3, 8180 `/graph` | `public/` (`/var/www/html/public/index.php`) | Unique, 2 components match | `file:///var/www/html/src/Graph.php:17` | same |
| 4, 8180 `/loop` | `public/` | Reused | `file:///var/www/html/public/index.php:43` | same |

The inferred mappings are `/srv/app -> /work/spikes/testbed/app` and `/var/www/html -> /work/spikes/testbed/app`. Each pause is translated back and printed as the local path, for example `shown to the user as /work/spikes/testbed/app/src/Graph.php:17`. The walk visited 10 entries in 94 µs and 104 µs. From `init` to the first `run`, with two breakpoints and their remote checks, took 6.3 ms, 8.1 ms, 10.0 ms and 7.6 ms.

The root layout is a tie on suffix alone, because both `index.php` and `public/index.php` end with `index.php`. Content decides it. A plain longest suffix rule would have picked either one.

## Criterion 2: where inference fails or is ambiguous, and what to ask

Linux cases, run by `ambiguous.sh` against the fixtures in `fixtures/`:

| Case | Transcript | What happened | What the daemon should do |
|---|---|---|---|
| Same relative path in two folders (`dup/api/public/index.php`, `dup/admin/public/index.php`), only `admin/` in the container at `/var/www/admin` | `case-dup.txt` | Unique: 3 components match `admin/public/index.php` against 2 for `api/`. Content matches. Paused. | Nothing to ask. |
| Two identical copies (`dup-identical/a` and `b`), `b` mounted at `/var/www/same` | `case-dup-identical.txt` | Tie on 2 components, both contents match. No mapping. | Ask: "2 local folders hold an identical copy of /var/www/same/public/index.php: .../a, .../b. Which one is the project?" Offer both, remember the answer. |
| Local tree holds an older or different copy (local root `dup/api`, container runs `admin`) | `case-stale.txt` | Unique by name, content differs. The mapping is still inferred and the breakpoint paused at line 4. | Warn, do not ask: "The container runs other code than the local tree (stale image or another project?)". Line numbers may not match. |
| `vendor/` mounted elsewhere (`/srv/vend` for the app, `/opt/vendor` for the vendor folder) | `case-vendor-missing.txt` | App mapping `/srv/vend -> fixtures/vendored`. The vendor breakpoint translates to `file:///srv/vend/vendor/acme/lib/src/Lib.php`, the remote check says `MISSING (can not open file)`, and the request ends without a pause. | Mark the breakpoint as not placed, and ask: "This file is not at /srv/vend/vendor/... in the container. Where is vendor/ mounted?" Or learn it, see the next row. |
| Same, learned from a frame | `case-vendor-learned.txt` | Break in the app, `step_into` enters `file:///opt/vendor/acme/lib/src/Lib.php`, which is outside every known prefix. A second inference on that frame gives `/opt -> fixtures/vendored` (5 components). The vendor breakpoint at `file:///opt/vendor/acme/lib/src/Lib.php:9` then pauses. | Learn a second mapping from any frame outside the known prefixes, and place pending breakpoints again. Ask only if that inference is ambiguous. |
| Symlinked folder (`current -> real`), script run as `/srv/sym/current/index.php` | `case-symlink-link.txt`, `case-symlink-real.txt` | `init` reports `file:///srv/sym/real/index.php`, and frames report `real/` even for a file required as `/srv/sym/current/other.php`. Breakpoints sent as `current/lib.php` and as `real/lib.php` both pause, reported at `real/lib.php:5`. | Nothing to ask. Resolve symlinks on the local side too, so a breakpoint set on `current/lib.php` is shown where Xdebug reports it. |
| Breakpoint with the wrong case on Linux (`src/graph.php` for `src/Graph.php`) | `case-wrong-case.txt` | Remote check `MISSING`, no pause. | Warn that the file does not exist in the container, with the closest name found locally. |

Two Xdebug behaviours explain the symlink row. `breakpoint_set` runs `VCWD_REALPATH` on the file before it stores the breakpoint (`handler_dbgp.c`, line 1151 in 3.5.3), and PHP reports the resolved path of every included file. The daemon's walk does not follow symlinks and lists the ones it sees (`walk: symlink current -> real (not followed)`), so it can map a path through a link to its target.

### Windows and macOS hosts

The test bed runs on Linux. These cases are checked with unit tests (`cargo test`, 19 pass) against `init` packets in `recorded/`. Two of them were recorded. The others are the PHP 8.5 recording with only `fileuri` changed to the value Xdebug 3.5.3 builds on that platform, as `recorded/README.md` explains.

| Setup | `fileuri` | Test | Result |
|---|---|---|---|
| Windows, PHP in a Linux container (Docker Desktop), project on `C:` | `file:///var/www/html/public/index.php` | `windows_host_with_docker_desktop` | Maps to `C:\Users\Dev\Projects\shop`. A breakpoint given as `c:/users/dev/projects/shop/src/Graph.php` (lower-case drive, forward slashes) becomes `file:///var/www/html/src/Graph.php`, and the frame maps back to `C:\Users\Dev\Projects\shop\src\Graph.php`. |
| Windows, native PHP | `file:///C:/laragon/www/My%20Shop/public/index.php` | `windows_native_php_drive_uri` | The URI decodes to `C:\laragon\www\My Shop\public\index.php`. With a local root written `c:\Laragon\www\My Shop`, a breakpoint on `C:\laragon\WWW\my shop\src\Graph.php` becomes `file:///C:/laragon/www/My%20Shop/src/Graph.php`: drive and names compare without case, and the remote spelling is kept. |
| Windows URI encoding | `file:///D:/dev/caf%C3%A9%7E1/a%2Bb/index.php` | `windows_uri_encoding_matches_xdebug` | Matches Xdebug's encoding of `D:\dev\café~1\a+b\index.php`. `file:///c:/...`, a backslash left in a URI and the `\\?\` prefix all parse. |
| Windows, code on a UNC share | `file://fileserver/www/shop/index.php` | `windows_unc_uri` | Parses to `\\fileserver\www\shop\index.php` and back to the same URI. |
| WSL 2, editor on Windows | `file:///home/dev/shop/public/index.php` | `wsl_project_opened_from_windows` | Local root `\\wsl.localhost\Ubuntu\home\dev\shop`, breakpoints become `file:///home/dev/shop/...`. |
| Another drive | `file:///C:/www/shop/index.php` | `windows_drive_change_does_not_match` | A local path on `D:` does not map. |
| macOS, Docker Desktop, case-insensitive APFS | `file:///var/www/html/Public/index.php` | `macos_docker_case_insensitive_tree` | The local file is `public/index.php`. With case folding on the local side the match is unique and the remote prefix is `/var/www/html`. |
| Same packet, case-sensitive tree | same | `case_sensitive_tree_does_not_fold_case` | Only `index.php` matches, and the local root moves into `public/`. The daemon needs to know whether the local file system folds case. |

One risk on macOS comes from reading the Xdebug source, not from a run. `breakpoint_set` resolves the path with `realpath`, which on Linux keeps the case it was given. The check that marks a function as having line breakpoints (`mark_fse_as_having_line_breakpoints` in `debugger.c`) compares file names with case-sensitive `zend_string_equals`, while the later per-line check uses `zend_string_equals_ci`. A breakpoint sent with a case that differs from the one PHP opened the file with may therefore never pause, even when the bind mount accepts both spellings. The daemon should send the name as stored on disk, taken from its walk, and compare reported paths without case when the local file system folds case. A macOS host is needed to confirm it.

## Criterion 3: learn once, reuse, and what invalidates it

The mapping can be learned on the first session and reused. Sessions 2 and 4 in `results/layouts.txt` reuse it with no walk: the entry script is under a known remote prefix, and one `source` round trip confirms that its content matches the local file. The unit test `learned_mapping_is_reused_and_forgotten` covers the same logic.

What invalidates a learned mapping:

- The entry script content differs from the local file (`case-stale.txt`). The usual causes are an image built from an older commit, another branch checked out locally, or another project served from the same remote prefix. `/var/www/html` is the default for many images, so the remote prefix alone does not identify a project. The daemon forgets the mapping, infers again and, if the content still differs, shows the warning from criterion 2.
- A frame outside every known prefix (`case-vendor-learned.txt`). This adds a mapping and keeps the existing one.
- The remote check of a breakpoint fails (`MISSING`), which points to a changed mount.
- The local root is moved or deleted.

The check costs one `source` round trip per session. The first read of the entry script took 5.0 ms, against about 120 µs for a `feature_get`. The walk is needed only when there is no learned mapping.

## Criterion 4: DBGp features

Both PHP versions answered `feature_get` the same way, except for `language_version` (`results/probe-features-php85.txt` and `probe-features-php83.txt`). The other probes ran on PHP 8.5.

| Feature | Supported | How it was verified |
|---|---|---|
| `context_names` | Yes | `Locals` (0), `Superglobals` (1), `User defined constants` (2). `context_get -c 2` returns `APP_MODE` and `APP_DEBUG` with `facet="constant"` (`probe-statics.txt`). |
| Class statics in property trees | Yes | `$this` lists `currency`, `registry` and `instances` with `facet="static public"`, `"static private"` and `"static protected"`, including the parent's static. `property_get -n $this::currency` returns `EUR`. `Account::$currency` and `\Account::$currency` answer error 300. |
| Class constants in property trees | No | `$this` has no constants, with or without `show_hidden` and `extended_properties`. `property_get -n Account::LIMIT` and `self::LIMIT` answer error 300. `eval -- self::LIMIT` returns `100`, `self::SECRET` returns the private value, and `(new ReflectionClass($this))->getConstants()` returns `LIMIT`, `SECRET` and `VERSION`. Constants are reachable only through `eval`. |
| Readonly properties and enums | Shown | `id` has `facet="public readonly"`, `status` has `facet="public enum"` with `name` and `value` children. |
| Asynchronous `break` | No | `feature_get -n supports_async` is `0` and `-n break` is `supported="0"`. Sent 1 s into a 5 s script, `break` got no answer while the script ran. It was answered only after `run` returned `stopping`, with error 4 `unimplemented command` (`probe-async-break.txt`). The source has `/* DBGP_FUNC(break); */`. |
| `xdebug:ctrl_socket` `pause` | Yes, from inside the PHP container | See the next section. |
| `stdout` | Yes, copy and redirect | `stdout -c 1`: the `first line` output arrives as a `stream type=stdout` packet and the process prints it too. `eval -- print "from eval\n";` produces a stream packet. `stdout -c 2`: the client gets every line and the process prints nothing (`probe-stdout.txt`, `probe-stdout-redirect.txt`). |
| `stderr` | No | `feature_get -n stderr` says `1`, but `stderr -c 1` answers `success="0"`. |
| `property_set` | Yes, and it evaluates PHP | `property_set -n $balance -- strlen('abcdef')` sets `6` and `-t int -- 42` sets `42`, and the script then printed `42`. A readonly property is refused (`success="0"`, value unchanged). Xdebug builds `NAME = (cast) DATA` and evaluates it (`handler_dbgp.c`, line 1939): `-t` adds a cast only, so `-t string -- fortytwo` fails because `fortytwo` is not valid PHP. `property_set` runs code just as `eval` does. |
| Exception breakpoints on PHP errors | Yes | Names `Deprecated`, `Warning` and `Notice` paused at `errors.php` lines 11, 12 and 7, with `exception="Deprecated" code="8192"`, `"Warning" code="2"`, `"Notice" code="8"` and the PHP message. `RuntimeException` paused at line 15 even though the exception is caught. `-x *` pauses on all four (`probe-exceptions*.txt`). |
| Errors silenced with `@` | Paused too | On `GET /warning` a `Warning` breakpoint paused at line 54 (`@$settings['missing']`) and at line 55. `eval -- error_reporting()` returned `4437` at line 54 and `30719` at line 55, so the daemon can tell a silenced error and skip it, at the cost of one `eval` round trip (`probe-warning-route.txt`). |
| `call` and `return` breakpoints | Yes | `call` on the internal function `define`, on `Account::__construct` and on `Account::balance`, and `return` on `Account::balance`, each paused (`probe-callreturn.txt`). With `breakpoint_include_return_value` set to 1, the `return` pause carries `<xdebug:return_value><property type="float">10.5</property>`. Its `xdebug:message` gives the caller's line (46), and `stack_get` gives the function's line (38). |
| `watch` breakpoints | No | `breakpoint_set -t watch` answers error 201 `breakpoint type is not supported`. `breakpoint_types` is `line conditional call return exception`. |
| Conditional breakpoints | Yes | Accepted and listed with the base64 expression. |
| Other features | | `max_children` 32, `max_data` 1024, `max_depth` 1, `encoding` `iso-8859-1`, `multiple_sessions` 0, `language_supports_threads` 0. `detach`, `source`, `eval`, `property_value` and `typemap_get` exist, `interact` and `stdin` do not. |

### The control socket

`init` carries `xdebug:ctrl_socket="xdebug-ctrl.<pid>"`, where `<pid>` is also `appid`. In Xdebug 3.5.3 this is an abstract Unix socket on Linux (`\0xdebug-ctrl.<pid>`), a named pipe `\\.\pipe\xdebug-ctrl.<pid>` on Windows, and nothing on macOS. It accepts `ps` and `pause`. PHP polls it from the statement handler, every 25 ms by default (`xdebug.control_socket`).

`probe-ctrl-pause.txt`: with a DBGp session running a 5 s loop, `fixtures/probe/ctrl.sh` found the socket in `/proc/net/unix` inside `php85`. `ps` answered in 19.8 ms with the file, pid, run time and memory. `pause` answered `Breakpoint Signalled`, and the pending `run` returned `status="break"` at `long.php:7` after 1.85 s. `stack_get` and `property_get -n $spins` then worked. The same `ps` sent from the `toolchain` container failed with `Connection refused`: abstract sockets belong to a network namespace, so a daemon outside the PHP container cannot reach them.

`probe-ctrl-attach.txt`: with no trigger and so no session, `pause` answered `IDE Connection Signalled`. Xdebug then connected to the client in the middle of the loop: the `init` packet had no `idekey`, `status` was `starting`, and `stack_get` showed `long.php:7`. A request started without a trigger can therefore be attached to later.

The control socket is the only way to pause a running request. It matters for the pause button only if the daemon can run a command inside the PHP container (for example with `docker exec`), or when PHP runs on the same Linux host and network namespace as the daemon. On Windows the named pipe is not tested, and macOS native PHP has no control socket.

## Not verified

- Windows and macOS hosts: only the parsing and mapping logic was tested, against constructed packets. Docker Desktop's file sharing, the macOS case risk above and the Windows named pipe need those hosts.
- A large project: the walk was measured on 10 entries. Time and memory on a tree with a full `vendor/` folder are not measured.
- Xdebug 3.5's own `xdebug.path_mapping` setting (map files read on the server side) was not tried. It needs files in the application, so it does not fit a setup with no configuration.
- PHP versions other than 8.3 and 8.5, and Xdebug versions other than 3.5.3.

## Spec changes

ADRs:

- [ADR-0003](../../decisions/0003-framework-agnostic.md) stays accepted. S1 supports its claim that path mappings can be learned from the paths Xdebug reports. Its consequences can add that pausing a running request needs a helper inside the PHP container, because Xdebug has no asynchronous `break`.
- Write an ADR for the path mapping rules: longest suffix with a content tie-break through `source`, a remote check of every breakpoint through `source` (because `breakpoint_set` accepts missing files), local symlinks resolved, case folding chosen from the local file system, mappings learned per remote prefix and checked by entry script content on every session, and a second mapping learned from any frame outside the known prefixes.

[OPEN.md](../../OPEN.md):

- O-05: add that `property_set` evaluates PHP, so the "set variables" scope gives the same power as "eval". Class constants can be shown only through `eval`.
- O-02: Xdebug 3.5.3 on PHP 8.3 and 8.5 gave the same feature answers. Older Xdebug versions are still untested.
- Add a question: how the daemon pauses a running request. The options are a helper run inside the PHP container through the control socket, which needs access to Docker, or no pause button. It blocks the "Pause a running request" row of `docs/features.md`, and it also decides whether a request started without a trigger can be attached to.
- Add a question: whether exception breakpoints on PHP errors skip errors silenced with `@`. Skipping costs one `eval` of `error_reporting()` per pause, which falls under O-05.

[docs/features.md](../../../docs/features.md), rows marked "to confirm in S1":

| Row | Result |
|---|---|
| Path mapping inferred from the `init` packet, asked for only when ambiguous | Confirmed. The questions to ask are listed in criterion 2. |
| Break on PHP warnings, notices and deprecations | Confirmed: exception names `Warning`, `Notice` and `Deprecated`, and `*`. Errors silenced with `@` pause too. |
| Break on function call or return | Confirmed, internal functions included. The return value comes with `breakpoint_include_return_value`. |
| Data watchpoints, not planned unless S1 shows otherwise | Stays not planned: the `watch` type answers error 201. |
| Pause a running request, async `break` | Refuted for DBGp `break`. Possible only through the control socket from inside the PHP container. The row needs the new question above. |
| Variables per frame: locals, superglobals and user constants | Confirmed: contexts 0, 1 and 2. |
| `$this`, class statics and class constants | Statics confirmed, with visibility in `facet`. Class constants are refuted for the property tree and reachable only through `eval`. |
| Console `echo` output through the `stdout` command | Confirmed with `stdout -c 1`. |
