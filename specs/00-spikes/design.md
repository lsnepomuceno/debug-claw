# 00: Spikes, design

## Test bed

All spikes that need PHP share one fixture in `spikes/testbed/`:

- A Docker Compose setup with nginx and PHP-FPM, with Xdebug installed, `xdebug.mode=debug`, `xdebug.start_with_request=trigger` and `xdebug.client_host` pointing at the host.
- A second PHP service with a different PHP version, for S5. The versions follow [O-02](../OPEN.md#o-02-minimum-php-and-xdebug-versions); until it is answered, use the oldest and newest PHP versions in active support.
- A small plain PHP application with no framework: a front controller in `public/index.php`, a loop, an object graph deep enough to test variable trees, and an endpoint that runs about 100 PDO queries against SQLite.
- A copy of the same application mounted at a different container path, for the S1 layouts.

Requests are fired with `curl` and the `XDEBUG_TRIGGER` cookie, so that each spike controls exactly which requests start a session.

## Spike code

Each spike has its own Cargo project in `spikes/sN-name/`. The spikes do not share a workspace or code, so each can be deleted on its own. A spike that needs a DBGp client writes the minimum it needs (S1, S2 and S6 will each have a small one, and that duplication is accepted).

| Spike | Folder | What it builds |
|---|---|---|
| S1 | `spikes/s1-path-mapping/` | A DBGp listener that reads `init`, infers the mapping, sets one breakpoint and runs |
| S2 | `spikes/s2-logpoints/` | A DBGp listener that sets a breakpoint, captures values on each pause and continues, with timing |
| S3 | `spikes/s3-mcp-wait/` | An `rmcp` server over Streamable HTTP with a `wait` tool driven by a timer, plus a stdio shim |
| S4 | `spikes/s4-egui/` | An eframe app with the four views from the requirements, fed with generated data |
| S5 | `spikes/s5-extension/` | An `ext-php-rs` extension with one observer, plus a socket listener that prints what it receives |
| S6 | `spikes/s6-ide-proxy/` | A DBGp proxy between Xdebug and PhpStorm that injects reads |

## Order

S1, S2 and S3 come first, because milestone 1 depends on them. S6 reuses what S1 and S2 learn about DBGp. S4 and S5 are independent and can run at any point.

## Reports

Each spike ends with `reports/sN-name.md`, which answers its exit criteria one by one, with the commands and numbers that back each answer. A report ends with what changes in the specs: ADRs to accept, reject or write, and questions to add to or remove from [OPEN.md](../OPEN.md).
