# ADR-0013: Path mapping inferred from what Xdebug reports

- Status: accepted
- Date: 2026-11-07
- Evidence: [S1 report](../00-spikes/reports/s1-path-mapping.md)

## Context

[ADR-0003](0003-framework-agnostic.md) requires that the daemon learn path mappings from the paths Xdebug reports and ask the user only when inference fails. Spike S1 tested that on two container layouts, with ambiguous cases, and with recorded `init` packets for Windows and macOS hosts ([ADR-0012](0012-platforms.md)). Two Xdebug behaviours shape the rules below: `breakpoint_set` accepts a file that does not exist and answers with an id and no error, and Xdebug resolves symlinks both in `breakpoint_set` and in the paths it reports for included files.

## Decision

Remote paths are the paths Xdebug reports as `file://` URIs. Local paths are paths in the user's project tree. The daemon maps between them with a list of learned mappings, each a remote prefix and a local root.

1. Inference starts from the `fileuri` of the `init` packet, decoded the way Xdebug encodes it: percent encoding, `file:///C:/...` for a drive and `file://server/share/...` for UNC.
2. The daemon walks the local project tree for files with the same base name. The walk does not follow symlinks, skips version control folders, `node_modules` and `.idea`, and stops at a fixed cap of entries.
3. The candidate with the longest common suffix of path components wins. The remote prefix and the local root are what remains of each path.
4. When there is one candidate or several tie, the daemon reads the entry script through DBGp (`source -f URI`) and compares it with the local file. Content breaks a tie. A tie that content does not break becomes a question for the user, offering every candidate, and the answer is remembered.
5. A unique candidate whose content differs from the local file is still used, with a warning that the container runs other code than the local tree.
6. Every breakpoint is translated, then checked on the remote side with `source -f URI -b L -e L` before `breakpoint_set`. A breakpoint whose remote file is missing is reported as not placed, never as set. All of this happens before the first `run`.
7. A frame whose path is outside every known remote prefix starts a second inference on that path. A new mapping is added next to the existing ones, and breakpoints that were not placed are tried again.
8. A learned mapping is reused on later sessions. On each session the daemon checks the entry script by content, one `source` round trip. The mapping is dropped and inferred again when that content differs, when a remote breakpoint check fails, or when the local root is gone. The remote prefix alone never identifies a project, because many images use `/var/www/html`.
9. Local symlinks are resolved before comparing, so a breakpoint set through a link is shown where Xdebug reports it.
10. Case folding follows the local file system: names compare without case when it folds case (Windows, and macOS by default), and with case otherwise. A breakpoint is always sent with the spelling found on disk by the walk, and the remote spelling is kept in every URI the daemon builds.
11. Windows paths keep their drive letter and accept backslashes, forward slashes and either case of the drive. A local path on another drive never matches.

## Alternatives rejected

- Longest suffix alone. In the root layout of S1 both `index.php` and `public/index.php` match one component, and the rule would pick either.
- Trusting `breakpoint_set`. Xdebug answers it for files that do not exist, so a wrong mapping would look like a breakpoint that is never hit.
- Xdebug 3.5's `xdebug.path_mapping` setting. It needs map files inside the application, which works against minimal configuration ([vision](../../docs/vision.md#principles), principle 3).
- Asking the user for the mapping up front, as most IDEs do.

## Consequences

- Each session costs one `source` round trip for the entry script check and one per breakpoint. S1 measured 5.0 ms for the first entry script read and 6.3 to 10.0 ms from `init` to the first `run` with two breakpoints.
- The walk was measured only on a 10-entry tree. Its time and memory on a project with a full `vendor/` folder are unknown, and its cap is a design value.
- Windows and macOS hosts were covered by unit tests against recorded and constructed `init` packets. Docker Desktop file sharing and the macOS case risk in rule 10 (S1 read it from the Xdebug source, where one check compares file names with case) need runs on those hosts.
- How the daemon learns where the local project tree is stays open ([O-20](../OPEN.md#o-20-the-local-project-root)).
