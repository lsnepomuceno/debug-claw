#!/bin/sh
# Runs inside php85. Finds the Xdebug control socket of a running PHP
# process in /proc/net/unix (abstract names start with @) and sends it
# one command.
#
#   sh /probe/ctrl.sh list | ps | pause
names=$(grep -o '@xdebug-ctrl\.[0-9]*' /proc/net/unix | tr -d '@')
if [ "$1" = list ]; then
    echo "control sockets: ${names:-none}"
    exit 0
fi
name=$(echo "$names" | head -n 1)
[ -n "$name" ] || { echo "no Xdebug control socket" >&2; exit 1; }
exec /target/debug/s1-path-mapping ctrl "$name" "$1"
