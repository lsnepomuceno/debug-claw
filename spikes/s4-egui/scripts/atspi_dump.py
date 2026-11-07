"""Walk the AT-SPI tree of every application on the a11y bus and print
the roles, names and states a screen reader would get.

Run by scripts/atspi.sh inside a D-Bus session with the AT-SPI bus up.
"""
import collections
import sys

import gi

gi.require_version("Atspi", "2.0")
from gi.repository import Atspi  # noqa: E402

limit = int(sys.argv[1]) if len(sys.argv) > 1 else 40
desktop = Atspi.get_desktop(0)
print("apps", desktop.get_child_count())

for i in range(desktop.get_child_count()):
    app = desktop.get_child_at_index(i)
    if app is None:
        continue
    roles = collections.Counter()
    buttons = []
    sample = []

    def walk(node, depth):
        if node is None or depth > 60:
            return
        role = node.get_role_name()
        name = node.get_name() or ""
        roles[role] += 1
        if role in ("button", "push button"):
            states = node.get_state_set()
            enabled = states.contains(Atspi.StateType.ENABLED)
            buttons.append(name + ("" if enabled else " (disabled)"))
        elif name and len(sample) < limit:
            sample.append("%s:%s" % (role, name[:60]))
        for k in range(node.get_child_count()):
            walk(node.get_child_at_index(k), depth + 1)

    walk(app, 0)
    print("app", repr(app.get_name()), "nodes", sum(roles.values()))
    print("roles", dict(roles))
    print("buttons", ", ".join(buttons))
    print("sample", " ; ".join(sample))
