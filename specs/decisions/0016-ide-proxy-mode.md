# ADR-0016: IDE proxy mode

- Status: accepted
- Date: 2026-11-07
- Evidence: [S6 report](../00-spikes/reports/s6-ide-proxy.md)

## Context

Xdebug connects to one client per request ([ADR-0003](0003-framework-agnostic.md)). A user who debugs in PhpStorm and also wants an agent to read the same session needs debug-claw between the two. Spike S6 built a proxy and tested it with PhpStorm 2026.3 and with an emulator that replays PhpStorm's captured command sequence.

## Decision

1. In proxy mode the daemon listens for Xdebug on 9003 and connects to the IDE on another port. The IDE drives the session: breakpoints, stepping, eval and everything else it sends reach Xdebug as it sent them.
2. The daemon owns the transaction IDs on the Xdebug socket. Every command written to Xdebug, from the IDE, from the agent or from the daemon, gets the next number of a per-session counter. A table maps each number to the original ID and to where the response goes, and the response is written with the original ID restored. The table holds at most 1024 entries. A response whose ID is not in the table goes to the IDE unchanged.
3. `init`, `notify` and `stream` packets go to the IDE unchanged and in order. The agent does not see them.
4. An agent may only read, and only with these commands: `stack_depth`, `stack_get`, `context_names`, `context_get`, `property_get`, `property_value`, `typemap_get`, `source`, `status`, `feature_get`, `breakpoint_get` and `breakpoint_list`. Per-command options (`-m`, `-p`, `-d`, `-c`) are allowed. Every other command is refused, because the list is an allowlist.
5. An agent read is written only while the session is paused and only for the pause the agent waited for. The daemon tracks the state from the IDE's continuation commands and Xdebug's answers, and checks the state under the same lock the IDE's commands take.
6. A refused agent command gets a DBGp error with a code outside Xdebug's range: 900 unsafe command, 901 not paused, 902 no session or session ended, 903 the session has moved to another pause.
7. When the IDE closes during a session, the daemon sends `detach` to Xdebug. When the IDE cannot be reached at the start of a session, the daemon drops the session.

## Alternatives rejected

- A range of transaction IDs reserved for the agent. It depends on how PhpStorm counts.
- Agent reads only when the IDE has no command in flight. The agent would wait on the IDE.
- Letting the agent step, set breakpoints, change features, redirect output, set values or eval. Each of these changes what the IDE sees or the state it shows.

## Consequences

- The agent's view of an IDE-driven session is read only, and only during the IDE's pauses.
- The proxy added 12 to 38 us at the median and up to 173 us at p99 per round trip. The slowest single round trip took 726 us.
- PhpStorm makes a pause for itself: after `init` it sends `step_into`, so the script stops on its first line while PhpStorm reads `PHP_IDE_CONFIG` and sets its breakpoints. The agent's wait sees that pause too ([O-18](../OPEN.md#o-18-proxy-mode-details)).
- PhpStorm sends `eval` itself, three times per session, to read Xdebug settings and `PHP_IDE_CONFIG`. Those pass through unchanged: the eval policy ([O-05](../OPEN.md#o-05-eval-policy)) covers the daemon and the agent, not the IDE.
- PhpStorm ends a finished session with `detach`.
- How the daemon finds the IDE, agent breakpoints while the IDE drives, stream packets for the agent, which session the agent reads when several run at once, and whether the agent's wait skips the IDE's own pauses stay open ([O-18](../OPEN.md#o-18-proxy-mode-details)).
