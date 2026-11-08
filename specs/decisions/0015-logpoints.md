# ADR-0015: Logpoints over DBGp

- Status: accepted
- Date: 2026-11-07
- Evidence: [S2 report](../00-spikes/reports/s2-logpoints.md)

## Context

A logpoint records a message built from expressions each time a line runs, and the request goes on. DBGp has no breakpoint that records without pausing, so a logpoint is a line breakpoint that pauses, reads, and continues. Spike S2 measured what that costs with Xdebug 3.5.3 on PHP 8.5: with 5 expressions, 64 to 77 us added per hit when each read waits for its answer, and 41 to 45 us when the reads and the `run` go out in one write.

## Decision

1. A logpoint is a line breakpoint. At each pause the daemon writes every `property_get` of the logpoint and the `run` in one write, then reads the answers in order.
2. While eval is off, a logpoint accepts only expressions that match the path grammar of [ADR-0014](0014-reading-values.md). A message template such as `total={$order->total}` is split into literal text and path expressions. The check runs when the logpoint is set. An expression that needs eval is refused then, with the reason, under [ADR-0018](0018-eval-policy.md).
3. Each value is read with `max_children` 8 and `max_data` 256 (`-m 256` on each `property_get`), smaller than the session defaults of ADR-0014, because captures stay in a buffer of many entries.
4. Hit conditions map to DBGp: "from hit K on" is `-h K -o >=`, "only hit K" is `-h K -o ==`, and "every K-th hit" is `-h K -o %`. Xdebug evaluates them and pauses only on matching hits.
5. "At most K captures" has no DBGp operator. The daemon removes the breakpoint with `breakpoint_remove` at the K-th capture.
6. Hit counts are read with `breakpoint_get` while paused, because Xdebug refuses it once the script is stopping.
7. Captures go to a buffer with a fixed cap. The oldest capture is dropped first and the drop is counted ([rule 4](../../docs/performance.md#rules)). The cap value is [O-14](../OPEN.md#o-14-logpoint-capture-buffer-cap).

## Alternatives rejected

- One command at a time. It costs 35 to 40% more per hit.
- A `<` hit condition for the cap. DBGp defines none.
- Logpoints through the `auto_prepend_file` dump client or the extension ([ADR-0005](0005-layered-php-integration.md)). Both need code installed in the application, and layer 1 must work with Xdebug alone.

## Consequences

- The worker pauses on every hit. Features describe a logpoint as pausing briefly and continuing, not as running without a pause.
- A breakpoint whose hit condition does not match still costs about 0.4 us per visit.
- `breakpoint_update -s disabled` should also work as the cap, but S2 did not test it.
- S2 measured the daemon and PHP on one host, in containers. A daemon on the host talking to PHP in a container, PHP 8.3 and Windows or macOS hosts were not measured.
- Two requests with logpoints at once ran 17 to 25% slower each. S2 did not find the cause.
