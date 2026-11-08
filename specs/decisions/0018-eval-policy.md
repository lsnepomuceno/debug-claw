# ADR-0018: Eval policy and permission scopes

- Status: accepted
- Date: 2026-11-07
- Answers: O-05, and the eval part of O-12

## Context

Evaluating an expression runs PHP inside the application, so it can change data, send mail or call external services. The [vision](../../docs/vision.md) keeps it off by default. S1 found that `property_set` evaluates its value as PHP, so setting a variable has the same power as eval. S2 found that most reads match the path grammar of [ADR-0014](0014-reading-values.md) and need no eval. An agent that could turn eval on by itself would make the default meaningless.

## Decision

1. There are three permission scopes: read, control execution, and eval. Setting a variable belongs to eval. Every MCP tool belongs to one scope, and `design.md` lists them.
2. Scopes are set per project. Read is always granted. Control execution is on by default and can be turned off, which makes a project read only. Eval is off by default.
3. Only the user changes a project's scopes, outside MCP: in milestone 1 through a command of the daemon, and later also in the desktop app. No MCP tool turns a scope on. The design chooses the form of that command.
4. In milestone 1 a project's scopes apply to every MCP client. Granting different scopes per client comes with the desktop app.
5. While eval is off, expressions outside the path grammar are refused when they are sent or added (evaluate, watches, logpoints), with the reason and how eval is turned on.
6. The daemon may evaluate fixed expressions it writes itself, whatever the scope, when no user input reaches the expression. The only one so far is `error_reporting()`, read at a pause on a PHP error to tell errors silenced with `@` apart (O-12). These are not recorded as evaluations, because no client sent them.
7. In proxy mode the IDE's own `eval` commands pass through unchanged ([ADR-0016](0016-ide-proxy-mode.md)). The policy covers the daemon and its MCP clients.

## Alternatives rejected

- One global switch. A user who trusts eval on a local toy project would have it on for every client project too.
- A switch per session. The agent opens sessions all the time, so the user would be asked again and again, and the answer would end up always yes.
- A separate scope for setting variables. It would suggest less power than eval, which S1 showed is not true.
- An MCP tool that turns eval on. The agent could grant itself the scope.

## Consequences

- Class constants are visible only with eval on (S1).
- With eval off, a watch or a logpoint expression outside the path grammar is refused when it is added, not when it would run.
- Per-client grants need a way to tell clients apart beyond the name each one reports, which the desktop app milestone works out.
