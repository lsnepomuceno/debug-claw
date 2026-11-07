# Desktop app

The desktop app should be pleasant enough that people keep it open next to the editor. Two references set the direction:

- Ray and LaraDumps for the feed: dumps arrive as cards with a colored edge, a label and a link to the line that produced them, and a sidebar filters them.
- The PhpStorm debugger for the debugging panes: frames, variables by context, watches, an evaluate field and stepping with the same keys.

This file describes intent. The app milestone turns it into requirements. [S4](../specs/00-spikes/reports/s4-egui.md) found that egui can reach it, and the toolkit and the view rules it relies on are in [ADR-0001](../specs/decisions/0001-rust-native-ui.md).

## Principles

1. Every action in the app is an engine capability that MCP also has ([ADR-0002](../specs/decisions/0002-mcp-first.md)). The app keeps a single registry where each action has a name, a one-sentence description, an icon, a shortcut and the MCP tool behind it. Buttons, menus, the command palette and tooltips are built from that registry, so they cannot drift apart.
2. Main actions show an icon and a text label. Icon-only buttons are used only where space is tight, such as the stepping toolbar.
3. Every button has a tooltip with the action's name, its shortcut, what happens when it is clicked, and the matching MCP tool (`MCP: step`), so a person learns what the agent can do by using the app. A disabled button keeps its tooltip and says why it is disabled ("Needs a paused session").
4. With no session yet, the app shows the setup snippets with copy buttons and a check that turns green when the first session connects. Every empty view says what would fill it.
5. The interface uses one accent color. Dump colors are the only other strong colors on screen. Motion only marks a change of state and then stops, because the app repaints only on events ([performance.md](performance.md)). Live elements (a countdown, elapsed time, a spinner) repaint at most once per second and only while they are visible, because each one costs a full window repaint (S4).
6. A command palette reaches every action. Stepping uses the PhpStorm keymap by default, and every shortcut can be changed.
7. Values use a monospace font and the interface a proportional one. Light and dark themes follow the system. Text scales with the system setting, and screen readers work through AccessKit.
8. When an MCP client changes something (a breakpoint, a step, an evaluation), the app shows it in plain words and marks who did it.

## Layout

A top bar, a rail of views on the left, and the active view.

```
+----------------------------------------------------------------------+
| (*) Listening on 9003 | 2 sessions | Agent: idle    [ Search ]  [Pin] |
+----+-----------------------------------------------------------------+
|Feed|                                                                 |
|Reqs|                                                                 |
|Dbg |                          active view                            |
|Prof|                                                                 |
|Bkpt|                                                                 |
|Set |                                                                 |
+----+-----------------------------------------------------------------+
```

The top bar shows the listener state, the open sessions, what the agent is doing, a global search and the always-on-top toggle. Rail items show an icon with a label, and collapse to icons with tooltips when the window is narrow.

### Feed

```
+-------------+--------------------------------------------------------+
| Screens     | [Clear] [Pause feed] [Clear on new request]  [Filter]  |
|  default 12 +--------------------------------------------------------+
|  orders   3 || orders  GET /orders/42        OrderService.php:42  12:01|
| Colors      ||  App\Order { id: 42, total: 0.0, items: [3] }          |
|  red      2 |+--------------------------------------------------------+
|  green    5 || query   GET /orders/42        Repository.php:88    12:01|
| Labels      ||  SELECT * FROM orders WHERE id = ?   [42]   1.2 ms     |
| Requests    |+--------------------------------------------------------+
+-------------+--------------------------------------------------------+
```

Each card has the colored edge, the label, the request it belongs to, the origin link and the time. Hovering a card shows copy, pin, open in editor and "hide similar". The sidebar lists screens, colors, labels and requests with counts, and each one is a filter.

### Debugger

```
+----------------------------------------------------------------------+
| POST /orders  paused at OrderService.php:42  driven by: you  [4:51]  |
| [Resume] [Step over] [Step into] [Step out] [Run to cursor] [Stop]   |
+----------------------+-----------------------------------------------+
| Frames               | 40   $order = $this->orders->find($id);       |
| > OrderService::charge 41   $total = $order->total;   $total: 0.0   |
|   OrderController::store 42 > $this->gateway->charge($total);       |
|   (vendor, 14 frames)|                                               |
+----------------------+-----------------------------------------------+
| Locals | Superglobals | Constants | Watches                          |
|  > $order  App\Order #812                                            |
|      id      42                                                      |
|      total   0.0     (changed)                                       |
+----------------------------------------------------------------------+
| Console  >>> $order->items->count()                                  |
|          3                                                           |
+----------------------------------------------------------------------+
```

- The session header shows the request, where it is paused, who is driving it and how long until the paused worker is released. "Take over" appears when the agent is driving.
- The toolbar uses the PhpStorm keys: Resume F9, Step over F8, Step into F7, Step out Shift+F8, Run to cursor Alt+F9, Stop Ctrl+F2, Evaluate Alt+F8.
- The source pane shows the current line, breakpoints in the gutter (click to toggle, right-click to edit) and values next to the lines.
- The variables pane has one tab per Xdebug context plus the watches. Values that changed since the last step are marked.
- The console runs PHP in the paused frame, under the eval policy ([O-05](../specs/OPEN.md#o-05-eval-policy)). When eval is off, it says so and links to the setting.

### Requests

A table of requests (method, URI, status, duration, peak memory) and, for the selected one, tabs: Timeline, Request, Response, Dumps, Queries, Exceptions, Profile. The timeline is the default tab: one row per event, in order, with the agent's actions in the same list.

### Profiler

The top functions table first, because it answers "what is slow" without reading a graph. The flame graph sits below it and expands on demand.

### Breakpoints

All breakpoints grouped by file, each with a toggle, its condition and hit count, and a badge for who created it. The exceptions section has a checkbox for all exceptions, a field for class names and checkboxes for PHP warnings, notices and deprecations.

### Activity

A drawer that lists every state change an MCP client made, in plain words, with a link to what it touched. It is where a person checks what the agent did while they were away.

## Visual system

Fixed in the app milestone. The starting candidates:

- Fonts: Inter for the interface and JetBrains Mono for code and values, both under the SIL Open Font License, which allows bundling them with an MIT app.
- Icons: Phosphor (MIT), the set S4 used, embedded directly. The `egui-phosphor` 0.14 crate is not used, because it pulls in a second egui. Lucide (ISC) was not tried.
- Every icon next to text sits in its own font section, because Inter's private use glyphs collide with Phosphor's codepoints (S4).
- Glyphs outside the bundled fonts (CJK, emoji) depend on whether `system_font_fallback` stays on ([O-17](../specs/OPEN.md#o-17-system-font-fallback)).
- Copying: tree rows copy through their context menu, and text in dumps, source and tables copies by selection (S4).
- Dump colors: a named palette (red, orange, yellow, green, blue, purple, gray), checked for contrast in both themes.
- Spacing, radius and type sizes as named tokens, defined once and used by every view.
