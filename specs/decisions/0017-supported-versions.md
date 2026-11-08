# ADR-0017: Supported PHP and Xdebug versions

- Status: accepted
- Date: 2026-11-07
- Answers: O-02

## Context

The engine speaks DBGp to Xdebug, so the PHP version matters to it only through the Xdebug versions that run on it. Many projects still run PHP 7.4, often the ones that are hardest to debug. The spikes ran Xdebug 3.5.3 on PHP 8.3 and 8.5 only (S1, S2), and older Xdebug versions are untested. Xdebug 2 used other settings (`xdebug.remote_*`), another default port (9000) and no `xdebug.start_with_request`.

## Decision

1. debug-claw supports PHP 7.4 and later, with Xdebug 3.0 and later.
2. Xdebug 2 is not supported. When a session reports an Xdebug 2 version in its `init` packet, the daemon ends it and says why.
3. Milestone acceptance runs on both ends of the range: PHP 7.4 with the last Xdebug 3.0 release, and PHP 8.5 with the newest Xdebug the test bed uses (3.5.3 today). PHP 8.3 stays in the test bed.
4. A DBGp feature that Xdebug 3.0 lacks is either not used, or used only when the session's Xdebug has it, with a result that says what is missing on older versions. Each milestone's `design.md` lists those features.
5. Code that runs inside the application is written for PHP 7.4: the dump client file ([ADR-0005](0005-layered-php-integration.md)) and any PHP the daemon sends for evaluation.

## Alternatives rejected

- PHP 8.1 and later, the range S5 found for the extension. It would leave out the projects that still run 7.4 and 8.0, and the engine itself does not need 8.1.
- Supporting Xdebug 2 as well. Its configuration and port differ, the setup help would double, and it does not run on PHP 8.

## Consequences

- The test bed gains a PHP 7.4 container with Xdebug 3.0, built in milestone 1.
- The PHP extension cannot follow this range: `ext-php-rs` supports PHP 8.1 and later, and the observer API it uses exists from PHP 8.0 (S5). The extension's own minimum is [O-23](../OPEN.md#o-23-minimum-php-version-for-the-extension). Projects below it keep the engine and the dump client.
- Some behavior may differ between Xdebug 3.0 and 3.5 in ways the spikes did not see. The acceptance runs on 3.0 are where that shows up.
