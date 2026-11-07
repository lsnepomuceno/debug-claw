# 00: Spikes, tasks

Each task names the exit criteria it closes (see [requirements.md](requirements.md)).

## Test bed

- [ ] T-01 Build `spikes/testbed/` with nginx, PHP-FPM and Xdebug, and the plain PHP fixture application. Done when a `curl` with `XDEBUG_TRIGGER` opens a DBGp connection to the host on port 9003.
- [ ] T-02 Add the second PHP version service and the second mount path. Done when both answer requests.

## S1 Path mapping

- [ ] T-03 Read the `init` packet and infer the mapping by longest suffix. Closes S1 criterion 1 for the root layout.
- [ ] T-04 Repeat with the `public/` entry script layout. Closes S1 criterion 1.
- [ ] T-05 Build the ambiguous cases (duplicate relative path, vendor elsewhere, symlink) and record the results. Closes S1 criteria 2 and 3.
- [ ] T-06 Write `reports/s1-path-mapping.md`.

## S2 Logpoints

- [ ] T-07 Capture and continue on a breakpoint inside a loop. Closes S2 criterion 1.
- [ ] T-08 Measure the time per hit, local and container, with 1 and 5 expressions. Closes S2 criterion 2.
- [ ] T-09 Test `hit_value` and `hit_condition`. Closes S2 criterion 3.
- [ ] T-10 Run two triggered requests at once. Closes S2 criterion 4.
- [ ] T-11 List which expressions `property_get` can read without eval. Closes S2 criterion 5.
- [ ] T-12 Write `reports/s2-logpoints.md`.

## S3 Blocking MCP tools

- [ ] T-13 Serve a `wait` tool over Streamable HTTP and call it from Claude Code and one other client. Closes S3 criteria 1 and 2.
- [ ] T-14 Put the stdio shim in front of it. Closes S3 criterion 3.
- [ ] T-15 Test server-initiated notifications in the same clients. Closes S3 criterion 4.
- [ ] T-16 Write `reports/s3-mcp-wait.md`.

## S4 Native UI

- [ ] T-17 Build the variable tree and the dump list with generated data. Closes S4 criteria 1 and 2.
- [ ] T-18 Build the flame graph and the source view. Closes S4 criteria 3 and 4.
- [ ] T-19 Check selection, themes and AccessKit. Closes S4 criterion 5.
- [ ] T-20 Write `reports/s4-egui.md`, including criterion 6 if any view failed.

## S5 PHP extension

- [ ] T-21 Build the extension with one observer on `PDOStatement::execute`. Closes S5 criteria 1 and 4.
- [ ] T-22 Build and run it on both PHP versions. Closes S5 criterion 2.
- [ ] T-23 Measure the overhead on the 100-query endpoint. Closes S5 criterion 3.
- [ ] T-24 Write `reports/s5-extension.md` with the recommendation. Closes S5 criterion 5.

## S6 IDE coexistence

- [ ] T-25 Relay a session to PhpStorm through the proxy. Closes S6 criterion 1.
- [ ] T-26 Inject stack and variable reads while PhpStorm holds the pause. Closes S6 criteria 2 and 3.
- [ ] T-27 Write `reports/s6-ide-proxy.md`.

## Closing the milestone

- [ ] T-28 Apply what the reports say to the ADRs and OPEN.md, then write the folder and requirements for milestone 1.
