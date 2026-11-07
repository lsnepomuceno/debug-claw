# ADR-0008: Resource use is a requirement, with budgets

- Status: accepted
- Date: 2026-11-07

## Context

A debugger runs alongside the application it debugs, on the same machine. If it slows the request down, the developer measures the debugger instead of the application. If the daemon or the desktop app hold a lot of memory or wake the CPU when nothing happens, they compete with Docker, the IDE and the browser. Low resource use was set as a priority for this project.

## Decision

Memory use and CPU use are requirements. They have explicit budgets per component, listed in [docs/performance.md](../../docs/performance.md), and the rules in that document apply to every milestone:

- every milestone's `requirements.md` has requirements for the budgets that apply to it, with acceptance criteria that measure them;
- a milestone is not done while it misses a budget;
- once code exists, the budgets become automated checks.

The first numbers are drafts. The spikes measure the real costs and the spike reports confirm or revise them ([O-08](../OPEN.md#o-08-confirm-the-resource-budgets)).

## Alternatives rejected

- Optimize later, once the features work. Unbounded buffers, polling loops and eager loading of variables shape the architecture, and removing them later means rewriting the parts built on them.
- Performance as a goal without numbers. Without a budget there is no point at which a milestone fails, so the goal does not change any decision.

## Consequences

- Features that need unbounded memory (keeping every event forever, loading a whole object graph at once) are out unless they get a cap.
- The desktop app toolkit has to support repainting on demand, and S4 checks it.
- Some convenience is lost: variables load one level at a time, old events are evicted, and large results are paged.
