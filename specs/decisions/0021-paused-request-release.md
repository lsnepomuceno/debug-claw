# ADR-0021: Releasing a paused request

- Status: accepted
- Date: 2026-11-07
- Answers: O-21

## Context

A paused request holds a PHP worker ([ADR-0003](0003-framework-agnostic.md)). An agent can stop calling in the middle of a session, and S3 saw the Inspector CLI give up on a wait without cancelling it. A held worker stays held until something drives the session. S6 also found that nginx answers HTTP 504 when a request is paused for longer than its `fastcgi_read_timeout`, 60 s by default, while PHP stays paused.

## Decision

1. When a paused session gets no command for the release timeout, the daemon sends `detach`. The request runs to its end without the debugger, and the session is recorded as released, with the reason.
2. The release timeout is 5 minutes by default and can be changed by a setting.
3. Every MCP tool call that targets the session counts as a command and restarts the timer: reads, steps, evaluations, breakpoint changes in that session, and waits on that session or its request.
4. A session waiting for the answer to an ambiguous mapping (R-09) is covered by the same timeout.
5. The setup help tells the user to raise the web server's read timeout (for nginx, `fastcgi_read_timeout`) above the release timeout, and says what happens if they do not.

## Alternatives rejected

- No timeout. A forgotten session holds a worker until someone restarts PHP, and with few workers the application stops answering.
- `stop` instead of `detach`. The request would end without a response, and the user would see an error that the debugger caused.
- A timeout that only counts steps. An agent reading variables for a long time would lose the session while working.

## Consequences

- The desktop app shows the countdown in the session header ([docs/ui.md](../../docs/ui.md#debugger)).
- A request released by the daemon finishes normally, so the user may not notice it was released. `status` and the session's end result say so.
