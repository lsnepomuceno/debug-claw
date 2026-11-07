# 00: Spikes, tasks

Each task names the exit criteria it closes (see [requirements.md](requirements.md)).

## Test bed

- [x] T-01 Build `spikes/testbed/` with the `toolchain` service, nginx, PHP-FPM with Xdebug, and the plain PHP fixture application. Done when a `curl` with `XDEBUG_TRIGGER` opens a DBGp connection to a listener in the `toolchain` container on port 9003, and nothing was installed on the host.
- [x] T-02 Add the second PHP version service and the second mount path. Done when both answer requests.

## S1 Path mapping

- [ ] T-03 Read the `init` packet and infer the mapping by longest suffix. Closes S1 criterion 1 for the root layout.
- [ ] T-04 Repeat with the `public/` entry script layout. Closes S1 criterion 1.
- [ ] T-05 Build the ambiguous cases (duplicate relative path, vendor elsewhere, symlink) and record the results. Closes S1 criteria 2 and 3.
- [ ] T-06 Probe the DBGp features listed in S1 criterion 4, then write `reports/s1-path-mapping.md`. Closes S1 criterion 4.

## S2 Logpoints

- [ ] T-07 Capture and continue on a breakpoint inside a loop. Closes S2 criterion 1.
- [ ] T-08 Measure the time per hit, local and container, with 1 and 5 expressions, and the daemon's CPU and memory over 10 000 hits. Closes S2 criteria 2 and 3.
- [ ] T-09 Test `hit_value` and `hit_condition`. Closes S2 criterion 4.
- [ ] T-10 Run two triggered requests at once. Closes S2 criterion 5.
- [ ] T-11 List which expressions `property_get` can read without eval. Closes S2 criterion 6.
- [ ] T-12 Compare DBGp response sizes with and without the size limits. Closes S2 criterion 7.
- [ ] T-13 Write `reports/s2-logpoints.md`.

## S3 Blocking MCP tools

- [ ] T-14 Serve a `wait` tool over Streamable HTTP and call it from Claude Code and the MCP Inspector. Closes S3 criteria 1 and 2.
- [ ] T-15 Put the stdio shim in front of it. Closes S3 criterion 3.
- [ ] T-16 Test server-initiated notifications in the same clients. Closes S3 criterion 4.
- [ ] T-17 Measure the idle server for 10 minutes and the CPU of a blocked `wait`. Closes S3 criteria 5 and 6.
- [ ] T-18 Write `reports/s3-mcp-wait.md`.

## S4 Native UI

- [ ] T-19 Build the variable tree and the dump list with generated data. Closes S4 criteria 1 and 2.
- [ ] T-20 Build the flame graph and the source view. Closes S4 criteria 3 and 4.
- [ ] T-21 Build the styled mockups of the feed and the debugger, and check selection, themes, tooltips and AccessKit. Closes S4 criteria 5 and 9.
- [ ] T-22 Measure idle CPU and RSS per dataset on `glow` and `wgpu`, and compare tree memory at 1 000 and 100 000 nodes. Closes S4 criteria 6, 7 and 8.
- [ ] T-23 Write `reports/s4-egui.md`, including the alternative toolkit criterion if any view failed.

## S5 PHP extension

- [ ] T-24 Build the extension with one observer on `PDOStatement::execute`. Closes S5 criteria 1 and 7.
- [ ] T-25 Build and run it on both PHP versions. Closes S5 criterion 2.
- [ ] T-26 Measure the overhead on the 100-query endpoint, with the daemon up and down. Closes S5 criteria 3 and 5.
- [ ] T-27 Run 10 000 requests on one worker and compare its memory with and without the extension, and verify that unobserved functions get no handler. Closes S5 criteria 4 and 6.
- [ ] T-28 Write `reports/s5-extension.md` with the recommendation. Closes S5 criterion 8.

## S6 IDE coexistence

- [ ] T-29 Relay a session to PhpStorm through the proxy. Closes S6 criterion 1.
- [ ] T-30 Inject stack and variable reads while PhpStorm holds the pause, and measure the added latency per round trip. Closes S6 criteria 2, 3 and 4.
- [ ] T-31 Write `reports/s6-ide-proxy.md`.

## Closing the milestone

- [ ] T-32 Apply what the reports say to the ADRs and OPEN.md, then write the folder and requirements for milestone 1.
