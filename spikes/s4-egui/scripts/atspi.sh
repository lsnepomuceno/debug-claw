#!/bin/sh
# Screen reader check through AT-SPI under Xvfb. Starts a private D-Bus
# session and the AT-SPI bus, marks accessibility as enabled (what a
# running screen reader does), opens one view and walks its tree.
#   sh scripts/atspi.sh VIEW [app args...]
# Called again by itself inside dbus-run-session.
set -eu
if [ -z "${S4_IN_DBUS:-}" ]; then
    S4_IN_DBUS=1 exec dbus-run-session -- sh "$0" "$@"
fi
view=$1
shift
/usr/libexec/at-spi-bus-launcher --launch-immediately >/tmp/atspi-bus.log 2>&1 &
sleep 2
dbus-send --session --print-reply --dest=org.a11y.Bus /org/a11y/bus \
    org.freedesktop.DBus.Properties.Set string:org.a11y.Status string:IsEnabled variant:boolean:true >/dev/null
dbus-send --session --print-reply --dest=org.a11y.Bus /org/a11y/bus \
    org.freedesktop.DBus.Properties.Set string:org.a11y.Status string:ScreenReaderEnabled variant:boolean:true >/dev/null
/target/release/s4-egui --backend wgpu --view "$view" "$@" >/tmp/atspi-app.log 2>&1 &
pid=$!
sleep 6
python3 -I scripts/atspi_dump.py 40
kill "$pid"
