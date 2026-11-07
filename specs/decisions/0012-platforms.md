# ADR-0012: Linux, Windows and macOS

- Status: accepted
- Date: 2026-11-07

## Context

PHP developers work on all three desktop systems, often with the application in Docker and the editor on the host. [O-03](../OPEN.md) assumed Linux first and left the others open. The answer affects path mapping, the local connection between the desktop app and the daemon, the PHP extension builds and packaging.

## Decision

The daemon, the stdio shim and the desktop app run on Linux, Windows and macOS. macOS is supported where it is feasible; when a feature cannot work there, the documentation says so.

- Core crates use no API that exists on only one system. Anything system-specific sits behind the boundary traits of [ADR-0010](0010-workspace-layout.md) and is chosen in the binaries.
- Path mapping treats host paths as either Windows or POSIX paths: drive letters, backslashes and case-insensitive file systems (Windows, and macOS by default) are handled, as are `file:///C:/...` URIs from PHP running natively on Windows.
- Setup snippets differ per system: Docker Desktop on Windows and macOS resolves `host.docker.internal` on its own, and Linux needs `extra_hosts: host.docker.internal:host-gateway`.
- The PHP extension ([ADR-0005](0005-layered-php-integration.md)) has its own build matrix, which S5 reports. It may reach the three systems at different times.

## Alternatives rejected

- Linux only, with the others later. Path handling and the app-to-daemon connection would be designed around POSIX and need rework.
- Linux and macOS only. Windows is a large share of PHP developers, and its paths are the hardest case for path mapping.

## Consequences

- Development still happens in Docker on a Linux host ([ADR-0011](0011-docker-development.md)), which cannot run Windows or macOS. Those two are built and tested on CI runners once CI exists, and spikes cover them with recorded data and documentation where they cannot run them.
- Packaging and code signing differ per system ([O-10](../OPEN.md#o-10-packaging-and-code-signing)).
